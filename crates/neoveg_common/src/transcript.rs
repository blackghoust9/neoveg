//! Speech transcript segments for text-based editing and subtitling.

use serde::{Deserialize, Serialize};

/// One timed subtitle segment produced by speech recognition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
}
