//! Editing engine: timeline model and undoable edit commands.

pub mod autosave;
pub mod command;
pub mod edit;
pub mod project_file;
pub mod relink;
pub mod timeline;

pub use command::{EditCommand, History};
pub use timeline::{EventGeometry, EventId, Timeline, Track, TrackEvent, TrackId, TrackKind};
