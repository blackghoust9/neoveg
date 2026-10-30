//! Editing engine: timeline model and undoable edit commands.

pub mod aaf;
pub mod adjustment;
pub mod attributes;
pub mod autosave;
pub mod buses;
pub mod caption_scc;
pub mod captions;
pub mod command;
pub mod crossfade;
pub mod details;
pub mod edit;
pub mod edl;
pub mod effects;
pub mod envelope;
pub mod event_volume;
pub mod fcp7;
pub mod fcpxml;
pub mod groups;
pub mod keytrim;
pub mod markers;
pub mod mix;
pub mod navigation;
pub mod ops;
pub mod otio;
pub mod pan_crop;
pub mod project_file;
pub mod project_manager;
pub mod relink;
pub mod ripple;
pub mod selection;
pub mod slideshow;
pub mod snapping;
pub mod switches;
pub mod timeline;
pub mod track_motion;
pub mod track_ops;
pub mod transition_progress;
pub mod transitions;
pub mod trimmer;
pub mod velocity;
pub mod video_effects;

pub use captions::{Caption, CaptionId};
pub use command::{EditCommand, FxTarget, History, TrackChange, VideoFxTarget};
pub use envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
pub use markers::{MarkId, Marker, Marks, Region};
pub use selection::{EventRef, Selection, TimeRange};
pub use timeline::{
    AudioBus, BusId, BusOutput, EventGeometry, EventId, GroupId, ResampleMode, Timeline, Track,
    TrackDucking, TrackEffect, TrackEvent, TrackId, TrackKind, TrackMix, TrackSend,
};
