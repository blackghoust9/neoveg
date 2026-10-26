//! Editing engine: timeline model and undoable edit commands.

pub mod autosave;
pub mod captions;
pub mod command;
pub mod crossfade;
pub mod edit;
pub mod edl;
pub mod effects;
pub mod groups;
pub mod markers;
pub mod ops;
pub mod project_file;
pub mod relink;
pub mod selection;
pub mod timeline;

pub use captions::{Caption, CaptionId};
pub use command::{EditCommand, FxTarget, History, TrackChange};
pub use markers::{MarkId, Marker, Marks, Region};
pub use selection::{EventRef, Selection, TimeRange};
pub use timeline::{
    EventGeometry, EventId, GroupId, Timeline, Track, TrackDucking, TrackEffect, TrackEvent,
    TrackId, TrackKind,
};
