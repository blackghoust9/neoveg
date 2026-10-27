//! Editing engine: timeline model and undoable edit commands.

pub mod autosave;
pub mod captions;
pub mod command;
pub mod crossfade;
pub mod details;
pub mod edit;
pub mod edl;
pub mod effects;
pub mod envelope;
pub mod fcp7;
pub mod fcpxml;
pub mod groups;
pub mod markers;
pub mod mix;
pub mod navigation;
pub mod ops;
pub mod otio;
pub mod project_file;
pub mod project_manager;
pub mod relink;
pub mod ripple;
pub mod selection;
pub mod slideshow;
pub mod switches;
pub mod timeline;
pub mod trimmer;

pub use captions::{Caption, CaptionId};
pub use command::{EditCommand, FxTarget, History, TrackChange};
pub use envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
pub use markers::{MarkId, Marker, Marks, Region};
pub use selection::{EventRef, Selection, TimeRange};
pub use timeline::{
    EventGeometry, EventId, GroupId, Timeline, Track, TrackDucking, TrackEffect, TrackEvent,
    TrackId, TrackKind, TrackMix,
};
