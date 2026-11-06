//! Persistent Sync Links and Audio/Video Out-of-Sync Resynchronization.
//!
//! In VEGAS Pro semantics (manual v21, *Repairing audio/video synchronization offsets* and
//! *Using sync links*):
//! - A sync link is a persistent one-way link between a master event and a dependent (linked) event.
//! - The linked event stays synchronized with its master: moving the master moves the linked event,
//!   but moving the linked event does not move the master.
//! - When audio and video events (or events from the same source media) drift out of alignment,
//!   the out-of-sync drift is calculated as:
//!   `drift = (current_offset) - (base_offset)`
//! - Two resynchronization actions are provided:
//!   - `resync_by_moving`: moves the event's timeline start to restore synchronization.
//!   - `resync_by_slipping`: slips the event's source media in-point (`trim_in`) to restore
//!     synchronization without changing the event's position or duration on the timeline.

use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TIME_EPSILON};
use serde::{Deserialize, Serialize};

/// A persistent one-way synchronization link between a master event and a dependent linked event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncLink {
    /// Track of the master event.
    pub master_track: TrackId,
    /// Identifier of the master event.
    pub master_event: EventId,
    /// Track of the dependent (linked) event.
    pub linked_track: TrackId,
    /// Identifier of the dependent (linked) event.
    pub linked_event: EventId,
    /// Baseline offset in seconds:
    /// `(linked_start - master_start) - (linked_trim_in - master_trim_in)`.
    pub base_offset_sec: f64,
}

impl SyncLink {
    /// Creates a new persistent synchronization link with an explicit baseline offset.
    pub fn new(
        master_track: TrackId,
        master_event: EventId,
        linked_track: TrackId,
        linked_event: EventId,
        base_offset_sec: f64,
    ) -> Self {
        Self {
            master_track,
            master_event,
            linked_track,
            linked_event,
            base_offset_sec,
        }
    }

    /// Creates a sync link from current event geometries in the timeline, establishing the
    /// current relative alignment as the baseline offset.
    pub fn from_events(
        master_ref: EventRef,
        master: &TrackEvent,
        linked_ref: EventRef,
        linked: &TrackEvent,
    ) -> Self {
        let base_offset_sec = (linked.start - master.start) - (linked.trim_in - master.trim_in);
        Self {
            master_track: master_ref.0,
            master_event: master_ref.1,
            linked_track: linked_ref.0,
            linked_event: linked_ref.1,
            base_offset_sec,
        }
    }

    /// Calculates the signed out-of-sync drift in seconds given the current offset:
    /// `drift = current_offset_sec - base_offset_sec`.
    ///
    /// A positive drift indicates the linked event is later / delayed relative to baseline;
    /// negative indicates earlier.
    #[inline]
    pub fn calculate_out_of_sync(&self, current_offset_sec: f64) -> f64 {
        current_offset_sec - self.base_offset_sec
    }

    /// Calculates out-of-sync drift directly from current master and linked event states.
    pub fn calculate_drift(&self, master: &TrackEvent, linked: &TrackEvent) -> f64 {
        let current_offset = (linked.start - master.start) - (linked.trim_in - master.trim_in);
        self.calculate_out_of_sync(current_offset)
    }
}

/// Calculates the out-of-sync drift between two events created from the same source media file.
///
/// Returns `None` if the events do not share the same non-empty `media_id`.
pub fn calculate_media_out_of_sync(master: &TrackEvent, linked: &TrackEvent) -> Option<f64> {
    if master.media_id.is_none() || master.media_id != linked.media_id {
        return None;
    }
    let media_offset = linked.trim_in - master.trim_in;
    let timeline_offset = linked.start - master.start;
    Some(timeline_offset - media_offset)
}

/// Manager tracking one-way synchronization links across a project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncLinkManager {
    links: Vec<SyncLink>,
}

impl SyncLinkManager {
    /// Creates an empty sync link manager.
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// Returns a slice of all registered sync links.
    pub fn links(&self) -> &[SyncLink] {
        &self.links
    }

    /// Adds a sync link, replacing any preexisting link between the same pair.
    pub fn add_link(&mut self, link: SyncLink) {
        self.links.retain(|l| {
            !(l.master_track == link.master_track
                && l.master_event == link.master_event
                && l.linked_track == link.linked_track
                && l.linked_event == link.linked_event)
        });
        self.links.push(link);
    }

    /// Removes the sync link between `master` and `linked`.
    pub fn remove_link(&mut self, master: EventRef, linked: EventRef) -> bool {
        let before = self.links.len();
        self.links.retain(|l| {
            !(l.master_track == master.0
                && l.master_event == master.1
                && l.linked_track == linked.0
                && l.linked_event == linked.1)
        });
        self.links.len() < before
    }

    /// Removes all sync links where `event` is either master or dependent.
    pub fn remove_all_for_event(&mut self, event: EventRef) {
        self.links.retain(|l| {
            !(l.master_track == event.0 && l.master_event == event.1
                || l.linked_track == event.0 && l.linked_event == event.1)
        });
    }

    /// Returns all sync links where `master` is the master event.
    pub fn get_links_for_master(&self, master: EventRef) -> Vec<&SyncLink> {
        self.links
            .iter()
            .filter(|l| l.master_track == master.0 && l.master_event == master.1)
            .collect()
    }

    /// Returns the sync link where `dependent` is the linked event, if any.
    pub fn get_link_for_dependent(&self, dependent: EventRef) -> Option<&SyncLink> {
        self.links
            .iter()
            .find(|l| l.linked_track == dependent.0 && l.linked_event == dependent.1)
    }

    /// Returns true if `a` and `b` are linked in either direction.
    pub fn is_linked(&self, a: EventRef, b: EventRef) -> bool {
        self.links.iter().any(|l| {
            (l.master_track == a.0
                && l.master_event == a.1
                && l.linked_track == b.0
                && l.linked_event == b.1)
                || (l.master_track == b.0
                    && l.master_event == b.1
                    && l.linked_track == a.0
                    && l.linked_event == a.1)
        })
    }

    /// Calculates out-of-sync drift for a registered link against the timeline.
    pub fn calculate_drift(&self, timeline: &Timeline, link: &SyncLink) -> Option<f64> {
        let master = timeline
            .track(link.master_track)?
            .events
            .iter()
            .find(|e| e.id == link.master_event)?;
        let linked = timeline
            .track(link.linked_track)?
            .events
            .iter()
            .find(|e| e.id == link.linked_event)?;
        Some(link.calculate_drift(master, linked))
    }
}

/// Creates a resynchronization command by moving the linked event's timeline start position.
///
/// If `drift > 0` (linked event is later than baseline), `linked.start` is shifted backward by `drift`.
/// If `drift < 0` (linked event is earlier than baseline), `linked.start` is shifted forward.
pub fn resync_by_moving(
    timeline: &Timeline,
    master_ref: EventRef,
    linked_ref: EventRef,
    base_offset_sec: f64,
) -> Option<EditCommand> {
    let master = timeline
        .track(master_ref.0)?
        .events
        .iter()
        .find(|e| e.id == master_ref.1)?;
    let track = timeline.track(linked_ref.0)?;
    let linked = track.events.iter().find(|e| e.id == linked_ref.1)?;

    let current_offset = (linked.start - master.start) - (linked.trim_in - master.trim_in);
    let drift = current_offset - base_offset_sec;
    if drift.abs() <= TIME_EPSILON {
        return None;
    }

    let mut after = track.events.clone();
    let target = after.iter_mut().find(|e| e.id == linked_ref.1)?;
    target.start = (target.start - drift).max(0.0);

    Some(EditCommand::ReplaceEvents {
        changes: vec![TrackChange {
            track_id: linked_ref.0,
            before: track.events.clone(),
            after,
        }],
    })
}

/// Creates a resynchronization command by slipping the linked event's source in-point (`trim_in`).
///
/// The event's timeline position and duration remain completely unchanged; only the source media
/// offset (`trim_in`) is adjusted by `+drift` to restore alignment.
pub fn resync_by_slipping(
    timeline: &Timeline,
    master_ref: EventRef,
    linked_ref: EventRef,
    base_offset_sec: f64,
) -> Option<EditCommand> {
    let master = timeline
        .track(master_ref.0)?
        .events
        .iter()
        .find(|e| e.id == master_ref.1)?;
    let track = timeline.track(linked_ref.0)?;
    let linked = track.events.iter().find(|e| e.id == linked_ref.1)?;

    let current_offset = (linked.start - master.start) - (linked.trim_in - master.trim_in);
    let drift = current_offset - base_offset_sec;
    if drift.abs() <= TIME_EPSILON {
        return None;
    }

    let mut after = track.events.clone();
    let target = after.iter_mut().find(|e| e.id == linked_ref.1)?;
    target.trim_in = (target.trim_in + drift).max(0.0);

    Some(EditCommand::ReplaceEvents {
        changes: vec![TrackChange {
            track_id: linked_ref.0,
            before: track.events.clone(),
            after,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    fn test_timeline() -> (Timeline, EventRef, EventRef) {
        let mut tl = Timeline::new();
        let mut vtrack = tl.new_track(TrackKind::Video);
        let vid = tl.alloc_id();
        vtrack.events.push(TrackEvent {
            id: vid,
            name: "video".into(),
            media_id: Some(10),
            start: 2.0,
            duration: 5.0,
            trim_in: 1.0,
            ..Default::default()
        });
        let vref = (vtrack.id, vid);
        tl.tracks.push(vtrack);

        let mut atrack = tl.new_track(TrackKind::Audio);
        let aid = tl.alloc_id();
        atrack.events.push(TrackEvent {
            id: aid,
            name: "audio".into(),
            media_id: Some(10),
            start: 2.0,
            duration: 5.0,
            trim_in: 1.0,
            ..Default::default()
        });
        let aref = (atrack.id, aid);
        tl.tracks.push(atrack);

        (tl, vref, aref)
    }

    #[test]
    fn test_sync_link_drift_and_calculate_out_of_sync() {
        let (tl, vref, aref) = test_timeline();
        let master = &tl.tracks[0].events[0];
        let linked = &tl.tracks[1].events[0];

        let link = SyncLink::from_events(vref, master, aref, linked);
        assert_eq!(link.base_offset_sec, 0.0);
        assert_eq!(link.calculate_drift(master, linked), 0.0);

        // Drift when current_offset is 0.5s
        assert!((link.calculate_out_of_sync(0.5) - 0.5).abs() < 1e-9);
        assert!((link.calculate_out_of_sync(-0.25) - (-0.25)).abs() < 1e-9);
    }

    #[test]
    fn test_calculate_media_out_of_sync() {
        let (tl, _, _) = test_timeline();
        let master = tl.tracks[0].events[0].clone();
        let mut linked = tl.tracks[1].events[0].clone();

        // Aligned: drift is 0
        assert_eq!(calculate_media_out_of_sync(&master, &linked), Some(0.0));

        // Move linked audio forward by 1.2 seconds:
        linked.start = 3.2;
        assert!((calculate_media_out_of_sync(&master, &linked).unwrap() - 1.2).abs() < 1e-9);

        // Different media: returns None
        linked.media_id = Some(999);
        assert_eq!(calculate_media_out_of_sync(&master, &linked), None);
    }

    #[test]
    fn test_resync_by_moving() {
        let (mut tl, vref, aref) = test_timeline();
        // Shift audio forward by 0.8 seconds
        tl.tracks[1].events[0].start = 2.8;

        let cmd = resync_by_moving(&tl, vref, aref, 0.0).expect("command should be created");
        if let EditCommand::ReplaceEvents { changes } = cmd {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].track_id, aref.0);
            assert!((changes[0].after[0].start - 2.0).abs() < 1e-9);
            assert_eq!(changes[0].after[0].trim_in, 1.0);
        } else {
            panic!("expected ReplaceEvents");
        }
    }

    #[test]
    fn test_resync_by_slipping() {
        let (mut tl, vref, aref) = test_timeline();
        // Shift audio forward by 0.5 seconds on timeline
        tl.tracks[1].events[0].start = 2.5;

        let cmd = resync_by_slipping(&tl, vref, aref, 0.0).expect("command should be created");
        if let EditCommand::ReplaceEvents { changes } = cmd {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].track_id, aref.0);
            // Start remains untouched
            assert_eq!(changes[0].after[0].start, 2.5);
            // Source trim_in slipped by +0.5 to restore synchronization
            assert!((changes[0].after[0].trim_in - 1.5).abs() < 1e-9);
        } else {
            panic!("expected ReplaceEvents");
        }
    }
}
