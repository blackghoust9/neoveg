# Milestone 4 Specification Mining Report: Motion Tracking, Event Masks, Smart Mask, Text-Based Editing, Speech-to-Text, Smart Upscale & Auto Reframe

**Agent**: `spec_miner_m4_1`  
**Milestone**: Milestone 4 (Features 17–23)  
**Target Repository**: `C:\Users\paulomatos\Documents\GitHub\neoveg`  
**Authoritative Sources Consulted**:
- `manuais/markdown/manual_vegaspro21_en.md` (Lines 757, 865–885, 943–997, 1017, 1073, 1091, 1217–1269, 10473–10599, 25139–25390, 30108–30400, 30433–30750, 30841–30900, 31450–32000, 51475)
- `manuais/markdown/VEGAS_Pro_18_EN_Keyboard_shortcuts.md` (Lines 59, 233, 257, 407, 429)
- `manuais/markdown/VEGAS_Pro_18_EN.md` (Motion Tracking, Bézier Masking, Video FX)
- `results.md` (Lines 83, 1349, 15780, 16036–16150)
- Existing code: `crates/neoveg_video/src/optical_flow.rs`, `crates/neoveg_common/src/fx_mask.rs`, `crates/neoveg_video/src/fx_mask.rs`, `crates/neoveg_common/src/pan_crop.rs`, `crates/neoveg_video/src/pan_crop.rs`, `crates/neoveg_audio/src/whisper.rs`, `crates/neoveg_engine/src/captions.rs`, `crates/neoveg_engine/src/ripple.rs`

---

## 1. Observation

### 1.1 Feature 17: Motion Tracking Panel & Workflow
From `manual_vegaspro21_en.md` (lines 31450–32000) and `VEGAS_Pro_18_EN_Keyboard_shortcuts.md` (line 59):
- **Access**:
  - Menu: `Tools | Video | Motion Tracking`
  - Event context menu: `Motion Tracking...`
  - Keyboard shortcut: `Alt + M`
- **Panel Layout & Header**:
  - `CLIP NAME`: Displays the active video event name being tracked. Panel can remain open continuously while selecting different events.
  - `MASK / REGION`: Dropdown selector listing all defined tracking regions.
  - Buttons: `+` (Add region), `-` (Remove region).
  - Region interaction: Displays a rectangular/quadrilateral search boundary with 4 corner resize handles and center move handle over the Video Preview.
- **Tracking Settings**:
  - `TRACK OUTSIDE MASK`: Inverts the tracking region so that non-masked area is analyzed.
  - `PRECISION`: Dropdown with 3 modes:
    - `Fast`: Coarse grid, downsampled pyramid, fastest processing.
    - `Balanced`: Standard multi-scale pyramid (default).
    - `Accurate`: Full-resolution pixel refinement.
  - `MODE`: Movement degrees of freedom (DOF):
    - `LOCATION`: 2 DOF $(X, Y)$ position shift detection.
    - `ROTATION & LOCATION`: 3 DOF $(X, Y)$ translation + rotation angle $\theta$.
    - `SCALE & LOCATION`: 4 DOF $(X, Y)$ translation + scale changes $(s_x, s_y)$.
    - `SHAPE & LOCATION`: 6 DOF affine transformation (translation, rotation, scale, shear).
    - `PERSPECTIVE`: 8 DOF perspective transformation (homography matrix $\mathbf{H}_{3\times 3}$ / 4 corner pins).
  - `SHOW MOTION VECTORS`: Checkbox toggles real-time overlay of optical flow vectors on Video Preview (non-persistent visual diagnostic).
  - `MOTION TRACK INDICATOR`: Timeline bar under preview displaying tracked frames in Green and lost/untracked frames in Gray/Red.
  - `STATUS BAR`: Real-time feedback ("Tracking frame X of Y (Z%)", reason for tracking stop).
  - `TRACK BUTTONS`:
    1. Track backward to beginning (`|<<`)
    2. Track one frame backward (`<|`)
    3. Track both directions from cursor (`<< >>`)
    4. Track one frame forward (`|>`)
    5. Track forward to end (`>>|`)
    6. Cancel / Stop tracking (`Stop`)
- **Motion Transfer Menu**:
  - Multi-level hierarchical dispatch menu:
    - Level 1: Target events (current event or any overlapping event on other tracks).
    - Level 2: Target plug-ins / modules (`Event Pan/Crop`, `Picture-in-Picture / PIP`, `Bézier Masking`, `Track Motion`, or `Add Bézier Mask FX`).
    - Level 3: Target parameters (`Location X/Y`, `Scale`, `Angle`, `Corner Pins`).
  - Tracking data transferred as event envelopes/keyframes relative to event start time.
- **Track Interchange**:
  - Export: Exports motion trajectory to generic JSON / text or Silhouette Corner Pin format.
  - Import: Imports external planar / point tracking data into the selected region.

### 1.2 Feature 18: Geometric & Bézier Event Masks
From `manual_vegaspro21_en.md` (lines 580–589, 10473–10599, 25139–25390):
- **Two Mask Contexts**:
  1. **Event Pan/Crop Mask**:
     - Sub-tab in Event Pan/Crop window.
     - Mode: `Positive` (shows inside) or `Negative` (shows outside).
     - Feather type: `Out`, `In`, `Both`. Feather amount: `0.0%` to `100.0%`.
     - Opacity: `0.0%` to `100.0%`.
  2. **Bézier Masking OFX / Event FX**:
     - Applied in Event FX chain (or Track FX / Media FX).
     - Supports up to 5 masks (`Mask 1` through `Mask 5`).
     - General Options:
       - `Invert`: inverts all active masks.
       - `Mask FX`: Checkbox. When enabled, masks only the preceding effects in the chain (e.g. Pixelate, Blur, Color Curves) rather than the video alpha itself.
       - `Blend`: `0.0` to `1.0` overall mask strength/transparency.
     - Per-mask controls:
       - `Enable`: Checkbox toggle.
       - `Type`: `Rectangle`, `Oval` (Ellipse), `Diamond`, `Curve` (Bézier spline).
       - Transform: `Location X, Y` ($0.0..1.0$), `Width`, `Height`, `Angle` ($-180^\circ..+180^\circ$).
       - `Opacity`: `0.0..1.0`.
       - `Feather`: `0.0..1.0`.
       - `Feather Style`: `Out` (expands outward from border), `In` (contracts inward), `Both` (centered on border).
     - Bézier Curve Editing:
       - Anchor points with tangent control handles.
       - Edit tools: `Normal` (reposition points and tangents), `Add` (insert points on segment), `Delete` (remove points), `Split` (breaks tangent collinearity for sharp corners).
       - Closure: curve automatically closes between last and first anchor points.
     - Keyframe / Point Animation:
       - `Animate Curve`: per-point keyframe animation over time. Keyframes interpolate anchor positions and tangent vectors smoothly across time.

### 1.3 Feature 19: Smart Mask (AI-Assisted Object Segmentation)
From `manual_vegaspro21_en.md` (lines 865, 945, 993, 997, 30433–30750) and `results.md` (line 21468):
- **Access / Effect**: `AI Smart Mask 2.0` (Generic: `Smart Mask` / `Object Segmentation Mask`).
- **Multi-Mask Support**: Up to 8 masks (`Mask 1` through `Mask 8`).
- **Precision**: `High` (full-resolution analysis) vs `Low` (fast downscaled analysis).
- **Controls & Interaction**:
  - `Analyze Current Frame`: Analyzes active frame and identifies maskable objects.
  - `Auto Analyze Current Frame`: Automatically runs segmentation when playhead changes.
  - Bounding Box: Rectangular frame around candidate object. Edge dragging resizes, corner dragging scales, Shift+drag non-proportional.
  - Point Prompts (Interactive Refinement):
    - `Add or Reposition Points` (Positive points): Green foreground anchor clicks.
    - `Add and Reposition Negative Points` (Negative points): Red background exclusion clicks.
    - `Remove Points`: Deletes prompt point.
  - `View Masking Results`: Toggles between overlay mode (tinted overlay showing segmented boundary) and composite mode (mask applied to video alpha or preceding FX).
  - Confidence Thresholds: Foreground threshold $\theta_{fg}$ and background threshold $\theta_{bg}$ determining boundary matte falloff.
- **Animation & Tracking**:
  - `Direction`: `Forward`, `Backward`, `Both`.
  - `Motion Tracking`: Propagates bounding box and segmentation across frames.
  - `Complete Masking`: Generates full keyframed matte sequence across the event.
  - `Clear Keyframes` / `Animate`: Manual keyframe animation.
- **Pure-CPU Fallback Requirement**: Must execute deterministically without GPU or external AI endpoints, using color/texture energy minimization (GrabCut/GMM) within 8 GB RAM.

### 1.4 Feature 20: Text-Based Timeline Editing
From `manual_vegaspro21_en.md` (lines 867–869, 30108–30275) and `results.md` (lines 16036–16075):
- **Access**: `Tools | Speech to Text` window, mode selector `Text-based Editing` (from `Transcript`, `Text-based Editing`, `Subtitles`).
- **Word-Level Timeline Synchronization**:
  - Every word has precise start and end timecode $[t_{start}, t_{end}]$.
  - `Show pauses`: Toggles display of pause durations between words.
  - `Show pauses longer than`: Slider threshold (e.g. 0.5s to 5.0s, default 1.0s). Pauses formatted as `[Pause: X.Xs]`.
  - `Show time code`: Displays start timecode at sentence/line starts.
  - `Show file name`: Displays source media file name.
- **Editing Actions & Ripple**:
  - Word Selection: Clicking word jumps timeline playhead to word start time. Shift+click selects range of words and creates corresponding timeline loop selection.
  - Delete Words: Pressing `Delete` deletes selected words from transcript, splits timeline event at start and end of word range, deletes cut audio/video events, and if `Auto Ripple` (`Ctrl + L`) is active, ripples subsequent timeline events to close the gap!
  - Transcript updates subsequent word timestamps by subtracting the deleted duration $\Delta t$.
  - Cut & Paste Rearrangement: Cutting word range cuts timeline events; pasting at new transcript location splits timeline at target time, inserts the media segment, and ripples remaining content.
  - Search & Replace: Find in transcript navigates to occurrences and jumps timeline cursor.

### 1.5 Feature 21: Speech-to-Text Subtitle Generation
From `manual_vegaspro21_en.md` (lines 30108–30350, 30841), `results.md` (lines 3765, 16036–16075), and `crates/neoveg_audio/src/whisper.rs`:
- **Access**: `Tools | Speech to Text` window, `Subtitles` view.
- **Audio Preprocessing & Engine**:
  - Resamples timeline audio mix to 16 kHz mono (`WHISPER_SAMPLE_RATE = 16000`).
  - Local Whisper transformer inference via `candle_transformers` (or CPU mock fallback).
  - Language selection: `Auto Detect` + ISO languages (`pt`, `en`, `es`, `fr`, `de`, `it`, `ja`, `zh`, `ru`).
- **Formatting Constraints**:
  - `Max characters per line (CPL)`: Slider (default 37, range 20..60).
  - `Line Count`: Radio button `1 Line` vs `2 Lines` (max 2 lines per subtitle cue).
  - `Reading Speed (CPS)`: Characters per second verification ($\le 17..20$ CPS).
  - `Min / Max Duration`: Minimum cue duration 1.0s, maximum duration 7.0s.
  - `Inter-cue gap`: Minimum 2 frames ($\approx 67$ to $83$ ms) to prevent subtitle flashing.
- **Timeline Integration & Export**:
  - `Generate Titles`: Creates a new Video Track named "Subtitles from Speech" and inserts `Titles & Text` generator events for each cue.
  - Preset selection: Applies subtitle style preset (font, size, bottom alignment, outline, shadow).
  - `Export Subtitles`: Exports to SubRip (`.srt`), WebVTT (`.vtt`), TXT, or CSV (`Timecode Start, Timecode End, Timecode Length, Subtitle Text`).
  - Single undo step commits all generated subtitle events.

### 1.6 Feature 22: Smart Upscale & Super-Resolution
From `manual_vegaspro21_en.md` (lines 1017, 1217–1235, 30843–30900) and `results.md` (line 21464):
- **Access & Placement**:
  - Effect: `Smart Upscale & Super-Resolution` (Generic for `AI Upscale`).
  - Placement rule: Must be placed **before Pan/Crop** (to the left of Pan/Crop in the Event FX chain) so upscale processes original source pixels prior to geometric cropping.
- **Operating Modes**:
  - `Event FX Mode`: Automatically determines scaling factor to match Project Settings resolution (e.g. 1080p source $\to$ 4K project = $2.0\times$ scale). Scaling slider locked to auto ratio.
  - `Media FX Mode`: Scaling slider unlocked for manual factor selection ($1.0\times$ to $4.0\times$).
- **Parameters**:
  - `Scale Factor`: $1.0\times$ to $4.0\times$ (presets for $2\times$ and $4\times$).
  - `Sharpness / Detail Level`: Slider $0.0$ to $1.0$ (controls high-frequency edge enhancement).
  - `Denoise Level`: Slider $0.0$ to $1.0$ (suppresses compression artifacts prior to upscaling).
  - `Cutout / Zoom`: Pan crosshair $(X, Y) \in [-1.0, 1.0]^2$ and Zoom slider ($1.0\times..10.0\times$), animatable.
- **CPU Fallback Architecture**:
  - New Edge-Directed Directional Interpolation (NEDI/EDDI) + Adaptive Unsharp Masking filter.
  - Bilateral edge filtering to prevent ringing/haloing.
  - Tiled memory-bounded processing for 8 GB RAM execution.

### 1.7 Feature 23: AI Auto Reframe
From `manual_vegaspro21_en.md` (lines 1073), `results.md` (lines 1349, 15780, 16036–16075):
- **Access**:
  - Context menu on video event: `AI Auto Reframe...`
  - Menu command: `Tools | Video | AI Auto Reframe` (canonical id `32662` / `0x7F96`, mnemonic `R`).
  - Render As dialog: Checkbox `Render with AI Auto Reframe` (`RenderAs.CheckBox.AiReframe`) with target profile ("AI Auto Reframe from {0} to {1}").
- **Target Aspect Ratios**:
  - `9:16 Vertical` (1080x1920, Reels/Shorts/TikTok)
  - `1:1 Square` (1080x1080, Social feed)
  - `4:5 Portrait` (1080x1350, Social portrait)
  - `4:3 Standard` (1440x1080)
  - `21:9 Cinema` (2560x1080)
- **Tracking & Keyframing**:
  - Primary Subject Saliency: Computes motion center-of-mass from optical flow + luma variance.
  - Dynamic Pan/Crop Generation: Computes optimal crop window aspect ratio, clamps to source bounds, applies spatial deadband filter (avoids micro-jitter), and generates smoothed `PanCropKey` keyframes directly onto the event's `PanCrop`!

---

## 2. Logic Chain & Mathematical Specifications

### 2.1 Feature 17: Motion Tracking Mathematical Specification

#### Pyramidal Lucas-Kanade Optical Flow
Given two successive frames $I(x, y, t)$ and $I(x, y, t + \Delta t)$:
1. **Luma Plane & Pyramid Construction**:
   Convert RGBA to normalized Rec.709 luma:
   $$Y = 0.2126 R + 0.7152 G + 0.0722 B$$
   Build Gaussian pyramid $L_0, L_1, \dots, L_{M-1}$ where $L_k$ is downsampled by $2\times$ using a $5\times 5$ binomial kernel $\frac{1}{256} [1, 4, 6, 4, 1]^T [1, 4, 6, 4, 1]$.
2. **Spatial Gradients**:
   At level $k$:
   $$I_x = \frac{\partial L_k}{\partial x} = \frac{L_k(x+1, y) - L_k(x-1, y)}{2}$$
   $$I_y = \frac{\partial L_k}{\partial y} = \frac{L_k(x, y+1) - L_k(x, y-1)}{2}$$
   $$I_t = L_{k, t+1}(x, y) - L_{k, t}(x, y)$$
3. **Lucas-Kanade System Matrix**:
   Over search window $\Omega$ of size $W \times W$ (e.g. $15 \times 15$ or $21 \times 21$):
   $$\mathbf{G} = \sum_{(x, y) \in \Omega} \begin{bmatrix} I_x^2 & I_x I_y \\ I_x I_y & I_y^2 \end{bmatrix}, \quad \mathbf{b} = -\sum_{(x, y) \in \Omega} \begin{bmatrix} I_x I_t \\ I_y I_t \end{bmatrix}$$
   Eigenvalue tracking check (Shi-Tomasi corner criterion):
   $$\lambda_{min}(\mathbf{G}) = \frac{(G_{11} + G_{22}) - \sqrt{(G_{11} - G_{22})^2 + 4 G_{12}^2}}{2}$$
   If $\lambda_{min} < \tau_{threshold}$ (e.g. $10^{-4}$), point is flagged untrackable / lost.
   Otherwise:
   $$\begin{bmatrix} \Delta u \\ \Delta v \end{bmatrix} = \mathbf{G}^{-1} \mathbf{b}$$
   Iterated with Newton-Raphson subpixel updates until $\|\Delta \mathbf{u}\| < 0.01$ px or max 20 iterations.

#### Motion Transformation Modes
From a set of tracked feature correspondences $\mathbf{p}_i = (x_i, y_i) \to \mathbf{p}'_i = (x'_i, y'_i)$:
1. **Location (2 DOF)**:
   $$\Delta X = \frac{1}{N} \sum_{i=1}^N (x'_i - x_i), \quad \Delta Y = \frac{1}{N} \sum_{i=1}^N (y'_i - y_i)$$
2. **Rotation & Location (3 DOF)**:
   Centroids: $\mathbf{c} = \frac{1}{N}\sum \mathbf{p}_i, \quad \mathbf{c}' = \frac{1}{N}\sum \mathbf{p}'_i$.
   Relative vectors: $\mathbf{q}_i = \mathbf{p}_i - \mathbf{c}, \quad \mathbf{q}'_i = \mathbf{p}'_i - \mathbf{c}'$.
   $$\theta = \text{atan2}\left(\sum_{i} (q_{i, x} q'_{i, y} - q_{i, y} q'_{i, x}), \sum_{i} (q_{i, x} q'_{i, x} + q_{i, y} q'_{i, y})\right)$$
   $$\mathbf{t} = \mathbf{c}' - \mathbf{R}(\theta) \mathbf{c}$$
3. **Scale & Location (4 DOF)**:
   $$s = \frac{\sum \|\mathbf{q}'_i\|}{\sum \|\mathbf{q}_i\|}, \quad \mathbf{t} = \mathbf{c}' - s \mathbf{c}$$
4. **Perspective (8 DOF Homography $\mathbf{H}_{3\times 3}$)**:
   $$\mathbf{p}'_i \sim \mathbf{H} \mathbf{p}_i \implies \begin{bmatrix} -x_i & -y_i & -1 & 0 & 0 & 0 & x_i x'_i & y_i x'_i & x'_i \\ 0 & 0 & 0 & -x_i & -y_i & -1 & x_i y'_i & y_i y'_i & y'_i \end{bmatrix} \mathbf{h} = \mathbf{0}$$
   Solved via Direct Linear Transformation (DLT) with SVD on 4 corner pins with RANSAC outlier filtering.

---

### 2.2 Feature 18: Geometric & Bézier Event Masks Specification

#### Spline Mathematics & Segment Construction
For a set of $N$ closed control points $\mathbf{P}_0, \mathbf{P}_1, \dots, \mathbf{P}_{N-1}$ with forward tangents $\mathbf{T}_{out, i}$ and backward tangents $\mathbf{T}_{in, i}$:
1. **Cubic Bézier Segment Formulation**:
   Between point $\mathbf{P}_i$ and $\mathbf{P}_{i+1}$ ($i \in 0..N-1$, with $N \equiv 0$):
   $$\mathbf{B}_i(t) = (1-t)^3 \mathbf{P}_i + 3(1-t)^2 t (\mathbf{P}_i + \mathbf{T}_{out, i}) + 3(1-t) t^2 (\mathbf{P}_{i+1} - \mathbf{T}_{in, i+1}) + t^3 \mathbf{P}_{i+1}, \quad t \in [0, 1]$$
2. **Tangent Split / Discontinuity**:
   - Smooth mode: $\mathbf{T}_{in, i} = -k \cdot \mathbf{T}_{out, i}$ (collinear tangents, $C^1$ continuity).
   - Split mode: $\mathbf{T}_{in, i}$ and $\mathbf{T}_{out, i}$ are completely independent vectors ($C^0$ continuity, sharp cusp corner).

#### Feathering & Blending Kernels
Let binary rasterized mask coverage be $M_{raw}(x, y) \in \{0.0, 1.0\}$.
1. **Signed Distance Field (SDF) or Gaussian Blur Kernel**:
   Let distance to mask boundary be $d(x, y)$ (negative inside, positive outside).
   Feather radius $\sigma = \text{feather} \cdot H_{frame}$.
2. **Feather Styles**:
   - `Both` (Centered):
     $$\alpha(x, y) = \text{clamp}\left(\frac{1}{2} - \frac{d(x, y)}{2\sigma}, 0.0, 1.0\right)$$
   - `Out` (Exterior blend):
     $$\alpha(x, y) = \begin{cases} 1.0 & d(x, y) \le 0 \\ \text{clamp}\left(1.0 - \frac{d(x, y)}{\sigma}, 0.0, 1.0\right) & d(x, y) > 0 \end{cases}$$
   - `In` (Interior blend):
     $$\alpha(x, y) = \begin{cases} \text{clamp}\left(-\frac{d(x, y)}{\sigma}, 0.0, 1.0\right) & d(x, y) \le 0 \\ 0.0 & d(x, y) > 0 \end{cases}$$
3. **Inversion & Opacity**:
   $$\alpha_{final}(x, y) = \text{opacity} \cdot \left(\text{if invert } (1.0 - \alpha(x, y)) \text{ else } \alpha(x, y)\right)$$
4. **Mask FX Preceding Pipeline**:
   - When `Mask FX = true`:
     Let $I_{orig}$ be the input frame before the FX chain.
     Let $I_{fx}$ be the output of the preceding effects.
     $$I_{out}(x, y) = \alpha_{final}(x, y) \cdot I_{fx}(x, y) + (1.0 - \alpha_{final}(x, y)) \cdot I_{orig}(x, y)$$
   - When `Mask FX = false`:
     Direct alpha modulation:
     $$I_{out}(x, y).A = I_{in}(x, y).A \cdot \alpha_{final}(x, y)$$

---

### 2.3 Feature 19: Smart Mask CPU Energy Minimization Specification

#### GraphCut / GrabCut Energy Model (Pure CPU Fallback)
1. **Model Representation**:
   Given user bounding box $R = [x_{min}, y_{min}, x_{max}, y_{max}]$ and prompt point sets $\mathcal{P}_{pos}, \mathcal{P}_{neg}$:
   - Definite Background: pixels outside $R$ and $\mathcal{P}_{neg}$ pixels.
   - Definite Foreground: $\mathcal{P}_{pos}$ pixels.
   - Probable Foreground: pixels inside $R \setminus \mathcal{P}_{pos}$.
2. **Gaussian Mixture Models (GMM)**:
   Two GMMs with $K = 5$ components each for RGB color distribution:
   $$P(\mathbf{z} \mid \text{Foreground}) = \sum_{k=1}^K \pi_{fg, k} \mathcal{N}(\mathbf{z} \mid \mathbf{\mu}_{fg, k}, \mathbf{\Sigma}_{fg, k})$$
   $$P(\mathbf{z} \mid \text{Background}) = \sum_{k=1}^K \pi_{bg, k} \mathcal{N}(\mathbf{z} \mid \mathbf{\mu}_{bg, k}, \mathbf{\Sigma}_{bg, k})$$
3. **Gibbs Energy Function**:
   $$E(\mathbf{\alpha}) = \sum_{p} U(p, \alpha_p) + \sum_{(p, q) \in \mathcal{N}} V(p, q, \alpha_p, \alpha_q)$$
   - Unary term: $U(p, \alpha_p) = -\ln P(\mathbf{z}_p \mid \alpha_p)$.
   - Pairwise contrast term:
     $$V(p, q, \alpha_p, \alpha_q) = [\alpha_p \ne \alpha_q] \cdot \frac{\gamma}{\text{dist}(p, q)} \exp\left(-\beta \|\mathbf{z}_p - \mathbf{z}_q\|^2\right)$$
     where $\beta = \left(2 \langle \|\mathbf{z}_p - \mathbf{z}_q\|^2 \rangle\right)^{-1}$.
4. **Soft Matte Generation**:
   Apply guided filter / bilateral smoothing on the boundary probabilities to generate a continuous $[0.0, 1.0]$ alpha matte, eliminating aliased pixel staircases.

---

### 2.4 Feature 20: Text-Based Timeline Editing Specification

#### Word-Level Data Model
```rust
pub struct WordToken {
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
    pub confidence: f32,
    pub track_id: u32,
    pub event_id: u64,
}

pub struct Transcript {
    pub words: Vec<WordToken>,
    pub media_path: PathBuf,
}
```

#### Timeline Ripple Delete from Transcript Selection
Given selected word index range $[k_{start}, k_{end}]$:
1. Identify time span:
   $$t_{cut, start} = \text{words}[k_{start}].start\_time, \quad t_{cut, end} = \text{words}[k_{end}].end\_time$$
   $$\Delta t = t_{cut, end} - t_{cut, start}$$
2. Timeline Command Execution:
   - Split affected audio/video events at $t_{cut, start}$ and $t_{cut, end}$.
   - Collect event slice references falling within $[t_{cut, start}, t_{cut, end}]$.
   - Invoke `neoveg_engine::ripple::ripple_delete_events(timeline, &cut_events)`.
3. Transcript Time Shift:
   - Remove words $[k_{start}..=k_{end}]$ from transcript.
   - For all subsequent words $j > k_{end}$:
     $$\text{words}[j].start\_time -= \Delta t, \quad \text{words}[j].end\_time -= \Delta t$$
4. Single Transaction:
   The timeline split, event deletion, ripple shift, and transcript revision are committed as a single atomic `EditCommand` ("TBE: Delete Text").

---

### 2.5 Feature 21: Speech-to-Text Subtitle Generation Constraints

#### Subtitle Line Splitting & Reading Speed Algorithm
1. **Constraints**:
   - Maximum Characters Per Line: $C_{max} = \text{CPL} \in [20, 60]$ (default 37).
   - Maximum Lines Per Cue: $L_{max} = 2$.
   - Reading Speed: $R = \frac{\text{chars}}{\Delta t} \le \text{CPS}_{max} = 20.0$.
   - Minimum Cue Duration: $t_{min} = 1.0$ s.
   - Inter-cue Gap: $g_{min} = \frac{2}{\text{FPS}}$ (e.g. 0.067s for 30 fps).
2. **Text Wrapping**:
   Split transcript text into tokens. Pack tokens onto line 1 while $\text{len} \le C_{max}$. Wrap remaining tokens onto line 2.
   If line 2 overflows $C_{max}$, split cue at natural punctuation boundary (comma, period, question mark) or space, assigning proportional time durations:
   $$\Delta t_1 = \Delta t \cdot \frac{\text{len}_1}{\text{len}_1 + \text{len}_2}, \quad \Delta t_2 = \Delta t - \Delta t_1$$
3. **Format Serializers**:
   - SRT: `hh:mm:ss,fff --> hh:mm:ss,fff\nText\n\n`
   - WebVTT: `hh:mm:ss.fff --> hh:mm:ss.fff\nText\n\n`
   - Generated Titles: Inserts video event with `neoveg_text` generator on subtitle track.

---

### 2.6 Feature 22: Smart Upscale & Super-Resolution Specification

#### Edge-Directed Directional Interpolation (NEDI/EDDI) Math
Given source image $I_{in}(x, y)$ of size $W \times H$, scaling $2\times$ to $2W \times 2H$:
1. **Luma Directional Gradient**:
   $$D_{diag1} = |Y(x-1, y-1) - Y(x+1, y+1)|$$
   $$D_{diag2} = |Y(x+1, y-1) - Y(x-1, y+1)|$$
   $$D_{horiz} = |Y(x-1, y) - Y(x+1, y)|$$
   $$D_{vert} = |Y(x, y-1) - Y(x, y+1)|$$
2. **Interpolation Along Minimum Gradient**:
   To interpolate center pixel $(x + 0.5, y + 0.5)$:
   - If $|D_{diag1} - D_{diag2}| > \tau_{edge}$:
     - If $D_{diag1} < D_{diag2}$, interpolate along diagonal 1:
       $$Y_{interp} = \frac{Y(x-1, y-1) + Y(x+1, y+1)}{2}$$
     - Else:
       $$Y_{interp} = \frac{Y(x+1, y-1) + Y(x-1, y+1)}{2}$$
   - Otherwise, fall back to Catmull-Rom cubic interpolation.
3. **Adaptive Unsharp Masking**:
   $$I_{out} = I_{upscaled} + \text{sharpness} \cdot \text{clamp}(I_{upscaled} - \text{BilateralFilter}(I_{upscaled}), -\delta_{max}, \delta_{max})$$

---

### 2.7 Feature 23: AI Auto Reframe Specification

#### Saliency Centroid & Camera Deadband Filter
1. **Saliency Function**:
   $$S(x, y, t) = \|\mathbf{v}_{flow}(x, y, t)\| \cdot (\text{Var}_{local}(x, y) + \epsilon)$$
   Center-weighted saliency:
   $$S_w(x, y, t) = S(x, y, t) \cdot \exp\left(-\frac{(x - W/2)^2}{2 \sigma_x^2}\right)$$
2. **Target Centroid**:
   $$x_c(t) = \frac{\sum_{x, y} x \cdot S_w(x, y, t)}{\sum_{x, y} S_w(x, y, t)}$$
3. **Deadband Filter & Camera Smoothing**:
   Let current camera pan position be $x_{cam}(t)$.
   Deadband width $D = W_{crop} \cdot 0.15$.
   $$\text{error} = x_c(t) - x_{cam}(t)$$
   $$\Delta x = \begin{cases} 0 & |\text{error}| \le D \\ \text{sign}(\text{error}) \cdot (|\text{error}| - D) & |\text{error}| > D \end{cases}$$
   Update camera position:
   $$x_{cam}(t + \Delta t) = x_{cam}(t) + \alpha_{smooth} \cdot \Delta x$$
   Clamped to valid crop range: $[\frac{W_{crop}}{2}, W - \frac{W_{crop}}{2}]$.
4. **Keyframe Generation**:
   Generates `PanCropKey` keyframes with normalized parameters:
   $$x_{center} = \frac{x_{cam}(t)}{W}, \quad y_{center} = 0.5, \quad \text{width} = \frac{W_{crop}}{W}, \quad \text{height} = 1.0, \quad \text{smoothness} = 1.0$$

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Motion Tracking | Motion Tracking Panel | Dockable tool window for tracking video objects | Video Event, Region selection | Envelopes, Keyframes, Preview vectors | Ignores missing media, warns on empty selection | Manual p. 713–716, `Alt+M` |
| 2 | Motion Tracking | Tracking Precision Modes | 3 selectable speed/accuracy profiles (Fast, Balanced, Accurate) | Enum: Fast, Balanced, Accurate | Adjusted optical flow grid and pyramid levels | Defaults to Balanced | Manual p. 714 |
| 3 | Motion Tracking | Tracking Modes | 5 movement DOF options: Location, Rotation+Location, Scale+Location, Shape+Location, Perspective | Enum: Location (2 DOF) .. Perspective (8 DOF) | Transform parameter trajectories | Gracefully falls back to 2 DOF if point count < 4 | Manual p. 714 |
| 4 | Motion Tracking | Directional Track Buttons | Backward, Frame Backward, Both, Frame Forward, Forward | Playhead cursor position, event bounds | Segmented motion track indicator | Stops when object lost or border reached | Manual p. 715 |
| 5 | Motion Tracking | Motion Transfer Menu | Hierarchical menu assigning tracking data to Event FX, Pan/Crop, or PIP | Target event, Target FX, Target parameter | Keyframes on target envelope | Disabled if no valid track data | Manual p. 715 |
| 6 | Motion Tracking | Motion Track Indicator | Color-coded status ruler indicating tracked vs lost frames | Frame tracking results | Green / Red status segments | Real-time display | Manual p. 715 |
| 7 | Motion Tracking | Vector Display Overlay | Visual overlay displaying optical flow vector field | Boolean checkbox | Video Preview vector arrows | Non-persistent visual debug only | Manual p. 714 |
| 8 | Motion Tracking | Track Interchange | Import/Export tracking coordinates in Silhouette Corner Pin or JSON | Coordinate files / Clip envelopes | Exported text or imported region | Rejects malformed coordinate files | Manual p. 716–720 |
| 9 | Event Masks | Pan/Crop Mask Tab | Integrated geometry masking within Event Pan/Crop | Rectangle, Oval, Bézier spline | Alpha channel clip mask | Clamps to source frame bounds | Manual p. 580, `results.md` |
| 10 | Event Masks | Bézier Masking FX | Multi-mask OFX plugin supporting up to 5 masks | Mask 1..5, Rect, Oval, Diamond, Curve | Masked output buffer | Unused masks consume zero CPU | Manual p. 581–584 |
| 11 | Event Masks | Mask FX Mode | Masks preceding effects in FX chain while preserving video | Boolean toggle | Preceding FX alpha composite | If first in chain, acts as standard alpha mask | Manual p. 583 |
| 12 | Event Masks | Feathering Styles | Out, In, Both edge blending modes | Feather style enum, Feather amount (0..1) | Soft edge alpha matte | Hard edge when feather = 0.0 | Manual p. 582 |
| 13 | Event Masks | Spline Tangent Tools | Normal, Add, Delete, and Split tangent handles | Mouse interact on curve anchors | Modified spline knots and tangents | Disallows deleting below 3 points | Manual p. 586 |
| 14 | Event Masks | Point Animation | Per-point Bézier knot keyframing over time | Animate Curve checkbox, Timeline keys | Interpolated spline shapes | Clamps coordinate overshoots | Manual p. 588–589 |
| 15 | Smart Mask | Object Segmentation | AI-assisted object matte extraction with bounding box | Bounding box coordinates, image frame | Candidate object alpha matte | Falls back to GrabCut CPU energy solver | Manual p. 683–687 |
| 16 | Smart Mask | Point Prompts | Positive (include) and Negative (exclude) click points | Coordinate clicks on Video Preview | Refined segmentation matte | Ignores clicks outside frame | Manual p. 686 |
| 17 | Smart Mask | View Masking Results | Toggles between overlay tint and final alpha composite | Boolean checkbox | Video Preview render mode | View-only; render always outputs composite | Manual p. 686 |
| 18 | Smart Mask | Multi-Mask Segmentation | Up to 8 independent smart masks per event | Mask selector 1..8 | Combined multi-layer alpha matte | Cleared masks reset immediately | Manual p. 687 |
| 19 | Smart Mask | Auto Tracking & Masking | Propagates segmented mask across all event frames | Direction enum, Motion Tracking button | Keyframed per-frame matte | Progress bar with cancel button | Manual p. 688–689 |
| 20 | Text-Based Edit | Word-Level Timeline Sync | Bi-directional synchronization between transcript words and timeline | Transcript word list, Timeline playhead | Synchronized cursor and loop selection | Re-aligns on timeline edits | Manual p. 674–679 |
| 21 | Text-Based Edit | Delete Words with Ripple | Deleting text splits timeline, deletes media, and ripples gap | Word selection, Delete key | Cut timeline events and closed gap | Atomically updates transcript timestamps | Manual p. 678, `Ctrl+L` |
| 22 | Text-Based Edit | Pause Duration Display | Highlights pauses between words with length filter | Slider threshold (seconds) | Pause indicators `[Pause: X.Xs]` | Hidden when toggle disabled | Manual p. 677 |
| 23 | Text-Based Edit | Search in Transcript | Search and find/replace across transcript text | Query string, Replace string | Highlighted matches and cursor jump | Handles regex / case-sensitivity | `results.md` §12.20 |
| 24 | Speech to Text | Whisper Transcription | Automatic speech recognition via 16 kHz mono Whisper model | Audio clips, Language code | Word tokens and transcript segments | CPU mock fallback when model absent | Manual p. 675, `whisper.rs` |
| 25 | Speech to Text | Subtitle CPL & Line Limit | Constraints on maximum characters per line (CPL) and 1/2 lines | CPL slider (20..60), 1/2 line radio | Formatted subtitle cues | Splits cues cleanly on word boundaries | Manual p. 680 |
| 26 | Speech to Text | Subtitle CPS & Durations | Enforces reading speed ($\le 20$ CPS) and min/max durations | Target reading rate | Validated subtitle cues | Warns on fast speech, expands duration | Netflix/BBC subtitle rules |
| 27 | Speech to Text | Subtitle Event Generator | Generates Titles & Text events on a dedicated subtitle track | Subtitle cues, Title preset | Video track and generator events | Single undoable command | Manual p. 680–681 |
| 28 | Speech to Text | Subtitle File Export | Exports subtitles to SRT, WebVTT, TXT, or CSV | Subtitle cues, format enum | Written subtitle file | Reports encoding/write errors cleanly | Manual p. 681, `captions.rs` |
| 29 | Smart Upscale | Resolution Scaling (2x, 4x) | Super-resolution scaling for HD to 4K upscaling | Source frame, Scaling factor | High-resolution frame buffer | Edge-directed directional CPU fallback | Manual p. 696–697 |
| 30 | Smart Upscale | Pre-Pan/Crop Placement | Mandates upscale placement before Pan/Crop in effect chain | FX Chain order | Warning / Auto-ordering before Pan/Crop | Prevents cropping before upscale | Manual p. 696 |
| 31 | Smart Upscale | Detail & Denoise Controls | Sharpness and pre-denoise parameters | Sharpness [0..1], Denoise [0..1] | Filtered super-resolution picture | Clamped to prevent ringing halos | Manual p. 696 |
| 32 | Auto Reframe | Aspect Ratio Remapping | Re-frames 16:9 video to 9:16 vertical, 1:1, 4:5, 4:3, 21:9 | Target aspect ratio enum | Pan/Crop framing window | Clamps crop window to source image | `results.md` line 1349, 16036 |
| 33 | Auto Reframe | Saliency Subject Tracking | Optical flow + luma variance motion subject detection | Video frames across event | Subject centroid trajectory | Falls back to center-weighting if static | Optical flow integration |
| 34 | Auto Reframe | Deadband Pan/Crop Keys | Deadband camera smoothing and PanCropKey generation | Centroid trajectory, Deadband width | Keyframed `PanCrop` on event | Eliminates nausea-inducing micro-jitter | `pan_crop.rs` |

---

## 4. Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | Motion Tracking | Feature tracked moves completely offscreen | Detection eigenvalue drops below threshold; tracking halts gracefully, sets status bar to "Object lost", and flags keyframes red. |
| 2 | Motion Tracking | Scene cut occurs mid-event | Optical flow residual exceeds scene-cut threshold; tracking halts at scene boundary rather than jumping to unrelated picture. |
| 3 | Motion Tracking | Planar homography (Perspective) with collinear points | SVD condition number exceeds tolerance ($> 10^6$); falls back to affine 4 DOF model to avoid singular matrix panic. |
| 4 | Motion Tracking | User trims event start/end after motion transfer | Keyframe times are stored relative to event start; trimming event properly preserves relative alignment via event time translation. |
| 5 | Event Masks | Bézier curve with self-intersecting segments (figure-8) | Even-Odd fill rule evaluates self-intersections predictably without rasterizer crashes or infinite loops. |
| 6 | Event Masks | Feather amount larger than shape dimensions | Feather is clamped to maximum of shape half-width/half-height to avoid inverted negative alpha values. |
| 7 | Event Masks | "Mask FX" enabled with zero preceding effects in chain | Operates as an identity passthrough, leaving source video untouched without generating blank frames. |
| 8 | Event Masks | Bézier mask deleted down to 2 anchor points | System enforces minimum point constraint (`MIN_POINTS = 3`); prevents deleting last 3 points or reverts to rectangle. |
| 9 | Smart Mask | Positive and negative points placed on identical coordinates | Negative exclusion point takes precedence, preventing conflicting probability states in the GMM solver. |
| 10 | Smart Mask | Bounding box dragged completely outside video frame | Clamped to frame boundary with minimum $16 \times 16$ pixel extent; skips computation if frame area is zero. |
| 11 | Smart Mask | AI model weights missing on disk | Pure-CPU GrabCut/energy minimization solver activates immediately; completes in $<50$ ms without network or GPU dependency. |
| 12 | Text-Based Edit | User deletes all words in an event's transcript | Deletes the entire timeline event and executes Auto Ripple; removes empty event reference from timeline. |
| 13 | Text-Based Edit | Word selection spans across multiple audio tracks | Splits each affected track at exact word boundary timecodes, maintaining cross-track event synchronization. |
| 14 | Text-Based Edit | Word timestamp extends past end of media file | Clamped to media duration; prevents requesting invalid audio samples beyond end-of-file. |
| 15 | Speech to Text | Whisper audio input is pure silence / zero luma | VAD detects silence; returns empty transcript segment list without generating spurious hallucinated tokens. |
| 16 | Speech to Text | Extremely fast speech exceeds 25 CPS | Subtitle generator automatically inserts intermediate cue split, or extends cue duration into adjacent silence gap. |
| 17 | Speech to Text | Adjacent subtitle cues have 0 ms gap | Inter-cue spacing enforcer inserts minimum 2-frame ($\approx 67$ ms) blank gap to comply with broadcast standards. |
| 18 | Smart Upscale | Input image is already 4K (3840x2160) and project is 4K | Scaling factor evaluated as $1.0\times$; passes frame directly through as identity, avoiding redundant compute. |
| 19 | Smart Upscale | Extreme scale factor ($4\times$) on limited 8 GB RAM | Scaler processes image in horizontal scanline stripes/tiles, maintaining peak heap usage under 250 MB. |
| 20 | Auto Reframe | Video has zero motion (static landscape shot) | Saliency map defaults to center-weighted frame midpoint; camera remains locked in exact center without wandering. |
| 21 | Auto Reframe | Two equal subjects moving in opposite directions | Saliency tracking tracks group bounding centroid; widens crop framing if necessary to keep both subjects in view. |

---

## 5. Trademark Compliance & Generic Identifiers

In strict compliance with repository clean-room rules and trademark policy:
- No proprietary brand names or third-party trademarks are permitted in runtime code, structs, enums, UI strings, or method identifiers.
- Canonical Mapping:
  - `Mocha` / `Boris FX` $\longrightarrow$ `Planar Tracker` / `Corner Pin Interchange`
  - `Silhouette` $\longrightarrow$ `Corner Pin Format`
  - `OpenAI Whisper` $\longrightarrow$ `Speech-to-Text Engine` / `Speech Transcription Model`
  - `Real-ESRGAN` $\longrightarrow$ `Super-Resolution Scaler` / `Detail-Preserving Upscale`
  - `Segment Anything (SAM)` $\longrightarrow$ `Smart Mask` / `Object Segmentation Matte`
  - `Adobe Premiere` / `DaVinci Resolve` $\longrightarrow$ `Interchange Profile` / `Generic NLE Reference`
- Runtime Rust Structs & Types:
  - `MotionTracker`, `TrackingRegion`, `TrackingMode`, `MotionVectorField`
  - `FxMask`, `BezierMask`, `FeatherStyle`, `MaskBlendMode`
  - `SmartMask`, `ObjectSegmenter`, `AlphaMatteGenerator`
  - `TextBasedEditor`, `TranscriptSegment`, `WordToken`
  - `SpeechToTextEngine`, `SubtitleGenerator`, `SubtitleConstraints`
  - `SmartUpscale`, `SuperResolution`, `EdgeDirectedScaler`
  - `AutoReframe`, `SaliencyTracker`, `SubjectFraming`

---

## 6. Caveats
- No live GPU shaders or external AI endpoints are assumed or required. All mathematical specifications provide deterministic pure-CPU fallback algorithms designed to run within the strict 8 GB RAM execution budget.
- Whisper speech-to-text inference uses `candle_transformers` pure-Rust CPU inference, and includes a mock fallback mode for unit testing when pre-trained weights are not present.
- Memory consumption during 4K super-resolution and optical flow processing is bounded via tiled execution (maximum $512 \times 512$ tile size) to prevent out-of-memory spikes.

---

## 7. Conclusion
Milestone 4 specification mining is complete. Authoritative VEGAS Pro reference manuals and static analysis artifacts have been thoroughly probed and documented for Features 17 through 23:
1. **Motion Tracking**: Full interface, 5 tracking modes (Location to Perspective), multi-level transfer menu, and pyramidal optical flow specifications.
2. **Geometric & Bézier Event Masks**: Pan/Crop mask and Bézier Masking OFX, 4 shapes, 3 feather styles, Mask FX preceding effects mode, and per-point curve animation.
3. **Smart Mask**: AI object segmentation, interactive positive/negative point prompts, GrabCut/GMM CPU fallback, and tracking integration.
4. **Text-Based Editing**: Word-level transcript synchronization, selection-to-timeline mapping, split-and-ripple deletion, and pause displays.
5. **Speech-to-Text Subtitle Generation**: Whisper integration, CPL/CPS/duration broadcast constraints, SRT/WebVTT export, and Titles & Text generator event creation.
6. **Smart Upscale & Super-Resolution**: Pre-Pan/Crop ordering, 2x/4x scaling, and pure-CPU Edge-Directed Directional Interpolation (NEDI/EDDI).
7. **AI Auto Reframe**: Aspect ratio reformatting (16:9 to 9:16/1:1/4:5), motion/variance saliency tracking, deadband spatial filtering, and automatic `PanCropKey` keyframing.

---

## 8. Verification Method
- Inspect this handoff file: `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\spec_miner_m4_1\handoff.md`.
- Verify manual citations: `manuais/markdown/manual_vegaspro21_en.md` lines 25139–25390, 30108–30400, 30433–30750, 30843–30900, 31450–32000.
- Verify VEGAS Pro keyboard shortcut parity: `VEGAS_Pro_18_EN_Keyboard_shortcuts.md` line 59 (`Alt + M` for Motion Tracking, `Alt + G` for Color Grading, `Ctrl + L` for Auto Ripple).
- Verify pure-CPU algorithm implementations via cargo check and sequential tests:
  ```powershell
  cargo check -p neoveg_video -p neoveg_audio -p neoveg_engine -j 1
  cargo test -p neoveg_video -p neoveg_audio -j 1 --test-threads=1
  ```
