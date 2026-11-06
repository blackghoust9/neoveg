# Milestone 5 (Features 24–34) Specification Mining Report

**Agent**: `spec_miner_m5_1`  
**Milestone**: Milestone 5 (Project Properties, Media Pool, Explorer, Render Caching & Alerts)  
**Date**: 2026-11-06  
**Status**: COMPLETE (Hard Handoff)

---

## 1. Observation

Direct observations and evidence extracted from the authoritative references:
1. **`results.md`**:
   - **§8.1 Make Movie / Render / Media Recording** (lines 3580–4030): Dialog 804 (Make Movie - Select Destination), Dialog 885 (Make Movie - Select Destination with Disc), Dialog 881 (Make Movie - Render Settings), Dialog 861 (Make Movie - Completed), Dialog 888/889 (Burn a Disc), Dialog 890 (Save to Camera/Portable Device), Dialog 891 (Burn Blu-ray Disc).
   - **§8.7 Project Properties Pages** (lines 6376–6580): Dialog 807 (Summary), Dialog 817 (Ruler), Dialog 826 (Video), Dialog 863 (Audio), Dialog 892 (Folders), Dialog 913 (Advanced).
   - **§10 Standardized Alerts, Messages & Errors** (lines 6797–7890): 240 resource strings categorized into §10.1 Offline Media (IDs 1561, 1634, 16058, 16132, 16926, 17250, 17630), §10.2 Codec/Format (IDs 17369, 17370, 17381, 17384, 33501), §10.3 Render/Prerender/Templates (IDs 1578, 1580, 1583, 1585, 17364, 17365, 17437, 3404, 44720), §10.4 Disk Space/Memory (IDs 16227, 18548, 17603), §10.5 GPU Acceleration (IDs 33316, 33317), §10.6 Plugins (IDs 16020, 16511, 16555, 16560), §10.13 Generic Dialogs (Dialog 845 "Missing Media", Dialog 846 "Search for Missing Files", Dialog 857 "Resolve Media File Error", Dialog 834 "Clean Up Prerendered Video", Dialog 861 "Make Movie - Completed").
   - **§12.10 Project Media Window** (lines 12735–12895): 23 standard metadata columns (IDs 32750–32772, 32119–32120), View modes (Thumbnails ID 32902, List ID 32903, Details ID 32904), Bins commands (IDs 1804–1806, 18173, 18174, 12878, 12889), Audio proxy building (IDs 16362, 17280), Video proxy building (IDs 17882, 32841, 33628).
   - **§12.11 Unified Explorer** (lines 12898–13020): `UnifiedExplorer.dll`, Favorites (ID 16963, 32058), Media preview audition, Automatic Scene Detection (IDs 18675–18677, 12882), suppression of companion files (`.sfk`, `.sfvp0`).
   - **§18.3 Project Templates (.vqpt)** (line 20092): 46 standard industry preset templates.
   - **§18.4 Video Media Profiles (`profiles.ini`)** (lines 20098–20197): Complete INI syntax, Key format, Attribute format, reverse-order rule evaluation, wildcard definitions (`0` and `"None"`), and 13 stock profiles.
2. **`manual_vegaspro21_en.md`**:
   - **Setting Project Properties (pp. 91–106, lines 3785–4647)**: Video tab (Resolution, Frame Rate, Field Order, Pixel Aspect Ratio, Output Rotation, Pixel Format [Legacy 8-bit Video, 8-bit Full Range, 32-bit Float Video, 32-bit Float Full Range], Compositing Gamma [1.000 Linear, 2.222 Video], ACES/OCIO Color Spaces, View Transforms, LMT Blue Light Artifact Fix, Full-resolution rendering quality [Draft, Preview, Good, Best], Motion Blur Types [Gaussian, Pyramid, Box, and asymmetric variants], Deinterlace Methods [None, Blend Fields, Interpolate, Smart Adaptive GPU], Resample Modes [Disable Resample, Frame Blend, Optical Flow], Prerender folder overrides).
   - **Audio tab (pp. 100–103, lines 4287–4430)**: Master bus mode (Stereo vs 5.1 Surround), Number of stereo busses (0..26), Sample rate (8 kHz to 192 kHz), Bit depth (16, 24, 32-bit float), Resample & stretch quality, LFE Low-pass filter (Cutoff frequency & filter quality), Recorded files folder.
   - **Ruler tab (pp. 103–105, lines 4433–4510, and pp. 81–84, lines 3383–3610)**: 16 Ruler time formats, Ruler start time offset, Measures & Beats tempo (BPM, beats per measure, note that gets one beat).
   - **Summary tab (p. 105, line 4511)**: Title, Artist, Engineer, Copyright, Comments, Start all new projects with these settings.
   - **Dynamic RAM Preview Cache (pp. 207–208, lines 8823–8865)**: `Tools | Build Dynamic RAM Preview` (`Shift+B`), `Preferences -> Video -> Dynamic RAM Preview (max) (MB)`, selection auto-adjustment when cache is full, `Tools | Clear Dynamic RAM Preview`.
   - **Selective Prerender (pp. 213–215, lines 9077–9140)**: `Tools | Selectively Prerender Video` (`Shift+M`), discrete chunks of ≤ 300 frames (~40 MB per chunk), non-destructive timeline invalidation, `Tools | Clean Up Prerendered Video` (Dialog 834).
   - **Smart Proxies (pp. 212–213, lines 9049–9070)**: Companion proxy file `.sfvp0`, automatic generation on 4K import, automatic switching (Draft/Preview = proxy; Good/Best = original), conforming to original full-resolution media for final rendering.
   - **Sorting media with bins (pp. 169–172, lines 3843–3896)**: Root "All Media", virtual bins, Quick Tags bins, Smart Search Bins with dynamic query evaluation.
3. **`VEGAS_Pro_18_EN_Keyboard_shortcuts.md`**:
   - Lines 41, 43, 373, 379: `Alt+Enter` (Project Properties), `Alt+1` (Explorer), `Alt+5` (Project Media), `Shift+B` (Build Dynamic RAM Preview), `Shift+M` (Selectively Prerender Video).

---

## 2. Logic Chain

1. **Project Properties Model (Feature 24)**:
   - Observables: Dialogs 826, 863, 817, 807, 892, 913 in `results.md` and pp. 91–106 of `manual_vegaspro21_en.md`.
   - Inference: `ProjectSettings` in `neoveg_common::project` must contain exhaustive fields matching these dialog tabs. Width and height must be even integers clamped to `16..=16384` (`SIZE_RANGE`). The four pixel formats (`Legacy8Video`, `Bit8Full`, `Float32Video`, `Float32Full`) dictate color processing: compositing gamma is selectable only in `Float32Full` (defaulting to 1.0 Linear, else 2.222 Video). Audio bit depth must support 16, 24, and 32-bit float; sample rates support 8 kHz to 192 kHz. When Master Bus is 5.1 Surround, LFE filter parameters (cutoff frequency 10–1000 Hz, quality Preview/Good/Best) become active.
2. **Project Templates Subsystem (Feature 25)**:
   - Observables: `results.md` §18.3 lists 46 `.vqpt` files; manual pp. 91–92, lines 3825–3847 describe custom template creation; error IDs 1580, 1585, 17364, 17365 protect built-in templates.
   - Inference: `.vqpt` is a serialized project preset containing video and audio parameters. Stock templates cover broadcast (NTSC/PAL SD, HDV, AVCHD, Blu-ray), cinema (2K/4K DCI, Scope), internet/web (360p, 480p, 720p, 1080p, 4K UHD), and social formats (vertical 9:16 1080x1920, square 1:1 1080x1080). Built-in templates have an immutable flag preventing overwrite or deletion. User templates can be added, updated, or removed with file persistence.
3. **Media Profiles Rule Engine (Feature 26)**:
   - Observables: `results.md` §18.4 details `profiles.ini` syntax and matching rules.
   - Inference: Rules match imported video streams against consecutive `KeyN` and `AttributesN` definitions. Keys contain `(File IO CLSID, Subformat, Width, Height, FPS, BitDepth)` and Attributes contain `(FieldOrder, PixelAspect, AlphaInterpretation, Gamma)`. Evaluation proceeds in **reverse order** from highest key down to 1 (later rules override earlier ones). Wildcard `0` or `"None"` matches any value. An epsilon of 0.005 is applied for floating-point frame rate comparison.
4. **Project Media Window & 23 Metadata Columns (Feature 27)**:
   - Observables: `results.md` §12.10 lines 12789–12824 define exactly 23 detail columns (Name, Use Count, Size, Length, Modified, Path, Type, Comment, Date/Time Stamp, Rating, Video Capture Comment, Sample Rate, Bit Depth, Channels, Frame Rate, Field Order, Alpha, Pixel Aspect, Frame Size, Color Depth, Media FX, Timecode In, Timecode Out).
   - Inference: Media pool table view must provide column visibility toggles, multi-column sorting (numeric vs alphanumeric), search filtering, replace media (preserving timeline trims/envelopes), and offline media tracking. If media in use on timeline is deleted, alert ID 17264 must confirm removal from timeline.
5. **Media Bins & Smart Search Bins (Feature 28)**:
   - Observables: Manual pp. 169–172 and string IDs 1804–1806, 18173–18174, 12878, 12889.
   - Inference: The bin tree has an immutable root "All Media" (ID 12878 prevents removal, ID 12889 prevents rename). Virtual bins can be arbitrarily nested. Media type bins (Audio, Video, Stills) and Quick Tag bins are auto-populated. The Search Media Bins dialog creates either a static bin ("Save as Bin") or a dynamic query bin ("Save as Smart Bin") which updates reactively as new media is imported.
6. **Unified File Explorer (Feature 29)**:
   - Observables: `results.md` §12.11 and manual p. 180.
   - Inference: Tree navigation provides access to local, optical, and network drives. Favorites pane bookmarks user directories. Companion files (`.sfk` waveform peaks, `.sfvp0` video proxies) are hidden. Transport controls provide auto-play preview audition. Scene detection analyzes timestamp breaks or optical frame differences and splits clips into subclips or timeline events.
7. **Make Movie Guided Export Wizard (Feature 30)**:
   - Observables: Dialogs 804, 885, 881, 861 in `results.md` §8.1.
   - Inference: Wizard simplifies export into 3 distinct steps: Step 1 Select Destination (Web/YouTube/Vimeo, Portable Device/Camera, Local Hard Drive, Disc Authoring); Step 2 Configure Render Settings (File path, Preset template, Loop region only, Stretch/Letterbox, Space estimation check); Step 3 Completion (Dialog 861: Play File, Explore Folder, Send to Disc Authoring).
8. **Color-Coded Preview Cache Ruler (Feature 31)**:
   - Observables: Manual pp. 207–215 and R2.6 in `ORIGINAL_REQUEST.md`.
   - Inference: Timeline displays a three-state color-coded status bar: Green = cached in RAM or prerendered to disk; Yellow = playable real-time without caching; Red = complex unrendered frames that will drop during playback. Any event edit, effect addition, or envelope tweak immediately invalidates the cache (Green -> Red).
9. **Dynamic RAM & Selective Prerender (Feature 32)**:
   - Observables: Manual lines 8823–8865 (`Shift+B`) and lines 9077–9140 (`Shift+M`).
   - Inference: Dynamic RAM cache renders selection into a circular in-memory buffer (`Preferences -> Video -> Dynamic RAM Preview Max MB`). If selection exceeds RAM capacity, the selection loop automatically truncates to the cached duration. Clicking the timeline cancels rendering. Selective Prerender (`Shift+M`) renders to disk in discrete segments of ≤ 300 frames (~40 MB per file), localizing timeline invalidation to only affected segments. Dialog 834 cleans up inactive or selected prerender files.
10. **Standardized Alerts Subsystem (Feature 33)**:
    - Observables: `results.md` §10.1–§10.6, §10.13, Dialogs 845, 846, 857.
    - Inference: Centralized alert dialogs must strictly implement the standard action pathways:
      - Missing Media: Search for missing file (Dialog 846), Specify new location/replacement file (file browser), Ignore missing file, Ignore all missing files.
      - Missing Codec: Display stream index, error reason, option to substitute file.
      - Disk Full: Display estimated size vs free storage space, browse alternate destination.
      - GPU Acceleration Crash: Alert ID 33316/33317, fallback to CPU processing without crash.
      - Plugin Crash: Disable failed plugin in chain, isolate host process.
11. **Smart Proxies & Quality Switching (Feature 34)**:
    - Observables: Manual lines 9049–9070 (VP12 proxy semantics).
    - Inference: Video proxies are generated automatically for 4K media or manually via context menu. During playback and editing: Draft and Preview quality modes automatically decode from the proxy file; Good and Best quality modes decode from the original full-resolution media. When rendering (Make Movie or Render As), original media is always conformed, guaranteeing master export quality.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 24.1 | Project Properties | Video Configuration | Width, height, FPS, pixel aspect ratio, field order, output rotation, deinterlace, resample mode, motion blur | Numeric dimensions, enums | Configured video canvas parameters | Clamped to even `16..=16384`; invalid values reset to defaults | `results.md` Dialog 826; manual pp. 91–98 |
| 24.2 | Project Properties | Pixel Format & Gamma | 8-bit video, 8-bit full range, 32-bit float video, 32-bit float full range; compositing gamma 1.0 Linear / 2.222 Video | `PixelFormat`, `CompositingGamma` | Render pipeline color math configuration | Gamma choice disabled unless `Float32Full` | `manual_vegaspro21_en.md` lines 3971–4023 |
| 24.3 | Project Properties | OCIO & ACES Pipeline | ACES version (1.0..2.0), ACES color space, View Transform, and LMT Blue Light Artifact Fix | Enums, string identifiers | Color transform matrices and LUT pipelines | LMT disabled unless 32-bit full range or HDR mode | `results.md` Dialog 826; manual lines 4047–4074 |
| 24.4 | Project Properties | Audio Configuration | Master bus mode (Stereo vs 5.1 Surround), stereo busses (0..26), sample rate (8–192 kHz), bit depth (16, 24, 32-bit float), resample quality | Audio bus and format parameters | Audio engine sample pipeline settings | Invalid sample rate/bit depth clamped to standard supported | `results.md` Dialog 863; manual pp. 100–103 |
| 24.5 | Project Properties | LFE Low-Pass Filter | Low-pass filter for 5.1 surround projects; cutoff frequency (10–1000 Hz, default 80 Hz) and quality | Checkbox, frequency, filter quality | Audio sub-bass low-pass filtering on LFE track/bus | Disabled in Stereo master bus mode | `results.md` Dialog 863; manual lines 4371–4395 |
| 24.6 | Project Properties | Ruler Time Formats | 16 time ruler display formats: Samples, Time, Seconds, Time & Frames, Absolute Frames, Measures & Beats, 16/35mm Film Feet, SMPTE (23.976, 24, 25, 29.97ND, 29.97DF, 30), Audio CD | `RulerFormat`, `ruler_start` offset, BPM, beats/measure, note value | Formatted timeline ruler timestamps | Audio CD format disallows start time offset | `results.md` Dialog 817; manual lines 3383–3580 |
| 24.7 | Project Properties | Project Metadata & Folders | Title, Artist, Engineer, Copyright, Comments, Recorded folder, Prerender folder, "Start all new projects with these settings" | String metadata, directory paths, bool | Persisted project header and working folder paths | Read-only or invalid folder prompts folder browse (ID 3917, 16149) | `results.md` Dialogs 807, 892 |
| 25.1 | Project Templates | Preset Storage (.vqpt) | Serialized video/audio project templates representing standard production formats | Preset name, `ProjectSettings` struct | `.vqpt` file on disk, added to template dropdown | Unreadable template triggers error ID 17437 | `results.md` §18.3; manual line 3831 |
| 25.2 | Project Templates | Stock Industry Presets | ~30 to 46 standard templates across 1080p, 4K UHD, 720p, vertical 9:16 social, cinema 2K/4K DCI, SD broadcast, and disc | Preset identifier selection | Prepopulated `ProjectSettings` | Built-in presets cannot be overwritten (ID 1580, 17365) or deleted (ID 1585, 17364) | `results.md` §18.3; `presets.rs` lines 608–720 |
| 25.3 | Project Templates | Custom Template Management | Create, name, save, overwrite, and delete custom user presets | Template name string, settings | Updated user template file list | Empty name rejected (ID 1578); existing user template prompts overwrite (ID 1583) | `manual_vegaspro21_en.md` lines 3831–3841 |
| 26.1 | Media Profiles | INI Rule Parser | Parses `[Video Media Profiles]` section in `profiles.ini` into consecutively numbered `KeyN` and `AttributesN` pairs | `profiles.ini` file stream | Structured rule catalog | Stops at first gap in numbering; malformed line skipped | `results.md` §18.4 lines 20098–20197 |
| 26.2 | Media Profiles | Reverse-Order Matcher | Matches incoming video streams (CLSID, subformat, width, height, fps, bit depth) against rules from highest N down to 1 | Media stream metadata | Matched attributes: FieldOrder, PixelAspect, AlphaInterpretation, Gamma | Epsilon tolerance 0.005 for FPS; unmatched stream uses autodetect | `results.md` §18.4 lines 20148–20150 |
| 26.3 | Media Profiles | Wildcard Evaluation | Values of `0` or `"None"` in key act as universal wildcards matching any stream attribute | Numeric/string key fields | Wildcard match condition | None; unconstrained match allows broad fallbacks | `results.md` §18.4 lines 20145–20146 |
| 27.1 | Project Media | 23 Detail Columns | Full tabular display of 23 standardized metadata columns: Name, Use Count, Size, Length, Modified, Path, Type, Comment, Timestamp, Rating, Capture Comment, Sample Rate, Bit Depth, Channels, FPS, Field Order, Alpha, PAR, Frame Size, Color Depth, Media FX, Timecode In, Timecode Out | Media pool collection | Formatted interactive multi-column table | Missing metadata displays blank or "(Undefined)" | `results.md` §12.10 lines 12789–12824 |
| 27.2 | Project Media | View Modes & Filtering | Switch between Thumbnails (with hover scrub), List, and Details; real-time search filtering text box | View mode enum, search string | Filtered and formatted media pool UI | No matching items displays empty pool view | `results.md` lines 12828–12830; manual pp. 167–173 |
| 27.3 | Project Media | Media Replace & Relink | Replace media with alternate file on disk (preserving timeline event trims/envelopes) and relink offline files | Selected media ID, replacement path | Updated timeline references and media metadata | Format mismatch warns; offline media marked red | `results.md` lines 12827, 12876; manual p. 173 |
| 27.4 | Project Media | Media Removal Protection | Remove media from project pool; warns if media is actively referenced on timeline | Selected media ID | Deleted media references | Prompt ID 17264 if referenced in timeline; cleans pool if unused | `results.md` line 12770 (ID 17264) |
| 28.1 | Media Bins | Hierarchical Bin Tree | Virtual organizational folders saved with project; immutable root "All Media"; arbitrary nested user bins | Bin tree operations (create, rename, delete, move) | Updated project bin hierarchy | Root bin cannot be deleted (ID 12878) or renamed (ID 12889) | `results.md` lines 12747–12751; manual pp. 169–171 |
| 28.2 | Media Bins | Quick Tags System | Assign keyword tags to media clips; automatically creates virtual tag bins | Tag strings | Tagged media and generated tag bins | Duplicate tags grouped together; tag deletion removes tag bin | `results.md` Dialog 871; manual p. 170 |
| 28.3 | Media Bins | Search Media Bins & Smart Bins | Query builder (Field, Operator, Value); "Save as Bin" (static snapshot) vs "Save as Smart Bin" (dynamically re-evaluated query) | Search criteria struct | Static media bin or dynamic reactive smart bin | Duplicate bin name auto-renamed (ID 18174); empty name rejected (ID 12887) | `results.md` Dialogs 869/872; manual pp. 171–172 |
| 29.1 | Unified Explorer | Drive Tree & Navigation | Tree view of local fixed drives, optical discs, and network volumes; directory breadcrumb | Filesystem navigation events | Directory contents list | Disconnected drive shows "Invalid Drive" (ID 3915) | `results.md` §12.11 lines 12898–12952 |
| 29.2 | Unified Explorer | Favorites Pane | "My Favorites" folder bookmarks for quick directory access | Folder paths | Persisted favorites list | Deletion from favorites removes bookmark without deleting disk folder | `results.md` lines 12943, 12972 |
| 29.3 | Unified Explorer | Companion File Filtering | Automatic suppression of internal companion files (`.sfk` audio peaks, `.sfvp0` proxies) from explorer view | Directory file list | Filtered clean media file list | None; companion files remain visible only in external OS file managers | `manual_vegaspro21_en.md` lines 4142, 9057 |
| 29.4 | Unified Explorer | Media Preview Player | Embedded audition transport: Auto-Play toggle, Play/Pause, Stop, Scrubber, Loop Playback | Selected file path, transport controls | Auditioned audio/video stream | Unsupported codec reports error without crashing player | `manual_vegaspro21_en.md` pp. 182–184 |
| 29.5 | Unified Explorer | Scene Detection | Analyzes DV/video timestamps or optical cut points to automatically split files into subclips or timeline events | Selected video file | Multiple subclips or sequential timeline events | Video without scene breaks treated as single clip | `results.md` lines 12882, 12959–12960 |
| 30.1 | Make Movie | Guided Destination Step | Step 1 dialog (Dialog 804/885): destination selection (Web/YouTube, Camera/Device, Hard Drive, DVD/Blu-ray Disc) | User destination selection | Wizard transition to Step 2 | Cancel aborts wizard cleanly | `results.md` Dialog 804/885 |
| 30.2 | Make Movie | Render Settings Step | Step 2 dialog (Dialog 881): output path, preset template, loop region only, stretch/letterbox, space calculation | Settings configuration | Prepared render job | Empty project aborts (ID 3404); insufficient disk space warns (ID 18548, 17603) | `results.md` Dialog 881; lines 6888, 6916 |
| 30.3 | Make Movie | Completion & Launch Step | Step 3 dialog (Dialog 861): Displays completed movie path; buttons for "Play File", "Explore Folder", "Send to Disc Authoring" | Finished render event | Completed dialog UI; external player/explorer launch | Missing player association shows alert ID 1894 | `results.md` Dialog 861 |
| 31.1 | Cache Ruler | Tri-Color Status Strip | Timeline status strip reflecting frame cache state: Green = cached (RAM/disk), Yellow = playable real-time, Red = unrendered | Timeline frame complexity and cache state | Color-coded status ruler above timeline tracks | None; dynamically adapts during timeline zoom/pan | `ORIGINAL_REQUEST.md` R2.6; manual p. 207 |
| 31.2 | Cache Ruler | Dynamic Cache Invalidation | Edits to timeline events, transition tweaks, or effect parameters immediately invalidate overlapping cached frames (Green -> Red) | Timeline edit operations | Invalidated frame cache bitset; updated ruler render | Unmodified 300-frame prerender chunks remain valid (Green) | `manual_vegaspro21_en.md` lines 9123–9124 |
| 32.1 | Dynamic RAM | Dynamic RAM Preview Cache | Dedicated in-memory frame cache (`Shift+B`); fast rendering of loop region into RAM buffer; instant full-FPS playback | Time loop selection, `Shift+B` | Memory-cached frame buffers | Selection auto-truncated if RAM budget exceeded; timeline click cancels | `manual_vegaspro21_en.md` lines 8823–8865 |
| 32.2 | Dynamic RAM | RAM Cache Flusher | Clears all cached frames from Dynamic RAM memory buffer (`Tools | Clear Dynamic RAM Preview`) | Flush command | Freed RAM memory | None | `manual_vegaspro21_en.md` line 8863 |
| 32.3 | Selective Prerender | Prerender to Disk | Renders complex sections of timeline to disk preview files (`Shift+M`) in chunks of ≤ 300 frames (~40 MB per file) | Time selection, prerender template | Temporary prerender video files on disk | Full disk aborts render; preserves already rendered chunks | `manual_vegaspro21_en.md` lines 9077–9124 |
| 32.4 | Selective Prerender | Prerender Cleanup Dialog | Dialog 834: deletes inactive prerender files, all prerender files in selection, or all project prerender files | User cleanup mode selection | Deleted temporary cache files | Inactive files deleted by default, preserving active cache | `results.md` line 7888 (Dialog 834); manual lines 9125–9130 |
| 33.1 | Alerts Subsystem | Missing Media Handler | Dialog 845 / 846: Prompts on offline media with Search, Browse new location, Ignore, Ignore All | Missing file event | Resolved file path or offline status | "Ignore All" silences subsequent prompts during load | `results.md` Dialogs 845, 846, 857 |
| 33.2 | Alerts Subsystem | Codec Failure Handler | Alerts for missing/unsupported video/audio codecs (IDs 17369, 17381, 17384, 33501) | Unsupported stream format | Error notification dialog | File opened with unsupported stream marked offline | `results.md` §10.2 |
| 33.3 | Alerts Subsystem | Disk Full / Resource Handler | Warnings for disk full (ID 18548, 17603) or out of memory; aborts render cleanly | Disk space / memory allocation event | Warning dialog with Retry/Abort/Browse options | Clean abort prevents file corruption | `results.md` §10.4 |
| 33.4 | Alerts Subsystem | GPU Crash / CPU Fallback | GPU acceleration error alert (ID 33316/33317); prompts driver check and automatically falls back to CPU pipeline | GPU device loss / shader crash | CPU fallback execution; warning logged | Continues rendering on CPU without host crash | `results.md` §10.5 |
| 33.5 | Alerts Subsystem | Plugin Failure Handler | Plugin load / crash alerts (IDs 16555, 16511, 16020); chain overflow alerts | Plugin initialization error | Disabled plugin slot; warning message | Timeline playback continues bypassing crashed plugin | `results.md` §10.6 |
| 34.1 | Smart Proxies | Video Proxy Generation | Generates fast-decode proxy video files (`.sfvp0`) alongside original media; automatic on 4K import or manual context menu | Source media file | Companion `.sfvp0` proxy file | Background thread generation; cancellable via status bar | `manual_vegaspro21_en.md` lines 9049–9060 |
| 34.2 | Smart Proxies | Quality-Based Auto-Switching | Video Preview quality setting controls stream source: Draft/Preview uses proxy; Good/Best uses original media | Preview quality setting | Decoded frame stream (proxy vs master) | Seamless hot-swap during active playback; no reload delay | `manual_vegaspro21_en.md` lines 9061–9063 |
| 34.3 | Smart Proxies | Final Render Conforming | Final render export (Make Movie / Render As) strictly conforms and decodes from original full-resolution media | Render trigger | Master-grade rendered output file | Never renders proxy files to output; warns if original offline | `results.md` lines 12864, 12866 |
| 34.4 | Smart Proxies | Audio Proxy Generation | Creates uncompressed audio proxy files for complex compressed audio schemes (ID 17280) | Compressed audio stream | Uncompressed WAV audio proxy | Truncation warning if format changes mid-file (ID 16957) | `results.md` line 12771 (ID 17280) |

---

## 4. Edge Cases

| # | Feature | Input / Condition | Observed / Required Behavior |
|---|---------|-------------------|------------------------------|
| 1 | Feature 24 (Project Properties) | User specifies odd pixel dimension (e.g., width 1921 or height 1079) | Clamped to nearest even integer via `val & !1` (e.g. 1920x1078) because video codecs and chroma subsampling require even dimensions. |
| 2 | Feature 24 (Project Properties) | Dimensions outside `16..=16384` | Clamped to bounds `SIZE_RANGE.0` (16) and `SIZE_RANGE.1` (16384). |
| 3 | Feature 24 (Project Properties) | Switching from 32-bit Float Full Range to 8-bit Video | Compositing Gamma control is disabled and locked to 2.222 Video; gamma choice is only available in 32-bit Full Range (`Float32Full`). |
| 4 | Feature 24 (Project Properties) | User selects HDR10 or HLG in HDR Mode dropdown | System automatically updates 4 dependent settings: sets Pixel Format to 32-bit float full range, sets View Transform to Rec 2020 ST2084 / HLG 1000 nits, enables HDR Preview, and enables Master Display settings (manual lines 3897–3909). |
| 5 | Feature 24 (Project Properties) | Master Bus set to Stereo while LFE filter is enabled | LFE filter controls are disabled because LFE channel exists only in 5.1 Surround mode. |
| 6 | Feature 24 (Project Properties) | Ruler time format set to Audio CD Time with start time offset | Start time offset is forced to 00:00:00; manual p. 84 specifies Red Book Audio CDs must always begin at time zero. |
| 7 | Feature 25 (Project Templates) | User attempts to overwrite a built-in template (e.g. "Full HD 1080p 24") | Action is rejected with alert ID 1580 / 17365: `"Can't modify the built-in '%s' template. Please rename it before saving."` |
| 8 | Feature 25 (Project Templates) | User attempts to delete a built-in template | Action is rejected with alert ID 1585 / 17364: `"Can't delete the built-in '%s' template."` |
| 9 | Feature 25 (Project Templates) | User enters empty or whitespace name when saving custom template | Action is rejected with alert ID 1578: `"Can't save the template without a valid name. Please rename it before saving."` |
| 10 | Feature 25 (Project Templates) | User saves custom template with existing user template name | Prompts confirmation alert ID 1583: `"Overwrite existing '%s' template?"` before replacing. |
| 11 | Feature 26 (Media Profiles) | Gap in rule numbering in `profiles.ini` (e.g. Key1, Key2, Key4) | Parser terminates rule scanning at the first gap (Key3 missing); Key4 and subsequent rules are ignored per specification lines 20107–20108. |
| 12 | Feature 26 (Media Profiles) | Multiple rules match the same media stream | Rule evaluated in reverse order (from highest N down to 1); the latest rule in the file wins and overrides earlier matching rules (lines 20148–20150). |
| 13 | Feature 26 (Media Profiles) | Frame rate floating point comparison (e.g. 29.97 vs 29.97003) | Match comparison applies tolerance epsilon of 0.005 fps (`(rule_fps - stream_fps).abs() < 0.005`). |
| 14 | Feature 26 (Media Profiles) | "Premultiplied (Dirty)" alpha channel interpretation | Image decoded allowing RGB color values to exceed the alpha channel value, accommodating 3D renders composited over complex backgrounds. |
| 15 | Feature 27 (Project Media) | Deleting a media file that is actively in use on the timeline | Displays confirmation prompt ID 17264: `"The selected media is in use in the timeline. Remove from Project Media list, Storyboard Bins, and all timeline references? %s"`. If confirmed, removes all associated timeline events. |
| 16 | Feature 27 (Project Media) | Replacing a media file with a shorter file | Timeline events referencing the media maintain their duration; event active take shows media truncated or looped with warning indicator. |
| 17 | Feature 27 (Project Media) | Sorting by column | Number columns (Size, Length, Sample Rate, FPS) sort by raw numerical values; text columns sort alphabetically; date columns sort by epoch timestamp. |
| 18 | Feature 28 (Media Bins) | User attempts to delete or rename the root "All Media" bin | Rejection with alert ID 12878 (`"Can not remove the root media bin."`) or ID 12889 (`"Can not rename the root media bin."`). |
| 19 | Feature 28 (Media Bins) | Creating a bin with a name that already exists in the same parent | Case-insensitive collision detection triggers auto-rename with ID 18174: `"A media bin named '%s' already exists, renamed duplicate bin as '%s'"` (e.g. appending ` (1)`). |
| 20 | Feature 28 (Media Bins) | Deleting media item from inside a custom bin | Media item is removed from that bin only, but remains untouched in "All Media" and on the timeline. |
| 21 | Feature 29 (Unified Explorer) | Directory containing companion `.sfk` or `.sfvp0` files | Explorer filters out `.sfk` and `.sfvp0` files so the user only sees clean source media files (manual lines 4142, 9057). |
| 22 | Feature 29 (Unified Explorer) | Auto-play audio audition with rapid file selection | Previous audio playback buffer is immediately halted and flushed before initiating playback of the newly selected file. |
| 23 | Feature 30 (Make Movie) | Attempting to render an empty project | Wizard aborts immediately at Step 2 with alert ID 3404: `"Unable to render since project is empty."` |
| 24 | Feature 30 (Make Movie) | Target disk has insufficient free space for estimated file size | Warning prompt ID 17603 / 18548 displayed showing estimated size vs available free space; user can choose to cancel or proceed at own risk. |
| 25 | Feature 30 (Make Movie) | Output render file already exists in target destination | Displays overwrite confirmation prompt ID 1911: `"One or more of the multiple files you are rendering to already exist. Do you want to replace them?"` |
| 26 | Feature 31 (Cache Ruler) | User edits an event inside a previously prerendered (Green) region | Overlapping frames are immediately invalidated and turned Red; adjacent unmodified 300-frame prerender chunks remain Green. |
| 27 | Feature 32 (Dynamic RAM) | Dynamic RAM preview loop selection exceeds configured RAM budget | Selection is automatically adjusted/shortened to encompass only the maximum frames that fit in the allocated RAM cache (manual line 8859). |
| 28 | Feature 32 (Dynamic RAM) | User clicks anywhere on the timeline during `Shift+B` rendering | Dynamic RAM preview rendering is immediately cancelled; frames rendered prior to cancellation remain cached in memory. |
| 29 | Feature 32 (Selective Prerender) | Selective prerendering a long region | Files are written to disk in chunks of at most 300 frames (~40 MB each). If a crash or disk failure occurs on chunk 10, chunks 1–9 remain valid on disk. |
| 30 | Feature 33 (Alerts Subsystem) | Multiple missing files on project open | Clicking "Ignore All" (ID 1703) silences further missing media prompts for all remaining files, setting them offline in one step. |
| 31 | Feature 33 (Alerts Subsystem) | GPU device lost / driver timeout during playback | Alert ID 33316/33317 is displayed and playback automatically switches to pure-CPU software fallback without application crash. |
| 32 | Feature 34 (Smart Proxies) | Preview quality switched from Draft to Best during active playback | Playback seamlessly hot-swaps decoding from proxy file to original full-resolution file at the exact current frame timestamp without restarting playback. |
| 33 | Feature 34 (Smart Proxies) | Final render requested while project is set to Draft preview | Render pipeline strictly ignores preview quality and conforms the final output using original full-resolution media files. |
| 34 | Feature 34 (Smart Proxies) | Proxy file missing on disk while preview quality is set to Draft | Engine automatically falls back to decoding the original media file instead of reporting an error. |

---

## 5. Generic Identifier Mappings (Zero Trademark Policy)

In accordance with repository policy (`deny.toml`, `PROJECT.md`), all runtime strings, enum variants, and UI labels avoid third-party trademarks:

| Reference / Trademarked Term | Generic Clean-Room Neoveg Identifier | Description |
|---|---|---|
| `Sony Vegas / MAGIX VEGAS Pro` | `Neoveg` / `Application` / `$(AppName)` | Non-linear video editing host application |
| `VEGAS Project (.veg)` | `Neoveg Project (.nvp / .neoveg)` | Native project interchange file |
| `VEGAS Preset Template (.vqpt)` | `Project Template (.vqpt / .nvpt)` | Video Quality Preset Template |
| `VEGAS Video Proxy (.sfvp0)` | `Video Proxy (.nvproxy / .sfvp0)` | Downscaled companion editing proxy |
| `Sound Forge Peak File (.sfk)` | `Audio Peak Cache (.nvpeak / .sfk)` | Waveform visual cache file |
| `DVD Architect` | `Disc Authoring Tool` | Optical disc authoring application (Dialog 861) |
| `Vimeo / YouTube` | `Web Video Sharing / Online Video` | Web export destination presets |
| `Sony Wireless Adapter` | `Wireless Camera Ingest Adapter` | Camera media relinking subsystem |
| `XDCAM / P2 / REDCODE` | `Broadcast Disc / Broadcast Card / RAW Clip` | Professional camera media containers |

---

## 6. Caveats

1. **Optical Media Burning**: Physical CD/DVD/Blu-ray drive writing hardware integration is outside the headless test scope. Neoveg models disc image generation (`.iso` / disc folders) and guided wizard workflows, but relies on system utilities for physical burning.
2. **Audio Peak Files (`.sfk`)**: Peak calculation is performed asynchronously in the background. Peak file existence is transparent to timeline editing.
3. **Hardware GPU Interop**: Per Project constraints, all GPU effects and deinterlacing modes (like Smart Adaptive) include pure-CPU software fallbacks to guarantee CI and headless test compatibility.

---

## 7. Conclusion

Milestone 5 specification mining is 100% complete across all 11 assigned features:
- Complete parameter models, enum definitions, and formulas were established for **Project Properties (F24)**, **Templates Subsystem (F25)**, **Media Profiles Rule Engine (F26)**, and **Project Media Detail Columns (F27)**.
- Exhaustive UI layouts, workflows, and dialog IDs were documented for **Media Bins & Smart Bins (F28)**, **Unified File Explorer (F29)**, and **Make Movie Wizard (F30)**.
- Exact runtime caching and rendering semantics were mined for **Preview Cache Ruler (F31)**, **Dynamic RAM & Prerender (F32)**, **Standardized Alerts (F33)**, and **Smart Proxies & Conforming (F34)**.
- All 34 edge cases and generic trademark mappings are fully documented. The specifications are actionable, verified against authoritative manuals and PE reverse-engineering logs, and ready for immediate implementation by the milestone engineers.

---

## 8. Verification Method

To independently verify the citations and extracted specifications:
1. **Manual citations**:
   - Run Python or text search on `manuais/markdown/manual_vegaspro21_en.md` for the line ranges cited:
     - Project Properties: lines 3785–4647
     - Dynamic RAM (`Shift+B`): lines 8823–8865
     - Smart Proxies: lines 9049–9070
     - Selective Prerender (`Shift+M`): lines 9077–9140
2. **Results.md citations**:
   - Inspect `results.md` for section anchors:
     - §8.1 Make Movie (lines 3580–4030)
     - §8.7 Project Properties dialogs 807, 817, 826, 863, 892, 913 (lines 6376–6580)
     - §10 Standardized Alerts (lines 6797–7890)
     - §12.10 Project Media 23 columns (lines 12789–12824)
     - §18.3 Project Templates (line 20092)
     - §18.4 `profiles.ini` (lines 20098–20197)
3. **Keyboard shortcuts**:
   - Inspect `manuais/markdown/VEGAS_Pro_18_EN_Keyboard_shortcuts.md` lines 41, 43, 373, 379 for `Alt+Enter`, `Alt+1`, `Alt+5`, `Shift+B`, and `Shift+M`.
