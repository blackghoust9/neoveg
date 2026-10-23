/**
 * OpenNeoveg 21 — Core TypeScript Data Models
 * Mirrors the Rust Core (`neoveg-core`), Python Backend, Frei0r, LADSPA & CorridorKey schemas.
 */

export type FadeCurveType = 'linear' | 'fast' | 'slow' | 'smooth' | 'sharp';

export type EditToolMode = 'normal' | 'razor' | 'slip' | 'slide' | 'rateStretch';

export type BlendModeType =
  | 'sourceAlpha'
  | 'add'
  | 'multiply'
  | 'screen'
  | 'overlay'
  | 'difference';

export type VideoFXType =
  | 'brightnessContrast'
  | 'colorGrade'
  | 'corridorKey'
  | 'chromaKey'
  | 'blur'
  | 'vignette'
  | 'sharpen'
  | 'vhsGlitch'
  | 'invert'
  | 'sepia'
  | 'frei0r.glow'
  | 'frei0r.cartoon'
  | 'frei0r.pixeliz0r'
  | 'frei0r.sobel'
  | 'frei0r.coloradj_RGB'
  | 'frei0r.posterize'
  | 'frei0r.vertigo'
  | 'frei0r.distort0r'
  | 'ladspa.sc4_1882'
  | 'ladspa.fast_lookahead_limiter_1913'
  | 'ladspa.mbeq_1197'
  | 'ladspa.plate_1423'
  | 'ladspa.gate_1410';

export interface VideoFXInstance {
  id: string;
  type: VideoFXType;
  name: string;
  enabled: boolean;
  params: Record<string, number | string | boolean>;
}

export interface PanCropKeyframe {
  time: number;       // Offset in seconds from start of clip
  x: number;
  y: number;
  scale: number;
  rotation: number;
}

export interface PanCropState {
  x: number;          // Normalized center offset (-1.0 to +1.0)
  y: number;          // Normalized center offset (-1.0 to +1.0)
  scale: number;      // Zoom factor (0.2 to 3.0, where 1.0 = 100%)
  rotation: number;   // Degrees (-180 to +180)
  flipH: boolean;
  flipV: boolean;
  lockAspect?: boolean;
  sizeAboutCenter?: boolean;
  keyframes?: PanCropKeyframe[];
}

export interface EnvelopePoint {
  time: number;       // Timeline seconds
  value: number;      // 0.0 to 1.0 (or 0.0 to 1.5 for audio gain)
}

export interface ClipEvent {
  id: string;
  name: string;
  mediaId: string;
  mediaFile?: string;
  mediaUrl?: string;
  mediaType: 'video' | 'audio' | 'generator' | 'title';
  generatorType?:
    | 'smpteBars'
    | 'cyberGrid'
    | 'titleText'
    | 'creditRoll'
    | 'solidColor'
    | 'greenScreenActor'
    | 'testSlate';
  titleText?: string;
  subtitleText?: string;
  transcriptText?: string;
  color: string;
  start: number;        // Start time on timeline (seconds)
  duration: number;     // Event length on timeline (seconds)
  trimIn: number;       // Source media offset (seconds)
  fadeIn: number;       // Fade-in duration (seconds)
  fadeOut: number;      // Fade-out duration (seconds)
  fadeInCurve: FadeCurveType;
  fadeOutCurve: FadeCurveType;
  transitionType?: 'crossfade' | 'wipeLeft' | 'irisCircle' | 'flashWhite' | 'zoomCross';
  gain: number;         // Opacity (0..1) for video, Gain (0..2) for audio
  playbackRate: number; // Velocity (0.25 .. 4.0)
  muted: boolean;
  locked: boolean;
  panCrop: PanCropState;
  fx: VideoFXInstance[];
  peaks?: number[];
  freq?: number;
}

export interface Track {
  id: string;
  name: string;
  type: 'video' | 'audio';
  color: string;
  height?: number;      // Individual track height in px (44 to 220, default 76)
  groupId?: string;     // Track Group ID (V+A or custom group)
  groupName?: string;   // Track Group label (e.g. "V+A Group 1")
  groupColor?: string;  // Track Group bracket hex color
  level: number;        // Opacity (0..1) for video, Volume (0..1.5) for audio
  pan: number;          // Stereo pan (-1.0 Left .. +1.0 Right) for audio
  blendMode: BlendModeType;
  muted: boolean;
  solo: boolean;
  locked: boolean;
  armed: boolean;
  showEnvelope: boolean;
  envelopePoints: EnvelopePoint[];
  fx: VideoFXInstance[];
  clips: ClipEvent[];
}

export interface TimelineDragPayload {
  kind: 'media' | 'generator' | 'fx' | 'audiofx' | 'transition';
  id: string;
  name: string;
  mediaType?: 'video' | 'audio' | 'generator' | 'title';
  duration?: number;
  color?: string;
  asset?: MediaAsset;
  fxType?: VideoFXType;
  fxDefaults?: Record<string, number>;
  transitionCurve?: FadeCurveType;
  transitionType?: ClipEvent['transitionType'];
}

export interface TimelineMarker {
  id: string;
  time: number;
  label: string;
  color: string;
}

export interface TimelineRegion {
  id: string;
  start: number;
  end: number;
  label: string;
}

export interface CrossfadeOverlap {
  trackId: string;
  trackIndex: number;
  outgoingClipId: string;
  incomingClipId: string;
  start: number;
  end: number;
  duration: number;
  outCurve: FadeCurveType;
  inCurve: FadeCurveType;
}

export interface MediaAsset {
  id: string;
  name: string;
  fileName?: string;
  url?: string;
  type: 'video' | 'audio' | 'generator' | 'title';
  generatorType?: ClipEvent['generatorType'];
  duration: number;
  width: number;
  height: number;
  fps: number;
  codec: string;
  sampleRate?: number;
  channels?: number;
  peakDb?: number;
  rmsDb?: number;
  peaks?: number[];
  color: string;
}

export interface NeovegProject {
  name: string;
  isDemo?: boolean;
  description?: string;
  width: number;
  height: number;
  fps: number;
  pixelAspect?: number;
  fieldOrder?: string;
  pixelFormat?: string;
  colorSpace?: string;
  motionBlurType?: string;
  deinterlaceMethod?: string;
  resampleMode?: string;
  sampleRate: number;
  audioBitDepth?: number;
  masterBusMode?: string;
  bpm: number;
  playhead: number;
  loopStart: number;
  loopEnd: number;
  loopEnabled: boolean;
  snapEnabled: boolean;
  rippleEnabled: boolean;
  autoCrossfadeEnabled: boolean;
  previewQuality: 'draft' | 'preview' | 'good' | 'best';
  splitScreenBypass: boolean;
  showSafeAreas: boolean;
  showGrid: boolean;
  masterVolume: number;
  tracks: Track[];
  markers: TimelineMarker[];
  regions: TimelineRegion[];
}
