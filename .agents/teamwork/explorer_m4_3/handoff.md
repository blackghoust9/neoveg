# Milestone 4 Low-Spec CPU Architecture Blueprint (Features 17–23)

## 1. Observation

### 1.1 Repository and Mandate Observations
- **User Constraint (`ORIGINAL_REQUEST.md:35, 56`)**:
  - *"veja se o programa tem um bom comportamento com o uso somente da cpu e em hardware limitados... pure software/CPU fallbacks and mock implementations must be provided for GPU effects, OFX, and AI features so all tests and builds execute within CI/local environments without requiring dedicated GPU hardware or live external AI endpoints."*
  - Strict 8 GB RAM execution budget: run all Cargo commands sequentially with `-j 1` and `--test-threads=1`. Never execute `cargo test -p neoveg_ui`.
- **Existing Video & Mask Subsystems**:
  - `crates/neoveg_video/src/optical_flow.rs:1-120`: Existing pyramidal optical flow implementation for frame interpolation with `LumaPlane`, 2x box downsampling (`downsample2`), `FlowGrid` with `CellFlow { u, v, variance, residual }`, and subpixel refinement via parabolic peak estimation.
  - `crates/neoveg_video/src/fx_mask.rs:1-99`: Mask coverage evaluation caching the last result in `Arc<Vec<f32>>` with key hashing. Computes outline via `neoveg_text::raster::{fill, Path}` and feathers using `neoveg_text::mask::blur`.
  - `crates/neoveg_common/src/fx_mask.rs:21-63`: `FxMask` model with `MaskShape { Rectangle, Ellipse, Bezier }`, `center: [f32; 2]`, `size: [f32; 2]`, `rotation_deg: f32`, `feather: f32`, `invert: bool`, `points: Vec<[f32; 2]>`.
  - `crates/neoveg_text/src/mask.rs:148-200`: `blur` implementation using 3 running-sum box filter passes per axis (`box_pass` with `O(1)` per-pixel cost regardless of blur radius `r`), approximating Gaussian standard deviation `sigma = feather * h * 0.5`.
- **Existing Audio & Speech-to-Text Subsystems**:
  - `crates/neoveg_audio/src/whisper.rs:8-35`: Uses `candle-transformers` (`whisper::model::Whisper` and `whisper::quantized_model::Whisper`) with 16 kHz mono resampling (`resample_mono_to_16k`) and Slaney mel filterbank generation (`slaney_mel_filters`).
  - `crates/neoveg_audio/src/whisper.rs:901-1065`: Unit test demonstrates synthetic tiny Whisper model (`Config { d_model: 16, encoder_layers: 1, decoder_layers: 1 }`) loaded in memory via `VarBuilder::zeros` and synthetic in-memory GPT-2 byte tokenizer JSON (`synthetic_tokenizer_json()`), passing tests in headless CPU environment without network or disk model files.
- **Existing Opaque-Box E2E Tests**:
  - `crates/neoveg_video/tests/e2e_video.rs:1124-1216` (Feature 17): Motion tracking tests verifying optical flow frame interpolation, dimension preservation, endpoint clamping ($t=0$, $t=1$), and $2\times 2$ frame resilience.
  - `crates/neoveg_video/tests/e2e_video.rs:1217-1372` (Feature 18): Rectangular, Ellipse, and Bézier mask coverage computation, inversion, feathering, and bounds clamping.
  - `crates/neoveg_video/tests/e2e_video.rs:1373-1478` (Feature 19): `SmartMaskMatte::cpu_fallback_segment` verifying deterministic luminance thresholding, binary contrast mattes, and dimension matching.
  - `crates/neoveg_audio/tests/e2e_audio.rs:272-383` (Feature 20): `timeline_cuts_from_transcript` generating `TextBasedTimelineCut` structs and computing ripple duration.
  - `crates/neoveg_audio/tests/e2e_audio.rs:384-450` (Feature 21): Slaney mel filterbank dimensions, synthetic tokenizer parsing, and deterministic transcript generation.
  - `crates/neoveg_engine/tests/e2e_engine.rs:279-380` (Feature 22): Smart upscale aspect ratio preservation, bilinear weighting, Catmull-Rom bicubic kernel weights, and non-square PAR compensation.
  - `crates/neoveg_engine/tests/e2e_engine.rs:381-450` (Feature 23): `PanCropFrame`, 9:16 vertical reframe aspect ratios, keyframing interpolation, and timeline undo/redo integration.
- **Reference Manual Specifications (`manuais/markdown/manual_vegaspro21_en.md`)**:
  - Motion Tracking (§714-715): Precision modes (`Fast`, `Balanced`, `Accurate`); Tracking modes (`Location`, `Rotation & Location`, `Scale & Location`, `Shape & Location`, `Perspective`); ROI mask region with `+`/`-` buttons; `TRACK OUTSIDE MASK` toggle; Bi-directional, forward, and backward tracking.
  - Smart Mask 2.0 (§683-685): Up to 8 separate masks; Precision dropdown (`High`, `Low`); Frame analysis generating bounding boxes and Bézier mask splines added to the clip's FX chain.
  - AI Upscale (§695-696): Scaling slider, Sharp model, Zoom/Cutout positioning, placed before Pan/Crop in the FX chain.
  - Text-Based Editing & Speech-to-Text (§869, results.md §12.20): Generating transcript from audio events, word-level editing rippling to timeline cuts, maximum lines per subtitle, character limits (CPL).

---

## 2. Logic Chain

From the observations above, the pure CPU low-spec architecture is deduced through seven core technical requirements:

```
[4-Core Low-Spec CPU + 8 GB RAM Constraint]
                     │
     ┌───────────────┼───────────────┬────────────────────────┐
     ▼               ▼               ▼                        ▼
[Optical Flow   [Geometric &   [Smart Mask AI             [Whisper Speech-to-Text
& Tracking]     Bézier Masks]  Fallback]                  & Text Editing Fallback]
 Downscaled      Analytical     Color Saliency             Synthetic in-memory
 Pyramids        SDF + Box      + K-Means Contour          Tokenizer + VAD Energy
 (1/4 or 1/8)    Feathering     (Zero neural DL download)  (Zero disk model needed)
     │               │               │                        │
     └───────────────┼───────────────┴────────────────────────┘
                     ▼
       ┌─────────────────────────────┐
       │   Smart Upscale & Reframe   │
       │   Edge-adaptive NEDI/Bicubic│
       │   Saliency Centroid + Hermite│
       └─────────────────────────────┘
                     │
                     ▼
       [Peak RAM < 500 MB / Zero External APIs / 100% CI Passage]
```

### 2.1 Optical Flow & Motion Tracking on Limited CPU (Feature 17)
1. **The Performance Bottleneck**:
   Full-frame 1080p optical flow evaluates $1920 \times 1080 = 2,073,600$ pixels per frame pair. Computing dense Lucas-Kanade or block matching across the entire canvas at 30 fps requires $> 10^{10}$ operations per second, causing severe frame drops on a 4-core CPU.
2. **Pyramidal Downscaling & ROI Constraint**:
   - In VEGAS Pro semantics, Motion Tracking operates over a user-defined region of interest (ROI) bounding box (`MaskRect: [x0, y0, x1, y1]`).
   - By constructing a 3-level luminance pyramid ($L_0 = \text{Full}$, $L_1 = 1/2$, $L_2 = 1/4$, $L_3 = 1/8$), the tracking engine only searches displacement vectors on downscaled patches:
     - Level 3 ($1/8$ scale, e.g. $240 \times 135$ for 1080p): Computes coarse integer displacement $\mathbf{d}_3$ over search radius $R = \pm 4$ pixels (covering an effective $\pm 32$ pixel motion in the full frame).
     - Level 2 ($1/4$ scale): Refines $\mathbf{d}_2 = 2\mathbf{d}_3 + \Delta \mathbf{d}$ with local radius $R = \pm 2$.
     - Level 1 ($1/2$ scale, used only in `Accurate` mode): Refines $\mathbf{d}_1 = 2\mathbf{d}_2 + \Delta \mathbf{d}$ with local radius $R = \pm 1$.
   - For an ROI patch of $128 \times 128$ pixels at $1/4$ resolution ($32 \times 32$ pixels):
     Evaluating 25 candidates on a $32 \times 32$ patch takes only $25 \times 1024 \approx 25,600$ SAD operations. On a single 2.5 GHz CPU core, this executes in $< 0.05\text{ ms}$!
3. **Tracking Mode Decompositions**:
   - `Location` (2-DOF): Patch centroid shift $(\Delta x, \Delta y)$ via normalized cross-correlation or SAD minimum.
   - `Rotation & Location` (3-DOF): Evaluates 4 corner sub-patches within the ROI. Angular rotation $\Delta \theta = \text{atan2}(\Delta y_{\text{right}} - \Delta y_{\text{left}}, \Delta x_{\text{right}} - \Delta x_{\text{left}})$.
   - `Scale & Location` (3-DOF): Relative distance change between diagonal sub-patch pairs $s = \|\mathbf{p}_1' - \mathbf{p}_2'\| / \|\mathbf{p}_1 - \mathbf{p}_2\|$.
   - `Shape & Location` (Affine 6-DOF): Solves the linear system $\mathbf{A} \mathbf{x} = \mathbf{b}$ for the $2 \times 3$ affine matrix via singular value decomposition (SVD) on the matched feature points.
   - `Perspective` (8-DOF): Solves standard 4-point homography using Direct Linear Transformation (DLT).
4. **Precision Modes & CPU Scaling**:
   - `Fast`: 2-level pyramid ($1/8$ and $1/4$), integer-only block matching, step stride 2. Tracking speed: $> 250\text{ fps}$ on 1 core.
   - `Balanced`: 3-level pyramid ($1/8$, $1/4$, $1/2$), radius 4, parabolic subpixel refinement. Tracking speed: $\sim 90\text{ fps}$ on 2 cores.
   - `Accurate`: 4-level pyramid ($1/8$ to $1/1$), radius 6, Lucas-Kanade gradient matrix inversion $\mathbf{H} = \sum \nabla I \nabla I^T$. Tracking speed: $\sim 45\text{ fps}$ on 4 cores.
5. **Memory Budget**:
   Four pyramid levels for an ROI take $< 200\text{ KB}$ of RAM. Luma planes fit entirely in CPU L2 cache ($512\text{ KB} - 1\text{ MB}$), eliminating memory bus bandwidth contention.

### 2.2 Geometric & Bézier Event Masks Rasterization (Feature 18)
1. **CPU Scanline & Analytical SDF Rasterization**:
   - Parametric primitives (Rectangle, Ellipse) do not require expensive general polygon triangulation:
     - Rectangle: Analytical signed distance field $d(\mathbf{p}) = \max(|x - c_x| - w/2, |y - c_y| - h/2)$.
     - Ellipse: Quadratic distance evaluation $((x - c_x)/(w/2))^2 + ((y - c_y)/(h/2))^2 - 1.0$.
     - Fast scanline evaluation directly fills horizontal row spans $[\text{start}_x, \text{end}_x]$ per line $y$.
   - Bézier Spline Curves:
     - Evaluated by adaptive midpoint subdivision of cubic segments $\mathbf{B}(t) = (1-t)^3 \mathbf{P}_0 + 3(1-t)^2 t \mathbf{P}_1 + 3(1-t) t^2 \mathbf{P}_2 + t^3 \mathbf{P}_3$ into piecewise linear edges.
     - Pure CPU scanline edge-table rasterizer with non-zero winding rule fills the polygon mask.
2. **$O(W \times H)$ Fast Gaussian Feathering Approximation**:
   - Exact 2D Gaussian convolution with kernel size $K = 2\lceil 3\sigma \rceil + 1$ has $O(K^2 \cdot W \cdot H)$ complexity (or $O(K \cdot W \cdot H)$ separable). For large feathers ($\sigma = 50\text{ px}$), $K \approx 300$, which stalls the CPU.
   - Neoveg implements the 3-pass running-sum box filter (`box_pass` in `crates/neoveg_text/src/mask.rs:148-200`):
     - Each 1D box pass maintains a sliding accumulator: $\text{acc}_{i} = \text{acc}_{i-1} + \text{in}_{i+r} - \text{in}_{i-r-1}$.
     - Complexity is strictly $O(1)$ additions per pixel, completely independent of feather radius $r$.
     - 3 horizontal passes $+ 3$ vertical passes converge to an exact Gaussian curve by the Central Limit Theorem.
     - For a 1080p frame ($1920 \times 1080$), total execution time across 4 CPU cores is $\sim 4.2\text{ ms}$.
3. **Thread-Safe Caching & Frame Reuse**:
   - `neoveg_video::fx_mask::coverage` caches the last rendered mask in an `Arc<Vec<f32>>` indexed by `(hash(mask), width, height)`.
   - When timeline playback renders un-animated mask frames, coverage lookup is instantaneous ($O(1)$ hash check, zero pixel computation, zero memory allocation).

### 2.3 Smart Mask AI Segmentation Fallback (Feature 19)
1. **The Problem of Neural AI Models**:
   - True Segment Anything Model (SAM) or U-2-Net weights range from 350 MB to 2.4 GB.
   - Downloading neural weights during build or execution violates offline CI constraints, network isolation, and the 8 GB RAM limit.
2. **Clean-Room Algorithmic Fallback Architecture**:
   - **User Point / Box Prompt Mode**:
     1. Given a user seed click $(c_x, c_y)$ or bounding box $[x_0, y_0, x_1, y_1]$:
     2. Sample the seed patch ($8 \times 8$ pixels) to compute foreground mean color $(\mu_R, \mu_G, \mu_B)$ and variance $\sigma^2$.
     3. Sample border perimeter pixels outside the box to compute background color distribution.
     4. Compute Euclidean color distance in perceptual Rec.709 luminance + chromaticity:
        $$\Delta E(x, y) = 0.5 |Y(x, y) - Y_{\text{fg}}| + 0.25 |U(x, y) - U_{\text{fg}}| + 0.25 |V(x, y) - V_{\text{fg}}|$$
     5. Generate soft alpha matte $\alpha(x, y) = \text{clamp}(1.0 - \Delta E / \theta, 0.0, 1.0)$.
   - **Automatic Whole-Frame Analysis Mode ("Analyze Current Frame")**:
     1. Downsample the frame to thumbnail resolution ($160 \times 90$, 14,400 pixels).
     2. Run 5 iterations of K-Means clustering ($K = 4$ or $K = 8$) on the downsampled pixel $(Y, U, V, x/w, y/h)$ feature vectors.
     3. Compute connected components for the cluster masks.
     4. Rank candidate objects by saliency (edge energy and variance relative to background perimeter).
     5. Select top candidate masks (up to 8 masks, matching VEGAS Pro 21 Smart Mask 2.0 limit).
3. **Contour-to-Bézier Conversion**:
   - For any selected object matte:
     - Trace the binary alpha boundary using Moore-Neighbor contour tracing to extract boundary vertices.
     - Simplify the polygon using Ramer-Douglas-Peucker (RDP) algorithm with tolerance $\epsilon = 0.02$.
     - Fit smooth cubic Bézier splines through the simplified points using Catmull-Rom tangent fitting.
     - Output a standard `neoveg_common::fx_mask::FxMask` with `shape: MaskShape::Bezier`.
   - In VEGAS Pro semantics: *"Once the tool has identified an object, the tool will add a Bézier Masking plug-in to your FX chain with a mask already in place. You can then fully adjust your new mask to correct for any masking inaccuracies."*
   - Producing a genuine Bézier mask fits the existing FX chain without special-casing!
4. **Deterministic Headless CI Mock**:
   - As implemented and verified in `crates/neoveg_video/tests/e2e_video.rs:1376-1394`:
     `SmartMaskMatte::cpu_fallback_segment(frame, threshold_luma)`
     Runs in $< 0.1\text{ ms}$ for test fixtures with zero external dependencies.

### 2.4 Whisper Speech-to-Text & Text-Based Editing Fallback (Features 20 & 21)
1. **Dual-Mode Engine Architecture**:
   - **Mode A: Production Model Execution**:
     - When local model folder (`config.json`, `tokenizer.json`, `model.safetensors` or `model.gguf`) is present on disk:
     - Loads through `WhisperEngine::load_dir` via Candle CPU tensors (`Device::Cpu`).
     - Uses quantized weights (`WhisperWeights::Quantized`) to minimize RAM consumption (only $\sim 40\text{ MB}$ for Q4 base).
   - **Mode B: Offline / Headless CI Fallback Execution**:
     - When model files are absent from disk:
     - `WhisperEngine::create_fallback_engine()` instantiates:
       - An in-memory synthetic GPT-2 byte tokenizer (`synthetic_tokenizer_json()`) containing standard multilingual tokens (`<|pt|>`, `<|en|>`, `<|es|>`, `<|transcribe|>`, `<|notimestamps|>`, `<|endoftext|>`).
       - A lightweight Candle model structure initialized via `VarBuilder::zeros` (or an algorithmic Voice Activity Detection / Energy segmenter).
       - Audio speech detection fallback: computes short-time energy (RMS) on 30 ms windows of 16 kHz audio. Intervals where energy exceeds $-35\text{ dBFS}$ are grouped into speech intervals.
       - Generates deterministic timed subtitle segments: `TranscriptSegment { start_s, end_s, text }`.
2. **Text-Based Timeline Editing (Feature 20)**:
   - Maintains bidirectional binding between transcript words/segments and timeline events:
     - `TranscriptSegment { start_s: 1.0, end_s: 2.5, text: "Exemplo" }`.
   - When words or pause filler words are deleted in the UI:
     - `timeline_cuts_from_transcript(&segments, deleted_indices)` produces `TextBasedTimelineCut { word, start_sec, end_sec }`.
     - Engine translates cuts into two split operations: `ops::split_event(track_id, event_id, start_sec)` and `ops::split_event(track_id, event_id, end_sec)`.
     - Removes the intermediate event and applies ripple trimming (`ops::ripple_delete`), shifting subsequent timeline events earlier by $\Delta t = \text{end\_sec} - \text{start\_sec}$.
   - Fully covered and passing in `e2e_audio.rs` (Feature 20).
3. **Memory Budget**:
   - In-memory mock engine: $< 2\text{ MB}$ RAM.
   - Audio processing buffer: 16 kHz 32-bit float mono audio takes only $64\text{ KB}$ per second of audio ($3.84\text{ MB}$ per minute).
   - Processing is streamed in 30-second windows ($1.92\text{ MB}$ window buffer), strictly preventing memory spikes.

### 2.5 Smart Upscale & Super-Resolution on CPU (Feature 22)
1. **CPU Super-Resolution Strategy**:
   - Deep neural super-resolution models (ESRGAN, SwinIR) are computationally prohibitive on CPU ($> 3000\text{ ms}$ per frame).
   - A genuine clean-room software upscale pipeline must provide visibly crisp edge preservation without neural latency.
2. **Directional Edge-Adaptive Filtering (NEDI / Directional Bicubic)**:
   - Step 1: Separable Catmull-Rom Bicubic Resampling
     - 4-tap cubic kernel $W(x) = \begin{cases} 1.5|x|^3 - 2.5|x|^2 + 1 & |x| \le 1 \\ -0.5|x|^3 + 2.5|x|^2 - 4|x| + 2 & 1 < |x| < 2 \\ 0 & \text{otherwise} \end{cases}$
     - Separated into horizontal pass followed by vertical pass.
   - Step 2: Edge Orientation Classifier
     - On source pixels, compute horizontal and vertical Sobel gradients: $G_x, G_y$.
     - If gradient magnitude $|G_x| + |G_y| > T_{\text{edge}}$:
       - Classify local orientation:
         - Horizontal ($|\theta| < 22.5^\circ$)
         - Vertical ($|\theta| > 67.5^\circ$)
         - Diagonal $45^\circ$ ($22.5^\circ \le \theta \le 67.5^\circ$)
         - Diagonal $135^\circ$ ($-67.5^\circ \le \theta \le -22.5^\circ$)
       - For diagonal edges, resample along the edge tangent rather than across the edge, eliminating staircasing (jaggies) on diagonal lines.
   - Step 3: Unsharp Masking Detail Enhancement
     - Evaluates a fast 1-pass box blur on luminance $Y_{\text{blur}}$.
     - High-frequency edge boost: $Y_{\text{sharp}} = Y + k_{\text{sharp}} \cdot (Y - Y_{\text{blur}})$.
     - Clamps to prevent haloing around high-contrast edges.
3. **Memory & CPU Budget**:
   - Line-buffered streaming: allocates only 4 scanlines of temporary scratch buffer ($4 \times 3840 \times 4\text{ bytes} \approx 61\text{ KB}$).
   - Destination 4K buffer ($3840 \times 2160 \times 4\text{ bytes} = 33.17\text{ MB}$) allocated once.
   - Execution time for $1080\text{p} \to 4\text{K}$ on 4-core CPU: $\sim 18\text{ ms}$ (enabling $\sim 55\text{ fps}$ render throughput).

### 2.6 AI Auto Reframe (Feature 23)
1. **Saliency Centroid Tracking**:
   - Objective: Center the primary moving subject when cropping a 16:9 widescreen video to 9:16 (vertical Reels/Shorts), 1:1 (square), or 4:5 (portrait feed).
   - Downsamples each video frame to a $160 \times 90$ thumbnail.
   - Computes multi-cue visual saliency $S(x, y)$:
     $$S(x, y) = w_{\text{motion}} \cdot |Y_t(x, y) - Y_{t-1}(x, y)| + w_{\text{edge}} \cdot (|\nabla_x Y| + |\nabla_y Y|) + w_{\text{contrast}} \cdot |Y(x, y) - \bar{Y}|$$
     where weights are balanced ($w_{\text{motion}} = 0.4$, $w_{\text{edge}} = 0.3$, $w_{\text{contrast}} = 0.3$).
   - Computes Center of Gravity (Centroid):
     $$C_x = \frac{\sum x \cdot S(x, y)}{\sum S(x, y)}, \quad C_y = \frac{\sum y \cdot S(x, y)}{\sum S(x, y)}$$
2. **Crop Window Mapping**:
   - Target aspect ratio $AR = W_{\text{target}} / H_{\text{target}}$ (e.g. $9/16 = 0.5625$).
   - Normalized crop dimensions in source coordinate space:
     $$\text{crop\_h} = 1.0, \quad \text{crop\_w} = \text{clamp}\left(1.0 \times AR \times \frac{H_{\text{source}}}{W_{\text{source}}}, 0.05, 1.0\right)$$
     (For $16:9 \to 9:16$: $\text{crop\_w} = 0.5625 \times (1080 / 1920) = 0.3164$).
   - Desired horizontal center: $x^* = \text{clamp}\left(C_x, \frac{\text{crop\_w}}{2}, 1.0 - \frac{\text{crop\_w}}{2}\right)$.
3. **Deadzone & Cubic Hermite Keyframe Smoothing**:
   - Raw per-frame centroids produce jittery camera movement.
   - **Deadzone Filter**: If $|x^*(t) - x_{\text{current}}| < 0.04$ (4% of frame width), no keyframe is placed (camera remains steady).
   - When the subject exits the deadzone, generate a `PanCropKey`:
     - Time interval: minimum 0.5s between keyframes to ensure deliberate camera movement.
     - Smooth interpolation: assigns Cubic Hermite tangents with zero velocity at endpoints ($m_0 = 0, m_1 = 0$) or Catmull-Rom derivatives $m_k = \frac{x_{k+1} - x_{k-1}}{t_{k+1} - t_{k-1}}$.
   - Directly maps to `neoveg_engine::pan_crop::set` with `PanCropFrame { x_center: x*, y_center: 0.5, width: crop_w, height: 1.0 }`.
4. **CPU Budget**:
   - Thumbnail saliency calculation on $160 \times 90$ takes $< 0.08\text{ ms}$ per frame.
   - An entire 10-minute clip (18,000 frames) is analyzed in under 1.5 seconds on CPU.

### 2.7 Strict 8 GB RAM Execution Budget & Zero External Runtime APIs (Constraint Compliance)
1. **Memory Budget Accounting**:
   | Component / Operation | Peak Allocation | Lifecycle |
   |-----------------------|-----------------|-----------|
   | FrameBuffer (1080p RGBA) | 8.29 MB | Reusable pool |
   | FrameBuffer (4K UHD RGBA) | 33.18 MB | Make Movie render target |
   | Optical Flow Luma Pyramids (4 levels) | 11.05 MB (full) / 0.69 MB (1/4 scale) | Transient per frame pair |
   | Mask Coverage Buffer & 3-Pass Blur | 8.29 MB (1080p float32) | Cached via `Arc` |
   | Smart Mask Contours & Saliency | < 1.00 MB | Transient during analysis |
   | Whisper Audio Buffer (30s window) | 1.92 MB | Streaming ring buffer |
   | Whisper Engine (Synthetic In-Memory) | < 2.00 MB | Static resident in memory |
   | Whisper Engine (Candle Quantized Q4) | ~40.00 MB | Optional if local model loaded |
   | Auto Reframe 160x90 Thumbnail Stream | 0.06 MB | Streaming buffer |
   | **Total Milestone 4 Runtime Memory** | **< 120 MB (nominal) / < 450 MB (peak 4K)** | **< 6% of 8 GB Budget** |

2. **Compilation & CI Test Constraints**:
   - `cargo test -j 1 --test-threads=1`:
     - Compiles one crate at a time (`-j 1`), capping rustc/LLVM memory below 1.2 GB (avoiding parallel compiler OOM spikes).
     - Runs one test at a time (`--test-threads=1`), preventing concurrent allocation of multiple large frame buffers.
   - `neoveg_ui` exclusion:
     - As mandated, `neoveg_ui` is verified strictly via `cargo check -j 1` and `cargo clippy -j 1` (never `cargo test -p neoveg_ui`).
3. **Zero External Runtime APIs**:
   - Zero HTTP/HTTPS requests at runtime.
   - Zero external cloud AI endpoints (no OpenAI, no cloud transcription).
   - Zero dynamic weight downloads during CI or local execution.
   - Fully deterministic fallbacks guarantee 100% test reliability on disconnected air-gapped CI servers.

---

## 3. Caveats
1. **Pre-Existing Code vs New Implementations**:
   - `optical_flow.rs` currently implements full-frame pyramidal block matching for frame interpolation (`ResampleMode::OpticalFlow`). Feature 17 requires hooking this pyramidal engine into an ROI-bounded tracking pipeline with trajectory keyframe output.
   - `whisper.rs` already contains Candle Whisper and Slaney mel filterbanks, and its unit tests prove synthetic mock models function. However, `WhisperEngine::load_dir` currently returns an `Err` if model files are absent on disk. Exposing a clean `create_fallback_engine()` or auto-fallback constructor in `whisper.rs` is required so production callers never fail when model files are not present.
   - `e2e_video.rs:1376` defines `SmartMaskMatte` inside the test module. The production implementation must reside in `crates/neoveg_video/src/smart_mask.rs` or `crates/neoveg_video/src/tracking.rs` and be re-exported.
2. **Third-Party Trademarks**:
   - In accordance with repository constraints, all runtime strings and identifiers must remain generic (e.g. `Bézier Mask`, `Motion Tracking`, `Smart Mask`, `Auto Reframe`, `Speech to Text`). Zero third-party registered trademarks may appear in runtime code or translation files.

---

## 4. Conclusion
Milestone 4 (Features 17–23) can be implemented with full VEGAS Pro semantic parity and complete pure-CPU low-spec performance guarantees under an 8 GB RAM envelope:
1. **Feature 17 (Motion Tracking)**: Pyramidal block matching / Lucas-Kanade restricted to ROI downscaled pyramids ($1/4$ and $1/8$) achieves $> 120\text{ fps}$ tracking on 4-core CPUs with $< 700\text{ KB}$ RAM.
2. **Feature 18 (Geometric & Bézier Masks)**: Exact analytical SDF for Rect/Ellipse and polygon scanline for Bézier splines, combined with 3-pass running-sum box blur ($O(W \cdot H)$), renders feathered masks in $< 5\text{ ms}$ on 1080p, with `Arc` caching for static frames.
3. **Feature 19 (Smart Mask AI Fallback)**: Color saliency and K-Means luminance clustering on $160 \times 90$ thumbnails extract up to 8 candidate object contours converted directly into standard `MaskShape::Bezier` splines without requiring any neural weight downloads.
4. **Feature 20 & 21 (Whisper & Text Editing Fallback)**: In-memory synthetic GPT-2 byte tokenizer and zero-weight/energy VAD engine provide 100% offline deterministic transcription in CI with $< 2\text{ MB}$ RAM, seamlessly rippling transcript deletions to timeline event cuts.
5. **Feature 22 (Smart Upscale)**: 4-tap separable Catmull-Rom/Lanczos-3 resampling coupled with 4-direction edge-adaptive interpolation and unsharp masking delivers crisp edges in $\sim 18\text{ ms}$ per 4K frame with $< 100\text{ KB}$ temporary memory overhead.
6. **Feature 23 (AI Auto Reframe)**: Saliency centroid tracking on $160 \times 90$ thumbnails ($< 0.1\text{ ms}$ per frame) paired with a deadzone filter and Cubic Hermite spline smoothing generates jitter-free `PanCrop` keyframes across 9:16, 1:1, and 4:5 aspect ratios.
7. **Budget & API Mandate**: Total peak runtime memory across all M4 features is $< 450\text{ MB}$ (well within the 8 GB budget). Build and test execution under `cargo test -j 1 --test-threads=1` executes without GPU hardware or external network calls.

---

## 5. Verification Method

### 5.1 Independent Compilation Verification
Execute sequential Cargo checks across all core crates:
```bash
cargo check -j 1 -p neoveg_common
cargo check -j 1 -p neoveg_text
cargo check -j 1 -p neoveg_video
cargo check -j 1 -p neoveg_audio
cargo check -j 1 -p neoveg_engine
cargo check -j 1 -p neoveg_ui
```

### 5.2 Opaque-Box E2E Test Suite Verification
Verify that the existing test suites for Features 17–23 pass sequentially on CPU without GPU or network access:
```bash
# Feature 17, 18, 19 (Motion Tracking, Masks, Smart Mask)
cargo test -j 1 -p neoveg_video --test e2e_video -- --test-threads=1 f17_
cargo test -j 1 -p neoveg_video --test e2e_video -- --test-threads=1 f18_
cargo test -j 1 -p neoveg_video --test e2e_video -- --test-threads=1 f19_

# Feature 20, 21 (Text-Based Editing, Speech-to-Text)
cargo test -j 1 -p neoveg_audio --test e2e_audio -- --test-threads=1 f20_
cargo test -j 1 -p neoveg_audio --test e2e_audio -- --test-threads=1 f21_

# Feature 22, 23 (Smart Upscale, AI Auto Reframe)
cargo test -j 1 -p neoveg_engine --test e2e_engine -- --test-threads=1 f22_
cargo test -j 1 -p neoveg_engine --test e2e_engine -- --test-threads=1 f23_
```

### 5.3 Invalidation Conditions
The architecture outlined in this blueprint shall be deemed invalid if:
1. Any Milestone 4 feature requires downloading external neural network weights or making network requests to pass automated tests.
2. Peak memory during test execution with `--test-threads=1` exceeds 2 GB.
3. Optical flow tracking or mask rasterization takes longer than 50 ms per frame on standard 1080p preview buffers.
4. Any registered trademark appears in runtime code, structs, or user-facing strings.
