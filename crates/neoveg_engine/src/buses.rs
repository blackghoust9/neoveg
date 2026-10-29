//! Audio buses and sends (manual v21 p.869-873): up to 26 buses besides the master, a track or a bus
//! going to the master or to another bus, and sends that put a copy of a track's sound on a bus.
//!
//! Everything here is a pure function over the [`Timeline`]; each change comes back as one
//! [`EditCommand`] (one undo step). A route that would make a loop of buses is refused, a bus that
//! is deleted hands what pointed at it to the bus before it (or the master), and a reference to a
//! bus that does not exist is read as the master by whoever plays the project.

use crate::command::{EditCommand, Route, RouteChange};
use crate::timeline::{AudioBus, BusId, BusOutput, Timeline, TrackKind, TrackMix, TrackSend};

/// Most buses a project has, besides the master.
pub const MAX_BUSES: usize = 26;

/// Lowest send level: at or under it the send is silent.
pub const MIN_SEND_DB: f32 = -60.0;
/// Highest send level.
pub const MAX_SEND_DB: f32 = 12.0;

/// The letter of the bus at `index`: `A` for the first, `B` for the second.
pub fn letter(index: usize) -> char {
    char::from(b'A' + (index % MAX_BUSES) as u8)
}

/// What the bus is called: its own name, or the letter of its place in the list.
pub fn label(timeline: &Timeline, id: BusId) -> String {
    match timeline.buses.iter().position(|b| b.id == id) {
        Some(index) => {
            let bus = &timeline.buses[index];
            if bus.name.trim().is_empty() {
                format!("Bus {}", letter(index))
            } else {
                bus.name.clone()
            }
        }
        None => "Bus ausente".into(),
    }
}

/// `true` when `from` can go to `to` without making a loop of buses (a bus never goes to itself or
/// to one that, further on, goes back to it).
pub fn can_route_bus(timeline: &Timeline, from: BusId, to: BusOutput) -> bool {
    let mut next = to;
    for _ in 0..=timeline.buses.len() {
        match next {
            BusOutput::Master => return true,
            BusOutput::Bus(id) => {
                if id == from {
                    return false;
                }
                match timeline.buses.iter().find(|b| b.id == id) {
                    Some(bus) => next = bus.output,
                    // A bus that is gone ends the chain at the master
                    None => return true,
                }
            }
        }
    }
    false
}

/// The buses in the order they have to be processed: every bus after the buses that feed it.
/// Indices into `timeline.buses`. A bus that is part of a loop (a damaged file) goes last.
pub fn processing_order(timeline: &Timeline) -> Vec<usize> {
    let n = timeline.buses.len();
    let index_of = |id: BusId| timeline.buses.iter().position(|b| b.id == id);
    // How many buses send into each one
    let mut feeders = vec![0usize; n];
    for bus in &timeline.buses {
        if let BusOutput::Bus(to) = bus.output {
            if let Some(i) = index_of(to) {
                feeders[i] += 1;
            }
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut ready: Vec<usize> = (0..n).filter(|&i| feeders[i] == 0).collect();
    while let Some(i) = ready.pop() {
        order.push(i);
        if let BusOutput::Bus(to) = timeline.buses[i].output {
            if let Some(j) = index_of(to) {
                feeders[j] -= 1;
                if feeders[j] == 0 {
                    ready.push(j);
                }
            }
        }
    }
    // Anything left is in a loop
    let looped: Vec<usize> = (0..n).filter(|i| !order.contains(i)).collect();
    order.extend(looped);
    order
}

/// A new bus (the next free place, up to [`MAX_BUSES`]) with id `id`, or `None` when the project
/// already has all of them.
pub fn add_bus(timeline: &Timeline, id: BusId) -> Option<EditCommand> {
    if timeline.buses.len() >= MAX_BUSES {
        return None;
    }
    Some(EditCommand::AddBus {
        bus: AudioBus {
            id,
            name: String::new(),
            mix: TrackMix::default(),
            output: BusOutput::Master,
            effects: Vec::new(),
        },
        index: timeline.buses.len(),
    })
}

/// The bus before `index` (what a deleted bus hands its tracks to), or the master.
fn previous_output(timeline: &Timeline, index: usize) -> BusOutput {
    index
        .checked_sub(1)
        .and_then(|i| timeline.buses.get(i))
        .map_or(BusOutput::Master, |b| BusOutput::Bus(b.id))
}

/// Removes bus `id`. The tracks and buses that went to it now go to the bus before it (or the
/// master), and the sends to it are dropped; undoing puts all of it back.
pub fn remove_bus(timeline: &Timeline, id: BusId) -> Option<EditCommand> {
    let index = timeline.buses.iter().position(|b| b.id == id)?;
    let bus = timeline.buses[index].clone();
    let heir = previous_output(timeline, index);
    let mut routes = Vec::new();
    for track in &timeline.tracks {
        let output = if track.output == BusOutput::Bus(id) {
            heir
        } else {
            track.output
        };
        let sends: Vec<TrackSend> = track
            .sends
            .iter()
            .filter(|s| s.bus != id)
            .copied()
            .collect();
        if output != track.output || sends.len() != track.sends.len() {
            routes.push(RouteChange::Track {
                id: track.id,
                before: Route {
                    output: track.output,
                    sends: track.sends.clone(),
                },
                after: Route { output, sends },
            });
        }
    }
    // The buses that went to it follow the same rule, but never into a loop (the bus before it may
    // be the very bus that is being rerouted, or lead back to it): then they go to the master
    let mut rest = timeline.clone();
    rest.buses.retain(|b| b.id != id);
    for other in &timeline.buses {
        if other.output == BusOutput::Bus(id) {
            let output = if can_route_bus(&rest, other.id, heir) {
                heir
            } else {
                BusOutput::Master
            };
            if let Some(b) = rest.buses.iter_mut().find(|b| b.id == other.id) {
                b.output = output;
            }
            routes.push(RouteChange::Bus {
                id: other.id,
                before: other.output,
                after: output,
            });
        }
    }
    Some(EditCommand::RemoveBus { bus, index, routes })
}

/// Puts `after` in place of the bus with the same id (its name, fader, effects or output), or
/// `None` when nothing changes, the bus does not exist or the output would make a loop. The fader
/// and the pan are brought into range.
pub fn set_bus(timeline: &Timeline, mut after: AudioBus) -> Option<EditCommand> {
    let before = timeline.buses.iter().find(|b| b.id == after.id)?.clone();
    after.mix.volume_db = if after.mix.volume_db.is_finite() {
        after.mix.volume_db.clamp(-60.0, 12.0)
    } else {
        0.0
    };
    after.mix.pan = if after.mix.pan.is_finite() {
        after.mix.pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if !can_route_bus(timeline, after.id, after.output) {
        return None;
    }
    (before != after).then_some(EditCommand::SetBus { before, after })
}

/// Sets where audio track `track_id` goes and which sends it has, or `None` when nothing changes,
/// the track is not an audio track or it points at a bus that does not exist. A send keeps one
/// entry per bus and its level inside [`MIN_SEND_DB`]..=[`MAX_SEND_DB`].
pub fn set_track_route(
    timeline: &Timeline,
    track_id: u64,
    output: BusOutput,
    sends: Vec<TrackSend>,
) -> Option<EditCommand> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Audio {
        return None;
    }
    let exists = |id: BusId| timeline.buses.iter().any(|b| b.id == id);
    if let BusOutput::Bus(id) = output {
        if !exists(id) {
            return None;
        }
    }
    let mut kept: Vec<TrackSend> = Vec::new();
    for mut send in sends {
        if !exists(send.bus) || kept.iter().any(|s| s.bus == send.bus) {
            continue;
        }
        send.level_db = if send.level_db.is_finite() {
            send.level_db.clamp(MIN_SEND_DB, MAX_SEND_DB)
        } else {
            0.0
        };
        kept.push(send);
    }
    let before = Route {
        output: track.output,
        sends: track.sends.clone(),
    };
    let after = Route {
        output,
        sends: kept,
    };
    (before != after).then_some(EditCommand::SetRouting {
        changes: vec![RouteChange::Track {
            id: track_id,
            before,
            after,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    /// A timeline with `buses` buses (ids 100, 101, ...) and two audio tracks.
    fn project(buses: usize) -> (Timeline, History, Vec<u64>) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let mut tracks = Vec::new();
        for _ in 0..2 {
            let t = tl.new_track(TrackKind::Audio);
            tracks.push(t.id);
            assert!(h.execute(&mut tl, EditCommand::AddTrack { track: t }));
        }
        for i in 0..buses {
            let cmd = add_bus(&tl, 100 + i as u64).unwrap();
            assert!(h.execute(&mut tl, cmd));
        }
        (tl, h, tracks)
    }

    fn send(bus: u64, level: f32) -> TrackSend {
        TrackSend {
            bus,
            level_db: level,
            post_volume: false,
            link_pan: false,
        }
    }

    #[test]
    fn a_project_has_up_to_twenty_six_buses_named_by_letter_or_by_the_user() {
        let (mut tl, mut h, _) = project(MAX_BUSES);
        assert!(add_bus(&tl, 999).is_none(), "the 27th is refused");
        assert_eq!(label(&tl, 100), "Bus A");
        assert_eq!(label(&tl, 100 + 25), "Bus Z");
        assert_eq!(letter(3), 'D');
        let mut named = tl.buses[1].clone();
        named.name = "Bateria".into();
        let cmd = set_bus(&tl, named).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(label(&tl, 101), "Bateria");
        // An emptied name goes back to the letter
        let mut cleared = tl.buses[1].clone();
        cleared.name.clear();
        let cmd = set_bus(&tl, cleared).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(label(&tl, 101), "Bus B");
        assert_eq!(label(&tl, 5), "Bus ausente");
    }

    #[test]
    fn a_track_goes_to_a_bus_and_has_sends_each_change_one_undo_step() {
        let (mut tl, mut h, tracks) = project(2);
        let cmd = set_track_route(
            &tl,
            tracks[0],
            BusOutput::Bus(100),
            vec![
                send(101, -6.0),
                send(101, 3.0),
                send(555, 0.0),
                send(100, 40.0),
            ],
        )
        .unwrap();
        assert!(h.execute(&mut tl, cmd));
        let t = tl.track(tracks[0]).unwrap();
        assert_eq!(t.output, BusOutput::Bus(100));
        // One entry per bus, none for a bus that does not exist, the level in range
        assert_eq!(t.sends.len(), 2);
        assert_eq!(t.sends[0], send(101, -6.0));
        assert_eq!(t.sends[1].level_db, MAX_SEND_DB);
        assert!(h.undo(&mut tl));
        let t = tl.track(tracks[0]).unwrap();
        assert!(t.output.is_master() && t.sends.is_empty());
        assert!(h.redo(&mut tl));
        assert_eq!(tl.track(tracks[0]).unwrap().sends.len(), 2);
        // Nothing to change, a bus that is not there and a video track are all refused
        let same = tl.track(tracks[0]).unwrap().clone();
        assert!(set_track_route(&tl, tracks[0], same.output, same.sends).is_none());
        assert!(set_track_route(&tl, tracks[0], BusOutput::Bus(777), vec![]).is_none());
        let video = tl.new_track(TrackKind::Video);
        let vid = video.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: video }));
        assert!(set_track_route(&tl, vid, BusOutput::Master, vec![send(100, 0.0)]).is_none());
    }

    #[test]
    fn buses_cannot_make_a_loop_and_are_processed_after_what_feeds_them() {
        let (mut tl, mut h, _) = project(3);
        // A -> B -> C -> master
        for (from, to) in [(100, 101), (101, 102)] {
            let mut bus = tl.buses.iter().find(|b| b.id == from).unwrap().clone();
            bus.output = BusOutput::Bus(to);
            let cmd = set_bus(&tl, bus).unwrap();
            assert!(h.execute(&mut tl, cmd));
        }
        let mut c = tl.buses[2].clone();
        c.output = BusOutput::Bus(100);
        assert!(
            set_bus(&tl, c.clone()).is_none(),
            "C -> A would close the loop"
        );
        c.output = BusOutput::Bus(102);
        assert!(set_bus(&tl, c).is_none(), "a bus never goes to itself");
        assert!(!can_route_bus(&tl, 102, BusOutput::Bus(100)));
        assert!(can_route_bus(&tl, 100, BusOutput::Master));
        let order = processing_order(&tl);
        let at = |i: usize| order.iter().position(|&x| x == i).unwrap();
        assert!(
            at(0) < at(1) && at(1) < at(2),
            "A, then B, then C: {order:?}"
        );
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn deleting_a_bus_hands_its_tracks_to_the_bus_before_and_undo_restores_everything() {
        let (mut tl, mut h, tracks) = project(4);
        // Track 0 goes to D (the 4th) and sends to D and to B; bus C goes to D
        let cmd = set_track_route(
            &tl,
            tracks[0],
            BusOutput::Bus(103),
            vec![send(103, -3.0), send(101, 0.0)],
        )
        .unwrap();
        assert!(h.execute(&mut tl, cmd));
        let mut c = tl.buses[2].clone();
        c.output = BusOutput::Bus(103);
        let cmd = set_bus(&tl, c).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let before = tl.clone();
        let cmd = remove_bus(&tl, 103).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(tl.buses.len(), 3);
        let t = tl.track(tracks[0]).unwrap();
        assert_eq!(t.output, BusOutput::Bus(102), "D's tracks go to C");
        assert_eq!(t.sends, vec![send(101, 0.0)], "the send to D is gone");
        assert_eq!(
            tl.buses[2].output,
            BusOutput::Master,
            "D's heir is C itself, so C goes to the master instead of to itself"
        );
        assert!(h.undo(&mut tl));
        assert_eq!(tl, before);
        assert!(remove_bus(&tl, 9999).is_none());
        // The first bus hands its tracks to the master
        let (mut tl, mut h, tracks) = project(2);
        let cmd = set_track_route(&tl, tracks[1], BusOutput::Bus(100), vec![]).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let cmd = remove_bus(&tl, 100).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert!(tl.track(tracks[1]).unwrap().output.is_master());
    }

    #[test]
    fn the_buses_and_the_routes_survive_saving_and_old_projects_have_none() {
        let (mut tl, mut h, tracks) = project(2);
        let mut bus = tl.buses[0].clone();
        bus.mix.volume_db = -6.0;
        bus.name = "Vozes".into();
        let cmd = set_bus(&tl, bus).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let cmd =
            set_track_route(&tl, tracks[0], BusOutput::Bus(100), vec![send(101, -9.0)]).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.buses, tl.buses);
        assert_eq!(
            back.track(tracks[0]).unwrap().sends,
            tl.track(tracks[0]).unwrap().sends
        );
        assert_eq!(back.track(tracks[0]).unwrap().output, BusOutput::Bus(100));
        // Nothing of it is written for a project that does not use it
        let plain = serde_json::to_string(&Timeline::new()).unwrap();
        assert!(!plain.contains("buses") && !plain.contains("sends") && !plain.contains("output"));
        let old: Timeline = serde_json::from_str(&plain).unwrap();
        assert!(old.buses.is_empty());
        // A fader out of range is brought back
        let mut wild = back.buses[0].clone();
        wild.mix.volume_db = 99.0;
        wild.mix.pan = -5.0;
        let Some(EditCommand::SetBus { after, .. }) = set_bus(&back, wild) else {
            panic!("a set command");
        };
        assert_eq!((after.mix.volume_db, after.mix.pan), (12.0, -1.0));
    }
}
