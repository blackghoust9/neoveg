//! Editing engine: timeline model and undoable edit commands.

pub mod aaf;
pub mod adjustment;
pub mod attributes;
pub mod autosave;
pub mod buses;
pub mod cache_ruler;
pub mod caption_mcc;
pub mod caption_scc;
pub mod captions;
pub mod command;
pub mod command_markers;
pub mod compositing;
pub mod crossfade;
pub mod details;
pub mod dual_mono;
pub mod edit;
pub mod edl;
pub mod effects;
pub mod empty_event;
pub mod envelope;
pub mod event_pitch;
pub mod event_volume;
pub mod explorer;
pub mod fcp7;
pub mod fcpxml;
pub mod groups;
pub mod keyedit;
pub mod keytrim;
pub mod make_movie;
pub mod markers;
pub mod media_bins;
pub mod media_pool;
pub mod media_props;
pub mod mix;
pub mod navigation;
pub mod ops;
pub mod otio;
pub mod pan_crop;
pub mod prerender;
pub mod project_file;
pub mod project_manager;
pub mod proxy;
pub mod relink;
pub mod render_track;
pub mod ripple;
pub mod selection;
pub mod slideshow;
pub mod smooth_cut;
pub mod snapping;
pub mod subclip;
pub mod subtitles;
pub mod switches;
pub mod takes;
pub mod test_fixtures;
pub mod text_edit;
pub mod timeline;
pub mod track_arm;
pub mod track_lock;
pub mod track_motion;
pub mod track_ops;
pub mod track_tree;
pub mod transition_progress;
pub mod transitions;
pub mod trim_monitor;
pub mod trimmer;
pub mod velocity;
pub mod video_effects;

pub use captions::{Caption, CaptionId};
pub use command::{EditCommand, FxTarget, History, TrackChange, VideoFxTarget};
pub use envelope::{
    AutomationMode, AutomationTarget, Envelope, EnvelopePoint, EnvelopeShape, TrackEnvelope,
};
pub use markers::{MarkId, Marker, Marks, Region};
pub use selection::{EventRef, Selection, TimeRange};
pub use timeline::{
    AudioBus, BusId, BusOutput, EventGeometry, EventId, GroupId, ResampleMode, Timeline, Track,
    TrackDucking, TrackEffect, TrackEvent, TrackId, TrackKind, TrackMix, TrackSend,
};
