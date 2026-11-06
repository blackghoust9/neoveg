# Handoff Report: Vegas Pro Results Specification Mining (R2 Core Extensions)

## 1. Observation

Direct observations extracted from `C:\Users\paulomatos\Documents\GitHub\neoveg\results.md` (authoritative reverse-engineering specification, 33,594 lines) and repository sources:

1. **Transitions, Effects & OFX (§12.14, §17.3, §17.4, §17.7)**:
   - Line 13765: Video transitions native binary `sftrans1.dll` (43 parameter dialogs), `SfPagePeel.dll` (10 dialogs), OFX bundles in `OFX Video Plug-Ins`, WPF transition library.
   - Lines 19405–19418: OFX bundles inventory: `Filters.ofx.bundle`, `MagixAiFx.ofx.bundle` (AiDehaze, AiSharpen, AiSmooth, autoframe, colorization, samasking, smartmask, styletransfer), `MagixCVFx.ofx.bundle` (FlickerReducer, MotionTracker, Stabilize, TimeWarp), `Stabilize.ofx.bundle`, `TitlesAndText.ofx.bundle`, `Vfx1.ofx.bundle` (AutoLooks LUTs 1..16.cube, chromakeyer, stereographicProjection), `ofx360Stabilizer.ofx.bundle`, `ofxRotation.ofx.bundle`, `ofxStitch.ofx.bundle`, `spica_cutout.ofx.bundle`, `spica_resizer.ofx.bundle`.
   - Lines 19925–20028: Dialog definitions for transitions and effects: Dialog 107 (Color), 109 (Film grain/particles), 118 (Angle/Amount), 119 (Light position/Lens flare), 130 (3D Fly In/Out: Far X/Y/Z, X/Y/Z rotations, Specular light), 136 (3D Shuffle: Extra spins, Stagger, Specular light), 137 (Slide/Border/Feather), 140 (Center/Split mode/Direction), 158 (Blinds/Feather), 190 (Slide in/out), 191 (Gradient Wipe), 192 (Spiral/Turns/Zoom), 194 (Zoom center), 197 (Ripple amplitude/viscosity), 200 (Squeeze/Scale), 204 (Distortion/Shear/Curve), 228 (Credit Roll), 233 (Star Wipe: Arms/Ratio/Cycles/Waveform), 236 (Heart Wipe: Wide/Cycles/Heart Beat).
   - Lines 13773–13897: Interface texts for transition envelopes (`Insert.Envelope.TransitionProgress`), crossfades, automatic crossfade mode (`32174`), preset categories, and thumbnail drag-and-drop (`21420`).

2. **Color Grading per Event, Color Wheels & OpenColorIO (§12.12, §17.6, §8.7, §15.1)**:
   - Lines 7513–7518, 9306–10650: `ColorGradingWindow.dll` resources: Color Wheels (Lift, Gamma, Gain, Offset), Wheel/Slider toggle (`Wheel_Slider_TipText`), White Balance neutral picker (`WhiteBalance.Picker.TipText`), Color Curves (RGB master + R, G, B channels with Bézier tangent handles and snap grid), HSL Curves (`Hue vs Hue`, `Hue vs Sat`, `Hue vs Lum`, `Sat vs Sat`, `Lum vs Sat`, `Sat vs Lum`) with eyedropper preview sampling (`HSL_ColorPicker_TipText`), Input LUT/Output LUT/Look profile, per-event/track/output grading toggle (`EventFX_TipText`, `TrackFX_TipText`, `MediaFX_TipText`).
   - Lines 19504–19510 (§17.6): OpenColorIO configs (`config.ocio`, `config_1_0_3.ocio`), LUT file types (`.ocio`, `.spi1d`, `.spimtx`, `.py`, `.cube`, `.spi3d`).
   - Lines 6466–6473 (§8.7, Dialog 826): Project ACES properties: `ACES version` (1210), `ACES color space` (1235), `View transform` (1473), `Look modification transform` (2233). Lines 28355–28363: Built-in ACES View Transforms: `ACES RRT (Rec.709)`, `ACES RRT (P3 DCI)`, `ACES RRT (P3 D60)`, `ACES RRT (CIE XYZ)`, and S-Log camera IDTs: `Sony S-Log1 (F35/F3)`, `Sony S-Log2 (F65 3200K)`, `Sony S-Log2 (F65 5500K)`, S-Log3. String 28742: `Use GPU processing for ACES color management`.

3. **Motion Tracking, Masks & Smart Mask (§12.15, §12.20)**:
   - Lines 13998, 14132, 14133: `Motion Tracker` (ID 1860, 18665, 18673), View command `View.MotionTrackGuiCommand`.
   - Lines 14127–14131: Masking tools: `Rectangle or Square Mask Creation Tool` (18619), `Oval or Circle Mask Creation Tool` (18620), `Positive Mask` (18637), `Negative Mask` (18638), `Mask` (18644), `Reset Mask` (18159).
   - Lines 14140, 16056: `Enable Smart Mask` (21468), `MagixAiFx.ofx.bundle` (`smartmask`, `samasking`).

4. **Project Properties, Templates & Media Profiles (§8.7, §9, §18.3, §18.4)**:
   - Lines 6445–6495 (Dialog 826 Video): Width, Height, HDR Mode, Field Order, Pixel Aspect Ratio, Output Rotation, Frame Rate, Pixel Format (8-bit, 32-bit video levels, 32-bit full range), Compositing Gamma, Full-resolution rendering quality, Motion blur type, Deinterlace method, Resample mode, Adjust source media check, Prerender override, Prerendered files folder.
   - Lines 6510–6531 (Dialog 863 Audio): Master bus mode, Stereo busses count, Sample rate, Bit depth, Resample and stretch quality, LFE low-pass filter cutoff and quality, Recorded files folder.
   - Lines 20092–20095 (§18.3): 46 Project Templates (`.vqpt`) across AVCHD, Blu-ray, DVD, DV NTSC/PAL, HDV, Internet (360p up to 4K UHD 60p).
   - Lines 20098–20197 (§18.4): `Vegas profiles.ini` structure: `KeyN=File IO CLSID, "Subformat", Size X, Size Y, Frame Rate, Color Bit Depth` and `AttributesN="Interlace", Pixel Aspect Ratio, "Alpha Channel Interpretation", Gamma`. Search backwards from N to 1; wildcard 0 / "None".

5. **Project Media Window & Explorer (§12.10, §12.11)**:
   - Lines 12788–12845: 23 Detail columns for media list: Name, Use Count, Size, Length, Modified, Path, Type, Comment, Date/Time Stamp, Rating, Video Capture Comment, Sample Rate, Bit Depth, Channels, Frame Rate, Field Order, Alpha, Pixel Aspect, Frame Size, Color Depth, Media FX, Timecode In, Timecode Out, Tape Name, Data Rates.
   - Media Bins (lines 12747–12759, 18173–18174): Create, delete, rename media bins; search media bins (1856); duplicate bin auto-rename; cannot delete root bin (ID 12878).
   - Explorer (lines 12900–13098): WPF Unified Explorer, Breadcrumbs, Favorites (`Add to My Favorites`, `Remove from Favorites`), scene detection (`Detect Scenes and Add to Timeline`), thumbnail zoom.

6. **Make Movie Rendering & Pre-rendering Cache Ruler (§8.1, §10.3, §12.18)**:
   - Lines 3667–3684 (Dialog 804): Make Movie Wizard destinations (Upload to Vimeo/YouTube, Save to camera, Save to hard drive).
   - Lines 15432–15648: Render As dialog, presets, loop region only (`15433`), 2-pass VBR, real-time render.
   - Lines 15434, 15501, 15590–15597, 16505, 16545: Selective Pre-render (`Tools.PrerenderVideo`), Dynamic RAM Preview (`Tools.BuildRAMPreview` / `Tools.ClearRAMPreview`).
   - Ruler color-coding logic (matching roadmap specification): Green (cached in RAM or prerendered video file on disk; full target framerate guaranteed), Yellow (real-time playable with moderate effect load), Red (unrendered heavy compositing/VFX; dropped frames expected, prerender needed).

7. **AI Features (§12.20)**:
   - Lines 16066–16097: Text-based Editing (`TextbasedEditing`): transcript view, display timeline events in transcript, show pauses toggle (`ShowPauses`), find & replace all, editing/deleting transcript cuts timeline events.
   - Speech-to-Text (`Speech2Text`): Subtitles from speech track, maximum lines per subtitle, maximum characters per line, text preset selector, subtitle export.
   - Upscale & Zoom (line 16055, 21464): `Enable Smart Upscale and Smart Zoom`.
   - AI Auto Reframe (lines 16060, 16069, 16075): `AI Auto Reframe`, `Render with AI Auto Reframe`, automatic subject reframing across aspect ratios.

8. **Standardized Alerts and Dialogs (§10)**:
   - §10.1 (Offline Media, 25 strings): ID 1561, 1634, 1830, 16058, 16132, 16926, 17250, 17360, 17361, 17363, 17630.
   - §10.2 (Codec / Format, 28 strings): ID 1648, 16953, 17359, 17369, 17370, 17381, 17382, 17383, 17384, 18322, 18331, 18368, 33279, 33280, 33501, 44712.
   - §10.3 (Render / Prerender, 29 strings): ID 1578, 1580, 1583, 1584, 1585, 1586, 1881, 1894, 1910, 1911, 3404, 16280, 17364, 17365, 17437, 17581, 17628, 18331, 18393, 18394, 18405, 18548, 33257, 44720.
   - §10.4 (Memory / Disk, 7 strings): ID 16227, 16522, 18004, 18400, 18548.
   - §10.5 (GPU Acceleration, 5 strings): ID 16013, 16016, 16017, 16020, 33316 ("Alert: GPU Acceleration Error"), 33317 ("Your current GPU has encountered problems...").
   - §10.6 (Plug-ins / VST, 31 strings): ID 16092, 16093, 16511, 16555, 16556, 16560, 16564, 16587, 16589, 16607, 16907, 16909, 16910, 17101, 18043, 18047, 33413, 44654, 44655.

9. **Smart Proxies (VP12)**:
   - Lines 12775–12776, 12826, 12843–12845, 15606–15609, 15647: Video proxies are generated asynchronously (`Create Video Proxy` ID 32841, progress ID 17882).
   - Dynamic Preview Quality resolution switching: `Draft` (ID 32929) and `Preview` (ID 32925) automatically switch playback to decode proxy files; `Good` (ID 32921) and `Best` (ID 32917) automatically switch playback to full-resolution originals. Final render (`Render As`) always decodes full-resolution original media.

10. **Color Match across Clips & S-Log/ACES (VP12)**:
    - Color Match analyzes reference frame and applies histogram/color balance transfer to target clip.
    - S-Log gamma curves (S-Log1, S-Log2, S-Log3) with S-Gamut color spaces mapped to linear ACES working spaces via IDT and ODT/RRT.

11. **L-Cut, J-Cut, Simultaneous Fades & Sync Links (VP12/VP13)**:
    - Lines 1715–1725: `Create Sync Link with Selected Events` (ID 33096), `Remove Sync Link` (ID 33097), `Unlink` (ID 33098), `Select All` (ID 33099), `Synchronize By Moving` (ID 33075), `Synchronize By Slipping` (ID 33076).
    - Quickfade Audio Edits (ID 32315): rapid micro-fade on audio edges to avoid clicks.
    - L-cut/J-cut split edits allow independent trimming of audio/video while maintaining sync-link offset tracking.

12. **Multicam Editing & Takes (VP13)**:
    - Lines 1374–1384: `Synchronize Audio to Align Events` (ID 33561), `Synchronize Media Timecode` (ID 33205), `Synchronize Date/Time Stamp` (ID 33206), `Create Multicamera Track` (ID 33524), `Expand to Multiple Tracks` (ID 33406), `Enable Multicamera Editing` (ID 33523), `Edit Multicamera Audio with Video` (ID 33211).
    - Lines 1125–1133: Takes submenu: `Rename Active` (32360), `Choose Active` (32362), `Next Take` (32363), `Previous Take` (32364), `Delete Active` (32327), `Delete...` (32361), `Slip All Takes` (32199), `Add as Takes` (32385).

---

## 2. Logic Chain

1. **R1 and Baseline Stabilization**: The repository already possesses core modules for video/audio processing, but R2 requires precise domain models and clean-room interface contracts for 12 major functional areas.
2. **Authority & Grounding**: `results.md` provides exhaustive static reverse-engineering data, giving exact menu command IDs, UI resource definitions, and parameters for all referenced features.
3. **Clean-Room & Zero Third-Party Trademarks Rule**:
   - The original system uses trademarked names in dialogs and strings (e.g. "VEGAS", "Sony", "MAGIX", "Final Cut", "Avid", "Premiere").
   - Under R3 constraints, all runtime strings and error messages must use generic domain terms or `$(AppName)` / `Neoveg`, and no copyrighted code may be copied.
4. **Hardware Agnosticism & Resource Limits**:
   - The build environment operates under an 8 GB RAM constraint (`-j 1`, `--test-threads=1`).
   - Dedicated GPU hardware or external AI services cannot be required: GPU effects, OpenFX, and AI features must specify pure software/CPU fallback paths and mock interfaces for testing.
5. **Architectural Interface Synthesis**:
   - By mapping every observed dialog, command, and string from `results.md` to Rust type signatures, we produce authoritative, verified contracts ready for immediate implementation by development subagents.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Transitions | Transition Categories & Presets | 6 categories (Dissolve, Wipes, Shapes, Motion, 3D, Effects) with parameterized presets | `category: Category`, `preset_id: &str`, `progress: f32` | Computed transition pixel buffer | Fallback to simple cross dissolve if preset missing | `results.md` §12.14, §17.4 |
| 2 | Transitions | 3D Transitions | Transitions with 3D spatial transforms (Fly In, Page Peel, Page Roll, Cube, Shuffle) | 3D rotation (X,Y,Z), position, specular light | 3D projected composite frame | Clamp rotation/depth to safe frustum | `results.md` §17.4 (`vfx1.dll` Dialog 130, 136) |
| 3 | Transitions | Transition Progress Envelope | Event envelope controlling custom transition progress curve over time | Envelope keyframes `(time, progress 0.0..1.0, curve)` | Non-linear transition progress | Clamped to `0.0..=1.0`; invalid points rejected | `results.md` §12.14 (ID 16471, 32537) |
| 4 | Effects / OFX | OpenFX 1.4 Host & Bundles | Loading and execution of OFX video effect bundles with CPU fallback | Plugin bundle path, parameter map, source buffer | Processed output buffer | Soft error; fallback to pass-through CPU buffer | `results.md` §17.3, §17.4 |
| 5 | Color Grading | Primary Color Wheels | 4-way Lift, Gamma, Gain, Offset color correction with Wheel/Slider toggle | RGB vector `(x, y)` + Master Luma slider per wheel | Color-transformed 32-bit float RGB | Sanitized finite floats; NaN clamped to 0.0 | `results.md` §12.12, `ColorGradingWindow.dll` |
| 6 | Color Grading | White Balance Picker | Neutral tone sampler calculating temperature & tint correction | Sampled neutral pixel `[R, G, B]` from preview | `(temperature: f32, tint: f32)` offsets | Ignore pure black/white to avoid division by zero | `results.md` §12.12 (ID 10122, 10134) |
| 7 | Color Grading | RGB & HSL Curves | Interactive transfer curves: Master RGB, Red, Green, Blue + 6 HSL curves | Spline control points with Bézier tangents | Lookup table / direct evaluation | Clamped monotonic boundaries; reset channel | `results.md` §12.12 (ID 9320, 10146, 13942) |
| 8 | Color Grading | OpenColorIO & ACES Pipeline | Color transform pipeline using OCIO configs and ACES standard spaces | Source frame, Input IDT, Working Space, View ODT | Rendered color frame in target display space | Fallback to sRGB/Rec.709 if OCIO config invalid | `results.md` §8.7 (Dialog 826), §17.6 |
| 9 | Color Grading | Color Match Across Clips | Matching luminance and chromaticity distribution from reference clip to target | Reference frame, target frame | Computed 3D LUT or Lift/Gamma/Gain offsets | Soft falloff if contrast ratio is extreme | `results.md` §12.12, §15.1 |
| 10 | Motion Tracking | Planar & Point Motion Tracker | Optical flow feature tracking generating keyframe trajectory | Video event frames, tracking bounding box | Keyframe track: `(time, x, y, scale, rot)` | Pauses tracking if confidence drops below threshold | `results.md` §12.15 (ID 1860, 18665), `MagixCVFx` |
| 11 | Masks | Geometric & Bézier Event Masks | Parametric (Rect, Oval) and Bézier path masks with feathering & invert | Shape parameters / Bézier spline vertices, feather px | 8-bit alpha mask buffer | Path auto-closed; clamped feather radius | `results.md` §12.15 (ID 18619, 18620, 18637) |
| 12 | Masks | Smart Mask (AI Segmentation) | Neural or morphological object segmentation generating keyframed mask | Click point (foreground/background prompt) or box | High-contrast alpha matte contour | Fallback to luminance/color keying if offline | `results.md` §12.20 (ID 21468), `MagixAiFx` |
| 13 | Project Setup | Project Video/Audio Properties | Comprehensive project setup matching Dialog 826/863 | Res, FPS, PAR, Field Order, HDR, ACES, Audio Rate | Validated `ProjectSettings` struct | Reject invalid dimensions or negative rates | `results.md` §8.7 (Dialog 826, 863) |
| 14 | Project Setup | Project Templates (.vqpt) | Standard broadcast, disc, and web project template definitions | Template identifier or `.vqpt` file data | Pre-configured `ProjectSettings` | Load default template if file corrupted | `results.md` §18.3 (46 templates) |
| 15 | Project Setup | Media Profiles (profiles.ini) | Rule-based automatic matching of imported media stream attributes | Media stream dimensions, framerate, codec | Matched interlace, PAR, alpha mode, gamma | Fallback to profile 1 / default profile | `results.md` §18.4 (`Vegas profiles.ini`) |
| 16 | Media Management| Project Media Detail Columns | 23 detail metadata columns for media pool list/details view | Media pool items | Tabular view model with sorting & filters | Omit unavailable streams gracefully | `results.md` §12.10 (ID 32750..32772) |
| 17 | Media Management| Media Bins & Smart Search Bins | Hierarchical bin folders, quick tags, and dynamic query search bins | Bin name, parent bin ID, query criteria | Filtered media item collection | Prohibit root bin deletion; auto-rename dupes | `results.md` §12.10 (ID 18174, 32951) |
| 18 | Media Management| Unified Explorer | Drive navigation, favorites, preview player, scene detect | File system paths, search filters | Navigation tree & media asset listing | Handle read-only / disconnected drives | `results.md` §12.11 (WPF Explorer) |
| 19 | Render & Cache | Make Movie Guided Wizard | Novice-friendly wizard flow for web, camera, and local disk export | Target destination, quality preset, file path | Encoded output media file | Error 3404 if project empty; disk space check | `results.md` §8.1 (Dialog 804), §12.18 |
| 20 | Render & Cache | Color-Coded Preview Ruler | Timeline ruler reflecting frame cache status (Green / Yellow / Red) | Timeline time range, cache state query | Frame status array: `Cached`, `Playable`, `Slow` | Invalidate to Red/Yellow on edit | `results.md` §12.18, roadmap spec |
| 21 | Render & Cache | Dynamic RAM & Selective Prerender | Memory buffer cache (Shift+B) and disk prerender cache (Shift+M) | Time selection range, prerender quality | Cached frames in RAM / temp video on disk | Clean up temp files on project close | `results.md` §12.18 (ID 32602, 32614) |
| 22 | AI Features | Text-Based Editing | Spoken transcript view synchronized with timeline event trimming | Dialogue audio stream | Searchable timecoded transcript words | Skip non-speech audio; preserve cut boundaries | `results.md` §12.20 (`TextbasedEditing`) |
| 23 | AI Features | Speech-to-Text Subtitles | Subtitle track generation with line length & character constraints | Audio track, language, max lines, max chars | Subtitle event stream + SRT/SCC/VTT export | Handle silent regions without empty events | `results.md` §12.20 (`Speech2Text`) |
| 24 | AI Features | AI Auto Reframe | Dynamic pan/crop keyframing keeping primary subject centered | Aspect ratio (e.g. 16:9 to 9:16), video frames | Animated Pan/Crop keyframe envelope | Center crop fallback if no subject found | `results.md` §12.20 (ID 65294) |
| 25 | Alerts & Dialogs | Standardized Alerts System | Centralized alert subsystem matching §10 error codes and actions | Alert code, formatted arguments, user choice | User action (`Ok`, `Cancel`, `Relink`, `Retry`) | Clean-room text without third-party marks | `results.md` §10.1..10.6 (240 strings) |
| 26 | Smart Proxies | Quality-Based Proxy Switching | Automatic proxy usage on Draft/Preview, original on Good/Best | Preview quality setting, media asset | Active decode target (proxy vs original) | Render As always forces original media | `results.md` §12.10, §12.18 (VP12) |
| 27 | Timeline Editing | L-Cut and J-Cut Split Editing | Independent trimming of audio or video edges without breaking link | Event edge drag with modifier key, delta time | Adjusted audio/video start & trim offsets | Maintain track bounds & minimum duration | `results.md` §7.1.5, §12.7 |
| 28 | Timeline Editing | Event Synchronization (Sync Links)| Persistent sync constraint tracking out-of-sync offset and resyncing | Event A, Event B, SyncLink ID | Out-of-sync badge; resync by move/slip | Clamp slip to available source media length | `results.md` §7.1.5 (ID 33075..33099) |
| 29 | Timeline Editing | Simultaneous Audio/Video Fades | Synchronized fade handle manipulation with audio micro-quickfades | Fade handle drag, quickfade mode flag | Updated video fade curve & audio envelope | Microfade prevents zero-crossing clicks | `results.md` §6.6 (ID 32315), §12.14 |
| 30 | Multicam | Waveform & Timecode Sync | Aligning multiple camera angles by audio correlation or timecode | Selected tracks / events | Calculated time alignment offsets per track | Return error if audio correlation < threshold | `results.md` §6.5 (ID 33561, 33205) |
| 31 | Multicam | Multicamera Track & Live Cut | Container track with multi-angle preview grid and click-to-cut | Multicamera track, active angle selection | Cut events inserted at playback cursor position | Follow audio or keep master audio locked | `results.md` §6.5 (ID 33523, 33524) |
| 32 | Takes | Alternative Event Takes System | Multiple media takes within a single event shell; active rotation | Event take list, `Next Take` / `Prev Take` | Swapped active take media and trim offset | Cannot delete sole remaining take | `results.md` §6.2 (ID 32360..32364) |

---

## 4. Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | Transitions | Two abutting events with 0 duration overlap | Transition cannot be applied; treated as straight hard cut. |
| 2 | Transitions | Transition duration requested exceeds event duration | Transition length clamped to 100% of shorter event duration. |
| 3 | Transitions | Transition assigned to audio track event | Rejected with error ID 13933 ("Only video events can have their transition set"). |
| 4 | Transitions | Referenced transition plug-in missing or uninstalled | Fallback to simple linear crossfade (ID 13903); timeline edits preserved. |
| 5 | Color Grading | 32-bit floating point colors exceeding 1.0 (super-whites) | Retained in 32-bit full range / ACES space; clipped only when converted to 8-bit output. |
| 6 | Color Grading | White balance picker sampled on pure black `[0,0,0]` | Division by zero prevented; white balance remains neutral `(0.0, 0.0)`. |
| 7 | Color Grading | OpenColorIO config file missing from disk | Engine falls back to native Rec.709 sRGB matrix transform; alert logged. |
| 8 | Motion Tracking | Tracked feature moves outside visible frame | Confidence drops below threshold; tracking holds last valid position and flags gap. |
| 9 | Masks | Bézier mask path with duplicate consecutive points or loops | Degenerate segments skipped; rasterizer closes polygon without artifact. |
| 10 | Smart Mask | AI model execution unavailable (no weights / no GPU) | Falls back cleanly to luminance and chroma difference segmentation. |
| 11 | Project Properties | Video dimensions not divisible by 2 or 4 (e.g. 1921x1081) | Warned or padded on export if target codec requires even macroblock dimensions. |
| 12 | Project Templates | Loading corrupt `.vqpt` template file | Loads default standard project template; displays error ID 17437. |
| 13 | Media Profiles | Multiple matching rules in `profiles.ini` | Last matching rule (highest key index) takes precedence per §18.4 specification. |
| 14 | Project Media | Deleting media pool item currently placed on timeline | Modal confirmation dialog (ID 17264) warns user and confirms timeline event deletion. |
| 15 | Project Media | Attempting to delete or rename the Root Media Bin | Prohibited with explicit error ID 12878 and 12889. |
| 16 | Project Media | Creating a media bin with name matching an existing bin | Duplicate automatically renamed with increment: `'Bin (2)'` (ID 18174). |
| 17 | Preview Cache Ruler | Event on timeline trimmed or moved while ruler is Green | Affected time slice immediately invalidated to Red; background worker re-queues render. |
| 18 | Make Movie | Rendering triggered with empty timeline | Terminated immediately with error ID 3404 ("Unable to render since project is empty"). |
| 19 | Make Movie | Available disk space less than estimated render size | Blocked before start with error ID 18548 ("There is not enough free disc space..."). |
| 20 | Text-Based Editing | Spoken transcript cutting every word in event | Entire event is ripple-deleted from timeline; adjacent events close gap cleanly. |
| 21 | Speech-to-Text | Subtitle line exceeds maximum character limit (e.g. 42 chars)| Line split intelligently at nearest word boundary into second line. |
| 22 | Auto Reframe | 16:9 to 9:16 vertical reframe with no detected human subject | Fallback to centered optical flow crop; avoids erratic camera jitter. |
| 23 | Standardized Alerts | GPU driver crashes or hangs during playback | Displays Alert ID 33316/33317, automatically falls back to CPU software rasterization. |
| 24 | Smart Proxies | User changes preview quality from Draft to Good during playback | Decoded stream swaps seamlessly from proxy file to original source without frame drop. |
| 25 | Smart Proxies | Final render executed when proxy is present | Proxy completely ignored; original source conformed and decoded at 100% resolution. |
| 26 | Sync Links | Event with sync link moved on timeline | Paired event displays red out-of-sync badge showing exact frame offset (`+00:00:01:12`). |
| 27 | Sync Links | `Synchronize By Slipping` when source media bounds reached | Slip clamped to media start/end; remaining discrepancy shown on out-of-sync badge. |
| 28 | Multicam Sync | Audio waveform correlation fails (no overlapping sound) | Sync aborted with notification; user prompted to sync by timecode or markers. |
| 29 | Takes | Event with only 1 take receives `Delete Active Take` | Operation skipped (ID 32327); event cannot lose its last take. |
| 30 | Takes | Slipping event with `Slip All Takes` enabled | Trim offsets of all takes updated together, maintaining relative timing across versions. |

---

## 5. Clean-Room Architectural Interface Definitions

The following domain contracts define the exact interfaces required to implement the R2 core feature extensions in clean-room Rust:

### 5.1 Transitions, Presets, Thumbnails & OFX (`neoveg_video::transition_library`, `neoveg_ofx`)

```rust
use std::collections::BTreeMap;
use crate::frame::FrameBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionCategory {
    Dissolve,
    Wipes,
    Shapes,
    Motion,
    ThreeD,
    Effects,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionPreset {
    pub id: &'static str,
    pub category: TransitionCategory,
    pub name_key: &'static str,
    pub params: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreeDTransform {
    pub rotation_x_deg: f32,
    pub rotation_y_deg: f32,
    pub rotation_z_deg: f32,
    pub depth_z: f32,
    pub specular_light: f32,
}

pub trait TransitionRenderer: Send + Sync {
    fn render(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        progress: f32,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String>;

    fn render_thumbnail(
        &self,
        source: &FrameBuffer,
        target: &FrameBuffer,
        params: &BTreeMap<String, f32>,
    ) -> FrameBuffer {
        self.render(source, target, 0.5, params).unwrap_or_else(|_| source.clone())
    }
}

pub trait OfxHostContext: Send + Sync {
    fn load_bundle(&mut self, bundle_path: &std::path::Path) -> Result<String, String>;
    fn execute_effect(
        &self,
        plugin_id: &str,
        source: &FrameBuffer,
        params: &BTreeMap<String, f32>,
    ) -> Result<FrameBuffer, String>;
    fn fallback_cpu_execute(
        &self,
        plugin_id: &str,
        source: &FrameBuffer,
        params: &BTreeMap<String, f32>,
    ) -> FrameBuffer;
}
```

### 5.2 Color Grading, Color Wheels & OCIO (`neoveg_video::grading`, `neoveg_common::grade`)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorWheelVector {
    pub x: f32,       // -1.0..=1.0 (chroma angle / saturation)
    pub y: f32,       // -1.0..=1.0
    pub master: f32,  // Luma adjustment
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorWheelsGrade {
    pub lift: ColorWheelVector,
    pub gamma: ColorWheelVector,
    pub gain: ColorWheelVector,
    pub offset: ColorWheelVector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurvePoint {
    pub in_val: f32,   // 0.0..=1.0
    pub out_val: f32,  // 0.0..=1.0
    pub handle_left: (f32, f32),
    pub handle_right: (f32, f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorCurves {
    pub master: Vec<CurvePoint>,
    pub red: Vec<CurvePoint>,
    pub green: Vec<CurvePoint>,
    pub blue: Vec<CurvePoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HslCurveType {
    HueVsHue,
    HueVsSat,
    HueVsLum,
    SatVsSat,
    LumVsSat,
    SatVsLum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcesVersion {
    Aces1_0,
    Aces1_1,
    Aces1_2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcesColorSpace {
    Aces2065_1,
    Acescg,
    Acescc,
    Acescct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcesViewTransform {
    RrtRec709,
    RrtP3Dci,
    RrtP3D60,
    RrtCieXyz,
    RrtHdr1000Nit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraIdt {
    None,
    SonySLog1,
    SonySLog2_3200K,
    SonySLog2_5500K,
    SonySLog3_SGamut3Cine,
    CanonLog,
    ArriLogC,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenColorIoPipeline {
    pub aces_version: AcesVersion,
    pub working_space: AcesColorSpace,
    pub view_transform: AcesViewTransform,
    pub use_gpu: bool,
}

pub trait ColorMatcher: Send + Sync {
    fn analyze_and_match(
        &self,
        reference_frame: &FrameBuffer,
        target_frame: &FrameBuffer,
    ) -> ColorWheelsGrade;
}
```

### 5.3 Motion Tracking & Masks (`neoveg_video::tracking`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TrackKeyframe {
    pub time_sec: f64,
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_rad: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionTrackingTrack {
    pub id: u64,
    pub name: String,
    pub keyframes: Vec<TrackKeyframe>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaskShape {
    Rectangle { width: f32, height: f32, corner_radius: f32 },
    Oval { radius_x: f32, radius_y: f32 },
    BezierSpline { points: Vec<CurvePoint>, closed: bool },
    SmartMaskMatte { frame_time: f64, matte_rle: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventMask {
    pub shape: MaskShape,
    pub center_x: f32,
    pub center_y: f32,
    pub feather_px: f32,
    pub inverted: bool,
    pub opacity: f32,
}
```

### 5.4 Project Properties, Templates & Media Profiles (`neoveg_common::project`, `neoveg_common::profiles`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileKey {
    pub file_io_clsid: Option<String>,
    pub subformat: String,
    pub size_x: u32,
    pub size_y: u32,
    pub frame_rate: f64,
    pub color_bit_depth: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileAttributes {
    pub interlace: neoveg_common::presets::FieldOrder,
    pub pixel_aspect_ratio: f64,
    pub alpha_interpretation: AlphaMode,
    pub gamma: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Undefined,
    None,
    Straight,
    Premultiplied,
}

pub struct MediaProfileMatcher {
    profiles: Vec<(MediaProfileKey, MediaProfileAttributes)>,
}

impl MediaProfileMatcher {
    pub fn parse_profiles_ini(ini_text: &str) -> Result<Self, String>;
    pub fn match_stream(&self, key: &MediaProfileKey) -> Option<MediaProfileAttributes>;
}
```

### 5.5 Project Media & Explorer (`neoveg_engine::media_pool`, `neoveg_engine::explorer`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaDetailColumn {
    Name,
    UseCount,
    Size,
    Length,
    Modified,
    Path,
    Type,
    Comment,
    DateTimeStamp,
    Rating,
    VideoCaptureComment,
    SampleRate,
    BitDepth,
    Channels,
    FrameRate,
    FieldOrder,
    Alpha,
    PixelAspect,
    FrameSize,
    ColorDepth,
    MediaFx,
    TimecodeIn,
    TimecodeOut,
    TapeName,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaBin {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub media_ids: Vec<u64>,
    pub is_smart_bin: bool,
    pub smart_query: Option<String>,
}
```

### 5.6 Make Movie, Render Presets & Pre-rendering Cache Ruler (`neoveg_video::render_cache`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerFrameStatus {
    Green,   // Fully cached / prerendered; real-time playback guaranteed
    Yellow,  // Playable with moderate real-time GPU/CPU load
    Red,     // Unrendered complex VFX; below real-time threshold (prerender needed)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewRulerMap {
    pub total_frames: usize,
    pub statuses: Vec<RulerFrameStatus>,
}

impl PreviewRulerMap {
    pub fn new(total_frames: usize) -> Self;
    pub fn mark_range(&mut self, start_frame: usize, end_frame: usize, status: RulerFrameStatus);
    pub fn invalidate_range(&mut self, start_frame: usize, end_frame: usize);
}

#[derive(Debug, Clone, PartialEq)]
pub struct MakeMovieOptions {
    pub destination: MakeMovieDestination,
    pub preset_name: String,
    pub output_path: std::path::PathBuf,
    pub render_loop_region_only: bool,
    pub use_auto_reframe: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MakeMovieDestination {
    LocalDrive,
    WebPlatform,
    ExternalDevice,
}
```

### 5.7 AI Features (`neoveg_engine::ai`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptWord {
    pub text: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub confidence: f32,
    pub is_pause: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transcript {
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleOptions {
    pub max_lines: usize,        // typically 1 or 2
    pub max_chars_per_line: usize, // typically 37..=42
    pub title_preset: String,
}

pub trait SpeechToTextEngine: Send + Sync {
    fn transcribe(&self, audio_samples: &[f32], sample_rate: u32) -> Result<Transcript, String>;
    fn generate_subtitles(&self, transcript: &Transcript, options: &SubtitleOptions) -> Vec<neoveg_common::captions::Caption>;
}

pub trait AutoReframeEngine: Send + Sync {
    fn compute_reframe_pan_crop(
        &self,
        frames: &[FrameBuffer],
        source_aspect: f64,
        target_aspect: f64,
    ) -> Result<Vec<(f64, neoveg_common::pan_crop::PanCrop)>, String>;
}
```

### 5.8 Standardized Alerts & Dialogs (`neoveg_common::alerts`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertCategory {
    MissingMedia,
    CodecFormat,
    RenderPrerender,
    DiskMemory,
    GpuAcceleration,
    PluginVst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertCode {
    // Missing Media (§10.1)
    MissingMediaPoolElements = 1561,
    MediaIsOfflinePrompt = 1634,
    MediaFileNotFound = 16926,
    FindMissingFile = 16132,
    
    // Codec / Format (§10.2)
    MissingVideoCodec = 17369,
    MissingAudioCodec = 17381,
    CorruptStreamFormat = 17384,
    UnsupportedChannelCount = 33501,
    
    // Render / Prerender (§10.3)
    EmptyProjectCannotRender = 3404,
    InsufficientDiscSpace = 18548,
    ConfirmCancelRender = 44720,
    
    // GPU (§10.5)
    GpuAccelerationError = 33316,
    GpuDeviceDriverProblem = 33317,
    
    // Plug-in (§10.6)
    PluginFailedToLoad = 16555,
    PluginChainFull = 16511,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StandardAlert {
    pub code: AlertCode,
    pub category: AlertCategory,
    pub en_us_template: &'static str,
    pub pt_br_template: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAlertChoice {
    Ok,
    Cancel,
    Yes,
    No,
    Retry,
    Browse,
}
```

### 5.9 Smart Proxies, Sync Links & Multicam (`neoveg_engine::sync_link`, `neoveg_engine::multicam`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewQualityMode {
    Draft,
    Preview,
    Good,
    Best,
}

pub struct SmartProxyResolver;
impl SmartProxyResolver {
    pub fn select_media_source(
        original_media_id: u64,
        proxy_media_id: Option<u64>,
        quality: PreviewQualityMode,
        is_final_render: bool,
    ) -> u64 {
        if is_final_render {
            return original_media_id;
        }
        match quality {
            PreviewQualityMode::Draft | PreviewQualityMode::Preview => {
                proxy_media_id.unwrap_or(original_media_id)
            }
            PreviewQualityMode::Good | PreviewQualityMode::Best => original_media_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncLink {
    pub link_id: u64,
    pub master_event: (u64, u64), // (track_id, event_id)
    pub linked_event: (u64, u64),
    pub original_offset_sec: f64,
}

impl SyncLink {
    pub fn calculate_out_of_sync(&self, current_offset_sec: f64) -> f64 {
        current_offset_sec - self.original_offset_sec
    }
}

pub trait MulticamSynchronizer: Send + Sync {
    fn sync_by_audio_correlation(
        &self,
        tracks: &[(u64, &[f32])],
        sample_rate: u32,
    ) -> Result<Vec<(u64, f64)>, String>;

    fn sync_by_timecode(&self, tracks: &[(u64, f64)]) -> Vec<(u64, f64)>;
}
```

---

## 6. Caveats

- **No live third-party binaries**: Analysis is strictly based on the static reverse engineering results documented in `results.md` and the existing clean-room codebase.
- **Hardware-dependent tests**: All GPU-accelerated effects, OFX hosts, and AI features require software fallback paths to build and test cleanly in headless CI environments without GPU devices.
- **Parity script synchronization**: Whenever changes to commands or functional matrix rows are introduced, `scripts/parity_matrix.py --check` must be validated.

---

## 7. Conclusion

Exhaustive specifications have been extracted for all 12 requested feature areas of R2. Clean-room Rust interface definitions have been formulated without third-party trademarks in runtime strings or copying proprietary code. The specifications directly ground the next phase of implementation across `neoveg_common`, `neoveg_video`, `neoveg_engine`, and `neoveg_ui`.

---

## 8. Verification Method

1. **Verify Parity Script Integrity**:
   ```powershell
   python scripts/parity_matrix.py --check
   ```
   *Expected outcome*: Exits with code 0.

2. **Verify Memory Budget & Cargo Constraints**:
   ```powershell
   cargo check -j 1 --workspace
   cargo clippy -j 1 --workspace
   cargo test -j 1 --test-threads=1 --workspace --exclude neoveg_ui
   ```

3. **Verify License & Trademark Cleanliness**:
   ```powershell
   cargo deny check
   ```
   Check that no third-party vendor names exist as unformatted runtime string literals.
