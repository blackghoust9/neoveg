//! Background frame decoding. The UI sends requests and polls replies; it never decodes video itself.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use neoveg_common::chroma::ChromaKey;
use neoveg_common::grade::Grade;

use crate::frame::FrameBuffer;
use crate::info::VideoInfo;
use crate::lut::Lut;

pub struct FrameRequest {
    pub media_id: u64,
    pub path: PathBuf,
    /// Time inside the source media.
    pub seconds: f64,
    /// Maximum decoded width (0 = source size).
    pub max_width: u32,
    /// Color table applied to the decoded picture (done on the worker thread).
    pub lut: Option<Arc<Lut>>,
    /// Primary color correction, applied before the LUT.
    pub grade: Grade,
    /// Chroma key (green/blue screen), applied before primary grading and LUT.
    pub chroma_key: ChromaKey,
}

pub struct FrameReply {
    pub media_id: u64,
    pub seconds: f64,
    pub result: Result<FrameBuffer, String>,
}

/// Reads video metadata (needs the `ffmpeg` feature).
pub fn probe(path: &Path) -> Result<VideoInfo, String> {
    #[cfg(feature = "ffmpeg")]
    {
        crate::decode::probe(path)
    }
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = path;
        Err(DISABLED.into())
    }
}

#[cfg(not(feature = "ffmpeg"))]
const DISABLED: &str =
    "decodificação de vídeo desabilitada: compile com `--features ffmpeg` (veja o README)";

/// Handle to the decoding thread.
pub struct FrameWorker {
    tx: Sender<FrameRequest>,
    pub rx: Receiver<FrameReply>,
}

impl FrameWorker {
    /// Starts the thread. `on_reply` runs after every reply (use it to request a repaint).
    pub fn spawn(on_reply: impl Fn() + Send + 'static) -> Result<Self, String> {
        #[cfg(not(feature = "ffmpeg"))]
        {
            let _ = on_reply;
            Err(DISABLED.into())
        }
        #[cfg(feature = "ffmpeg")]
        {
            use std::sync::mpsc::channel;
            let (tx, requests) = channel::<FrameRequest>();
            let (replies, rx) = channel::<FrameReply>();
            std::thread::Builder::new()
                .name("neoveg-video-decode".into())
                .spawn(move || run(requests, replies, on_reply))
                .map_err(|e| format!("cannot start video thread: {e}"))?;
            Ok(Self { tx, rx })
        }
    }

    pub fn request(&self, request: FrameRequest) {
        let _ = self.tx.send(request);
    }
}

#[cfg(feature = "ffmpeg")]
fn run(requests: Receiver<FrameRequest>, replies: Sender<FrameReply>, on_reply: impl Fn()) {
    use crate::decode::VideoDecoder;
    use std::collections::HashMap;

    const MAX_OPEN_DECODERS: usize = 4;
    let mut decoders: HashMap<PathBuf, VideoDecoder> = HashMap::new();

    while let Ok(mut request) = requests.recv() {
        // Scrubbing floods the queue: only the newest request is worth decoding
        while let Ok(newer) = requests.try_recv() {
            request = newer;
        }

        let result = (|| {
            if !decoders.contains_key(&request.path) {
                if decoders.len() >= MAX_OPEN_DECODERS {
                    decoders.clear();
                }
                decoders.insert(request.path.clone(), VideoDecoder::open(&request.path)?);
            }
            let decoder = decoders
                .get_mut(&request.path)
                .ok_or("decoder unavailable")?;
            let mut frame = decoder.frame_at(request.seconds, request.max_width)?;
            crate::chroma::apply_chroma_key(&mut frame, &request.chroma_key);
            crate::grading::apply_look(&mut frame, &request.grade, request.lut.as_deref());
            Ok::<_, String>(frame)
        })();
        if let Err(e) = &result {
            tracing::warn!(
                "[WARN] video decode failed for {}: {e}",
                request.path.display()
            );
            decoders.remove(&request.path);
        }

        let reply = FrameReply {
            media_id: request.media_id,
            seconds: request.seconds,
            result,
        };
        if replies.send(reply).is_err() {
            break;
        }
        on_reply();
    }
}

#[cfg(all(test, not(feature = "ffmpeg")))]
mod tests {
    use super::*;

    #[test]
    fn without_ffmpeg_the_worker_explains_itself() {
        assert!(FrameWorker::spawn(|| {}).is_err());
        assert!(probe(Path::new("x.mp4")).unwrap_err().contains("ffmpeg"));
    }
}

#[cfg(all(test, feature = "ffmpeg"))]
mod ffmpeg_tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn worker_decodes_requested_frames_off_thread() {
        let path = std::env::temp_dir().join(format!("neoveg_worker_{}.mp4", std::process::id()));
        let made = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=320x240:rate=25")
            .args(["-c:v", "mpeg4"])
            .arg(&path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !made {
            return;
        }

        let worker = FrameWorker::spawn(|| {}).unwrap();
        worker.request(FrameRequest {
            media_id: 7,
            path: path.clone(),
            seconds: 0.4,
            max_width: 160,
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
        });
        let reply = worker.rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(reply.media_id, 7);
        let frame = reply.result.unwrap();
        assert_eq!((frame.width, frame.height), (160, 120));

        worker.request(FrameRequest {
            media_id: 7,
            path: PathBuf::from("missing.mp4"),
            seconds: 0.0,
            max_width: 0,
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
        });
        assert!(worker
            .rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .is_err());
        let _ = std::fs::remove_file(path);
    }
}
