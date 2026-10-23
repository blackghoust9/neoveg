//! Editing engine: timeline model and undoable edit commands.

pub mod command;
pub mod timeline;

pub use command::{EditCommand, History};
pub use timeline::{EventId, Timeline, Track, TrackEvent, TrackId, TrackKind};
