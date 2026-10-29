//! The playback clock the picture follows: where the programme is at the loudspeaker *now*.
//!
//! The audio callback only knows the position of the buffer it just filled, and it runs every few
//! milliseconds. The device then plays that buffer a little later (its output latency). So the
//! callback publishes an anchor (the position of the first frame of the buffer, the time of the
//! callback and the latency) and the reader extrapolates from it with the wall clock. The picture
//! then lines up with what is heard, instead of with what was just handed to the device and moving
//! in steps of one buffer.
//!
//! The anchor is published without a lock (a sequence counter around plain atomics), so the audio
//! callback never waits for the reader.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// What the callback knows at the moment it fills a buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockAnchor {
    /// Programme position, in seconds, of the first frame of the buffer.
    pub position: f64,
    /// Microseconds since the player's time base at the callback.
    pub callback_micros: u64,
    /// Seconds from the callback until that first frame leaves the loudspeaker.
    pub latency: f64,
    /// Playback speed (`1.0` normal, negative backwards).
    pub speed: f64,
}

/// The programme position at the loudspeaker at `now_micros` (same time base as the anchor).
pub fn heard_position(anchor: &ClockAnchor, now_micros: u64) -> f64 {
    let elapsed = now_micros.saturating_sub(anchor.callback_micros) as f64 / 1e6;
    anchor.position + (elapsed - anchor.latency) * anchor.speed
}

/// The position to show: the new estimate, except that a forward clock never steps back by a hair
/// (a small correction when a new anchor arrives) and a backward one never steps forward. Bigger
/// jumps are real (a seek, a loop) and pass through.
pub fn steady(previous: f64, estimate: f64, speed: f64) -> f64 {
    const JITTER: f64 = 0.04;
    let against = if speed >= 0.0 {
        previous - estimate
    } else {
        estimate - previous
    };
    if against > 0.0 && against < JITTER {
        previous
    } else {
        estimate
    }
}

/// A lock-free cell holding the latest [`ClockAnchor`].
#[derive(Debug, Default)]
pub struct ClockCell {
    /// Odd while the callback is writing.
    sequence: AtomicU64,
    valid: AtomicBool,
    position: AtomicU64,
    callback_micros: AtomicU64,
    latency: AtomicU64,
    speed: AtomicU64,
}

impl ClockCell {
    /// Publishes a new anchor (called by the audio callback; never blocks).
    pub fn write(&self, anchor: &ClockAnchor) {
        self.sequence.fetch_add(1, Ordering::AcqRel);
        self.position
            .store(anchor.position.to_bits(), Ordering::Relaxed);
        self.callback_micros
            .store(anchor.callback_micros, Ordering::Relaxed);
        self.latency
            .store(anchor.latency.to_bits(), Ordering::Relaxed);
        self.speed.store(anchor.speed.to_bits(), Ordering::Relaxed);
        self.valid.store(true, Ordering::Relaxed);
        self.sequence.fetch_add(1, Ordering::AcqRel);
    }

    /// Forgets the anchor (a start or a seek: the old one describes another position).
    pub fn invalidate(&self) {
        self.valid.store(false, Ordering::Relaxed);
    }

    /// The anchor, or `None` when there is none yet or one is being written (the reader falls back
    /// to the buffer position).
    pub fn read(&self) -> Option<ClockAnchor> {
        for _ in 0..4 {
            let before = self.sequence.load(Ordering::Acquire);
            if before % 2 == 1 {
                continue;
            }
            let anchor = ClockAnchor {
                position: f64::from_bits(self.position.load(Ordering::Relaxed)),
                callback_micros: self.callback_micros.load(Ordering::Relaxed),
                latency: f64::from_bits(self.latency.load(Ordering::Relaxed)),
                speed: f64::from_bits(self.speed.load(Ordering::Relaxed)),
            };
            let valid = self.valid.load(Ordering::Relaxed);
            if self.sequence.load(Ordering::Acquire) == before {
                return valid.then_some(anchor);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor(position: f64, latency: f64, speed: f64) -> ClockAnchor {
        ClockAnchor {
            position,
            callback_micros: 1_000_000,
            latency,
            speed,
        }
    }

    #[test]
    fn the_estimate_runs_with_the_wall_clock_from_the_anchor_minus_the_latency() {
        let a = anchor(10.0, 0.05, 1.0);
        // At the callback itself the buffer has not reached the loudspeaker yet
        assert!((heard_position(&a, 1_000_000) - 9.95).abs() < 1e-9);
        // Fifty milliseconds later its first frame is heard
        assert!((heard_position(&a, 1_050_000) - 10.0).abs() < 1e-9);
        // A UI frame later, 16 ms on
        assert!((heard_position(&a, 1_066_000) - 10.016).abs() < 1e-9);
        // Double speed covers twice the programme; backwards goes the other way
        let fast = anchor(10.0, 0.0, 2.0);
        assert!((heard_position(&fast, 1_100_000) - 10.2).abs() < 1e-9);
        let back = anchor(10.0, 0.0, -1.0);
        assert!((heard_position(&back, 1_100_000) - 9.9).abs() < 1e-9);
        // A reader earlier than the callback does not run the clock backwards past the anchor
        assert!((heard_position(&a, 500_000) - 9.95).abs() < 1e-9);
    }

    #[test]
    fn small_steps_against_the_direction_are_held_and_real_jumps_pass() {
        assert_eq!(steady(5.0, 4.99, 1.0), 5.0, "a hair back, forward play");
        assert_eq!(steady(5.0, 5.02, 1.0), 5.02);
        assert_eq!(steady(5.0, 1.0, 1.0), 1.0, "a seek or a loop passes");
        assert_eq!(
            steady(5.0, 5.01, -1.0),
            5.0,
            "a hair forward, backward play"
        );
        assert_eq!(steady(5.0, 4.9, -1.0), 4.9);
        assert_eq!(steady(5.0, 9.0, -1.0), 9.0);
    }

    #[test]
    fn the_cell_returns_what_was_written_and_nothing_after_an_invalidation() {
        let cell = ClockCell::default();
        assert!(cell.read().is_none(), "nothing yet");
        let a = anchor(3.5, 0.02, 1.0);
        cell.write(&a);
        assert_eq!(cell.read(), Some(a));
        let b = anchor(4.0, 0.03, -2.0);
        cell.write(&b);
        assert_eq!(cell.read(), Some(b));
        cell.invalidate();
        assert!(cell.read().is_none(), "a seek drops the old anchor");
        cell.write(&a);
        assert_eq!(cell.read(), Some(a));
    }

    #[test]
    fn a_reader_never_sees_a_torn_anchor_while_the_writer_runs() {
        use std::sync::Arc;
        let cell = Arc::new(ClockCell::default());
        let writer = {
            let cell = Arc::clone(&cell);
            std::thread::spawn(move || {
                for i in 0..20_000u64 {
                    // Every field carries the same number, so a mixed read is visible
                    let v = i as f64;
                    cell.write(&ClockAnchor {
                        position: v,
                        callback_micros: i,
                        latency: v,
                        speed: v,
                    });
                }
            })
        };
        let mut seen = 0;
        while !writer.is_finished() {
            if let Some(a) = cell.read() {
                assert_eq!(a.position, a.callback_micros as f64);
                assert_eq!(a.position, a.latency);
                assert_eq!(a.position, a.speed);
                seen += 1;
            }
        }
        writer.join().unwrap();
        assert!(seen > 0 || cell.read().is_some());
    }
}
