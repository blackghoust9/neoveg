/**
 * OpenVegas Pro — Real-Time Multi-Track Video Compositor & Scope Engine
 */

import {
  BlendModeType,
  ClipEvent,
  EnvelopePoint,
  FadeCurveType,
  Track,
  VegasProject,
  VideoFXInstance,
} from '../types';

export function evaluateFadeCurve(curve: FadeCurveType, t: number): number {
  const x = Math.max(0, Math.min(1, t));
  switch (curve) {
    case 'linear':
      return x;
    case 'fast':
      return 1 - Math.pow(1 - x, 3);
    case 'slow':
      return Math.pow(x, 3);
    case 'sharp':
      return x < 0.5
        ? 0.5 * Math.pow(2 * x, 3)
        : 1 - 0.5 * Math.pow(2 * (1 - x), 3);
    case 'smooth':
    default:
      return 0.5 - 0.5 * Math.cos(Math.PI * x);
  }
}

export function formatSMPTE(seconds: number, fps = 29.97, dropFrame = true): string {
  const clamped = Math.max(0, seconds);
  const nomFps = Math.max(1, Math.round(fps));
  const totalFrames = Math.floor(clamped * fps + 1e-4);
  const frames = totalFrames % nomFps;
  const totalSeconds = Math.floor(totalFrames / nomFps);
  const secs = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const mins = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);
  const sep = dropFrame ? ';' : ':';
  return (
    String(hours).padStart(2, '0') +
    ':' +
    String(mins).padStart(2, '0') +
    ':' +
    String(secs).padStart(2, '0') +
    sep +
    String(frames).padStart(2, '0')
  );
}

export function evaluateTrackEnvelope(points: EnvelopePoint[], time: number, fallback: number): number {
  if (!points || points.length === 0) return fallback;
  const sorted = [...points].sort((a, b) => a.time - b.time);
  if (time <= sorted[0].time) return sorted[0].value;
  if (time >= sorted[sorted.length - 1].time) return sorted[sorted.length - 1].value;
  for (let i = 0; i < sorted.length - 1; i++) {
    const p0 = sorted[i];
    const p1 = sorted[i + 1];
    if (time >= p0.time && time <= p1.time) {
      const span = Math.max(0.0001, p1.time - p0.time);
      const ratio = (time - p0.time) / span;
      return p0.value + (p1.value - p0.value) * ratio;
    }
  }
  return fallback;
}

export function computeClipFadeMultiplier(clip: ClipEvent, playhead: number): number {
  const localT = playhead - clip.start;
  if (localT < 0 || localT > clip.duration) return 0;
  let mult = clip.gain;
  if (clip.fadeIn > 0.005 && localT < clip.fadeIn) {
    mult *= evaluateFadeCurve(clip.fadeInCurve, localT / clip.fadeIn);
  }
  const rem = clip.duration - localT;
  if (clip.fadeOut > 0.005 && rem < clip.fadeOut) {
    mult *= evaluateFadeCurve(clip.fadeOutCurve, rem / clip.fadeOut);
  }
  return Math.max(0, Math.min(2, mult));
}

function mapBlendMode(mode: BlendModeType): GlobalCompositeOperation {
  switch (mode) {
    case 'add':
      return 'lighter';
    case 'multiply':
      return 'multiply';
    case 'screen':
      return 'screen';
    case 'overlay':
      return 'overlay';
    case 'difference':
      return 'difference';
    case 'sourceAlpha':
    default:
      return 'source-over';
  }
}

export class VegasCompositor {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private offscreen: HTMLCanvasElement;
  private offCtx: CanvasRenderingContext2D;
  private scopeCanvas: HTMLCanvasElement | null = null;
  private videoPool: Map<string, HTMLVideoElement> = new Map();

  constructor(canvas: HTMLCanvasElement, scopeCanvas?: HTMLCanvasElement | null) {
    this.canvas = canvas;
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) throw new Error('2D context unavailable');
    this.ctx = context;

    this.offscreen = document.createElement('canvas');
    this.offscreen.width = canvas.width;
    this.offscreen.height = canvas.height;
    this.offCtx = this.offscreen.getContext('2d', { willReadFrequently: true })!;
    this.scopeCanvas = scopeCanvas || null;
  }

  public registerVideoElement(url: string): HTMLVideoElement {
    let vid = this.videoPool.get(url);
    if (!vid) {
      vid = document.createElement('video');
      vid.src = url;
      vid.muted = true;
      vid.preload = 'auto';
      vid.playsInline = true;
      vid.crossOrigin = 'anonymous';
      this.videoPool.set(url, vid);
    }
    return vid;
  }

  public syncVideos(project: VegasProject, isPlaying: boolean): void {
    const t = project.playhead;
    for (const track of project.tracks) {
      if (track.type !== 'video') continue;
      for (const clip of track.clips) {
        if (!clip.mediaUrl) continue;
        const vid = this.registerVideoElement(clip.mediaUrl);
        const active = t >= clip.start && t <= clip.start + clip.duration && !clip.muted && !track.muted;
        if (active) {
          const targetMediaTime = clip.trimIn + (t - clip.start) * (clip.playbackRate || 1);
          const drift = Math.abs((vid.currentTime || 0) - targetMediaTime);
          if (vid.readyState >= 1) {
            const dur = vid.duration || 6;
            const wrapped = targetMediaTime % dur;
            if (!isPlaying && drift > 0.03) {
              vid.currentTime = wrapped;
            } else if (isPlaying && drift > 0.22) {
              vid.currentTime = wrapped;
            }
            vid.playbackRate = Math.max(0.25, Math.min(4.0, clip.playbackRate || 1.0));
            if (isPlaying && vid.paused) {
              vid.play().catch(() => {});
            } else if (!isPlaying && !vid.paused) {
              vid.pause();
            }
          }
        } else if (!vid.paused) {
          vid.pause();
        }
      }
    }
  }

  public renderFrame(project: VegasProject, selectedClip: ClipEvent | null, showPanCropGizmo = false): void {
    const scaleMap: Record<VegasProject['previewQuality'], number> = {
      draft: 0.4,
      preview: 0.65,
      good: 0.85,
      best: 1.0,
    };
    const qScale = scaleMap[project.previewQuality] || 1.0;
    const targetW = Math.round(640 * qScale);
    const targetH = Math.round(360 * qScale);

    if (this.canvas.width !== targetW || this.canvas.height !== targetH) {
      this.canvas.width = targetW;
      this.canvas.height = targetH;
      this.offscreen.width = targetW;
      this.offscreen.height = targetH;
    }

    const w = this.canvas.width;
    const h = this.canvas.height;
    const ctx = this.ctx;

    // Clear master frame
    ctx.save();
    ctx.fillStyle = '#07080a';
    ctx.fillRect(0, 0, w, h);

    const anySolo = project.tracks.some((tr) => tr.type === 'video' && tr.solo);
    // In Vegas Pro, Track 1 (top) renders on top of Track 2 (below).
    // So we composite from bottom-most video track up to top-most video track.
    const videoTracks = project.tracks
      .filter((tr) => tr.type === 'video')
      .slice()
      .reverse();

    for (const track of videoTracks) {
      if (track.muted || (anySolo && !track.solo)) continue;
      const trackLevel = track.showEnvelope
        ? evaluateTrackEnvelope(track.envelopePoints, project.playhead, track.level)
        : track.level;
      if (trackLevel <= 0.001) continue;

      const activeClips = track.clips.filter(
        (c) => !c.muted && project.playhead >= c.start && project.playhead <= c.start + c.duration
      );

      for (const clip of activeClips) {
        this.renderClipToLayer(clip, track, trackLevel, project, w, h);
      }
    }

    // Overlays: Safe Areas, Rule-of-Thirds Grid, Pan/Crop Gizmo
    if (project.showGrid) {
      this.drawGridOverlay(ctx, w, h);
    }
    if (project.showSafeAreas) {
      this.drawSafeAreasOverlay(ctx, w, h);
    }
    if (showPanCropGizmo && selectedClip && selectedClip.mediaType !== 'audio') {
      this.drawPanCropGizmo(ctx, selectedClip, w, h);
    }

    ctx.restore();

    if (this.scopeCanvas) {
      this.updateVideoScopes();
    }
  }

  private renderClipToLayer(
    clip: ClipEvent,
    track: Track,
    trackLevel: number,
    project: VegasProject,
    w: number,
    h: number
  ): void {
    const octx = this.offCtx;
    octx.save();
    octx.clearRect(0, 0, w, h);

    const fadeMult = computeClipFadeMultiplier(clip, project.playhead);
    const effectiveAlpha = Math.max(0, Math.min(1, fadeMult * trackLevel));
    if (effectiveAlpha <= 0.001) {
      octx.restore();
      return;
    }

    const localT = project.playhead - clip.start;
    const fadeProgress = clip.fadeIn > 0.01 && localT < clip.fadeIn ? localT / clip.fadeIn : 1.0;

    // Transition clip-path / transform
    if (fadeProgress < 1.0 && clip.transitionType) {
      if (clip.transitionType === 'wipeLeft') {
        octx.beginPath();
        octx.rect(0, 0, w * fadeProgress, h);
        octx.clip();
      } else if (clip.transitionType === 'irisCircle') {
        const maxR = Math.hypot(w / 2, h / 2);
        octx.beginPath();
        octx.arc(w / 2, h / 2, maxR * fadeProgress, 0, Math.PI * 2);
        octx.clip();
      }
    }

    // Apply Pan/Crop Transform (including keyframe interpolation if present)
    const basePc = clip.panCrop || { x: 0, y: 0, scale: 1, rotation: 0, flipH: false, flipV: false };
    let pcX = basePc.x;
    let pcY = basePc.y;
    let pcScale = basePc.scale || 1;
    let pcRot = basePc.rotation || 0;

    if (basePc.keyframes && basePc.keyframes.length > 0) {
      const kfs = [...basePc.keyframes].sort((a, b) => a.time - b.time);
      if (localT <= kfs[0].time) {
        pcX = kfs[0].x;
        pcY = kfs[0].y;
        pcScale = kfs[0].scale;
        pcRot = kfs[0].rotation;
      } else if (localT >= kfs[kfs.length - 1].time) {
        const last = kfs[kfs.length - 1];
        pcX = last.x;
        pcY = last.y;
        pcScale = last.scale;
        pcRot = last.rotation;
      } else {
        for (let i = 0; i < kfs.length - 1; i++) {
          if (localT >= kfs[i].time && localT <= kfs[i + 1].time) {
            const span = Math.max(0.001, kfs[i + 1].time - kfs[i].time);
            const r = (localT - kfs[i].time) / span;
            pcX = kfs[i].x + (kfs[i + 1].x - kfs[i].x) * r;
            pcY = kfs[i].y + (kfs[i + 1].y - kfs[i].y) * r;
            pcScale = kfs[i].scale + (kfs[i + 1].scale - kfs[i].scale) * r;
            pcRot = kfs[i].rotation + (kfs[i + 1].rotation - kfs[i].rotation) * r;
            break;
          }
        }
      }
    }

    // Vegas Pan/Crop works like a camera crop frame: smaller crop box = zoomed in!
    const zoomFactor = 1 / Math.max(0.2, Math.min(3.0, pcScale));
    octx.translate(w / 2 - pcX * (w * 0.45), h / 2 - pcY * (h * 0.45));
    if (pcRot) {
      octx.rotate((-pcRot * Math.PI) / 180);
    }
    const sx = zoomFactor * (basePc.flipH ? -1 : 1);
    const sy = zoomFactor * (basePc.flipV ? -1 : 1);
    octx.scale(sx, sy);
    octx.translate(-w / 2, -h / 2);

    // Build CSS filter from Clip FX + Track FX
    const allFx: VideoFXInstance[] = [...(clip.fx || []), ...(track.fx || [])].filter((f) => f.enabled);
    const cssFilter = this.buildCssFilterString(allFx);

    if (!project.splitScreenBypass && cssFilter !== 'none') {
      octx.filter = cssFilter;
    }

    this.drawClipSource(octx, clip, localT, w, h);

    // If Split-Screen FX Bypass is active, render right half WITH FX and left half WITHOUT FX
    if (project.splitScreenBypass && cssFilter !== 'none') {
      octx.save();
      octx.beginPath();
      octx.rect(w / 2, 0, w / 2, h);
      octx.clip();
      octx.filter = cssFilter;
      this.drawClipSource(octx, clip, localT, w, h);
      octx.restore();
    }

    // Custom pixel/canvas overlays for Vignette, Color Grade Lift/Gain, VHS Scanlines
    for (const fx of allFx) {
      if (fx.type === 'vignette') {
        const strength = Number(fx.params.strength ?? 0.65);
        const grad = octx.createRadialGradient(w / 2, h / 2, w * 0.22, w / 2, h / 2, w * 0.62);
        grad.addColorStop(0, 'rgba(0,0,0,0)');
        grad.addColorStop(1, `rgba(0,0,0,${Math.min(0.95, strength)})`);
        octx.fillStyle = grad;
        octx.fillRect(0, 0, w, h);
      } else if (fx.type === 'colorGrade') {
        const liftR = Number(fx.params.liftR ?? 0);
        const liftB = Number(fx.params.liftB ?? 0);
        const gainR = Number(fx.params.gainR ?? 1);
        const gainB = Number(fx.params.gainB ?? 1);
        octx.save();
        octx.globalCompositeOperation = 'overlay';
        const rTint = Math.round(128 + (gainR - 1 + liftR) * 180);
        const bTint = Math.round(128 + (gainB - 1 + liftB) * 180);
        octx.fillStyle = `rgba(${Math.max(0, Math.min(255, rTint))}, 132, ${Math.max(0, Math.min(255, bTint))}, 0.28)`;
        octx.fillRect(0, 0, w, h);
        octx.restore();
      } else if (fx.type === 'vhsGlitch' || fx.type === 'frei0r.distort0r' || fx.type === 'frei0r.vertigo') {
        octx.save();
        octx.fillStyle = 'rgba(0, 0, 0, 0.18)';
        for (let y = 0; y < h; y += 4) {
          octx.fillRect(0, y, w, 1.5);
        }
        const barY = (localT * 90) % h;
        octx.fillStyle = 'rgba(59, 130, 246, 0.14)';
        octx.fillRect(0, barY, w, 14);
        octx.restore();
      } else if (fx.type === 'frei0r.glow') {
        octx.save();
        octx.globalCompositeOperation = 'screen';
        const gGrad = octx.createRadialGradient(w / 2, h / 2, 10, w / 2, h / 2, w * 0.55);
        gGrad.addColorStop(0, 'rgba(147, 197, 253, 0.28)');
        gGrad.addColorStop(1, 'rgba(30, 58, 138, 0.0)');
        octx.fillStyle = gGrad;
        octx.fillRect(0, 0, w, h);
        octx.restore();
      } else if (fx.type === 'corridorKey') {
        // Real-time Green Screen Color Unmixing & Spill Suppression (CorridorKey Engine)
        const imgData = octx.getImageData(0, 0, w, h);
        const d = imgData.data;
        const sim = Number(fx.params.similarity ?? 0.26) * 255;
        const despill = Number(fx.params.despill ?? 0.85);
        for (let i = 0; i < d.length; i += 4) {
          const r = d[i];
          const g = d[i + 1];
          const b = d[i + 2];
          const greenExcess = g - Math.max(r, b);
          if (greenExcess > 18) {
            const alpha = Math.max(0, Math.min(1, 1 - (greenExcess - 18) / Math.max(12, sim)));
            d[i + 3] = Math.round(d[i + 3] * alpha);
            // Color unmixing & spill removal on semi-transparent hair/edge pixels
            d[i + 1] = Math.round(g - greenExcess * despill);
          }
        }
        octx.putImageData(imgData, 0, 0);
      }
    }

    if (fadeProgress < 1.0 && clip.transitionType === 'flashWhite') {
      octx.fillStyle = `rgba(255,255,255,${(1 - fadeProgress) * 0.85})`;
      octx.fillRect(0, 0, w, h);
    }

    octx.restore();

    // Composite onto main canvas
    this.ctx.save();
    this.ctx.globalAlpha = effectiveAlpha;
    this.ctx.globalCompositeOperation = mapBlendMode(track.blendMode);
    this.ctx.drawImage(this.offscreen, 0, 0);

    if (project.splitScreenBypass) {
      this.ctx.strokeStyle = '#f97316';
      this.ctx.lineWidth = 2;
      this.ctx.beginPath();
      this.ctx.moveTo(w / 2, 0);
      this.ctx.lineTo(w / 2, h);
      this.ctx.stroke();
      this.ctx.fillStyle = 'rgba(0,0,0,0.65)';
      this.ctx.fillRect(8, 8, 86, 20);
      this.ctx.fillRect(w / 2 + 8, 8, 76, 20);
      this.ctx.fillStyle = '#e2e8f0';
      this.ctx.font = 'bold 10px Inter, sans-serif';
      this.ctx.fillText('BYPASS (RAW)', 14, 21);
      this.ctx.fillStyle = '#fb923c';
      this.ctx.fillText('FX ACTIVE', w / 2 + 14, 21);
    }
    this.ctx.restore();
  }

  private buildCssFilterString(fxList: VideoFXInstance[]): string {
    const parts: string[] = [];
    for (const fx of fxList) {
      if (!fx.enabled) continue;
      if (fx.type === 'brightnessContrast') {
        const b = 100 + Number(fx.params.brightness ?? 0) * 100;
        const c = Number(fx.params.contrast ?? 1) * 100;
        const s = Number(fx.params.saturation ?? 1) * 100;
        parts.push(`brightness(${b.toFixed(0)}%) contrast(${c.toFixed(0)}%) saturate(${s.toFixed(0)}%)`);
      } else if (fx.type === 'colorGrade') {
        const sat = Number(fx.params.saturation ?? 1.15) * 100;
        parts.push(`saturate(${sat.toFixed(0)}%) contrast(106%)`);
      } else if (fx.type === 'blur') {
        const rad = Number(fx.params.radius ?? 4);
        parts.push(`blur(${rad.toFixed(1)}px)`);
      } else if (fx.type === 'invert') {
        parts.push('invert(100%)');
      } else if (fx.type === 'sepia') {
        const amt = Number(fx.params.amount ?? 0.85) * 100;
        parts.push(`sepia(${amt.toFixed(0)}%)`);
      } else if (fx.type === 'frei0r.cartoon' || fx.type === 'frei0r.posterize') {
        parts.push('contrast(145%) saturate(135%)');
      } else if (fx.type === 'frei0r.sobel') {
        parts.push('contrast(220%) invert(10%) saturate(180%)');
      } else if (fx.type === 'frei0r.coloradj_RGB') {
        const r = Number(fx.params.r ?? 1.12);
        parts.push(`brightness(${(r * 100).toFixed(0)}%) contrast(110%)`);
      }
    }
    return parts.length > 0 ? parts.join(' ') : 'none';
  }

  private drawClipSource(
    ctx: CanvasRenderingContext2D,
    clip: ClipEvent,
    localT: number,
    w: number,
    h: number
  ): void {
    // 1. Check if real HTMLVideoElement is ready
    if (clip.mediaUrl && clip.mediaType === 'video') {
      const vid = this.registerVideoElement(clip.mediaUrl);
      if (vid && vid.readyState >= 2) {
        ctx.drawImage(vid, 0, 0, w, h);
        // Subtle corner clip badge
        ctx.fillStyle = 'rgba(0,0,0,0.45)';
        ctx.fillRect(w - 150, h - 26, 142, 20);
        ctx.fillStyle = '#93c5fd';
        ctx.font = '10px JetBrains Mono, monospace';
        ctx.fillText(`${clip.name.slice(0, 18)}`, w - 144, h - 13);
        return;
      }
    }

    // 2. Procedural Vegas Media Generators & Titles
    const gen = clip.generatorType || (clip.mediaType === 'title' ? 'titleText' : 'smpteBars');

    if (gen === 'titleText') {
      // Transparent lower-third / center title overlay
      const title = clip.titleText || clip.name || 'VEGAS PRO STUDIO 21';
      const sub = clip.subtitleText || 'NON-LINEAR VIDEO & AUDIO BROADCAST ENGINE';

      // Glassmorphic lower-third banner
      const bannerY = h * 0.68;
      const grad = ctx.createLinearGradient(w * 0.08, bannerY, w * 0.78, bannerY + 68);
      grad.addColorStop(0, 'rgba(15, 23, 42, 0.88)');
      grad.addColorStop(0.85, 'rgba(30, 58, 138, 0.72)');
      grad.addColorStop(1, 'rgba(30, 58, 138, 0.0)');
      ctx.fillStyle = grad;
      ctx.fillRect(w * 0.08, bannerY, w * 0.72, 66);

      // Accent bar
      ctx.fillStyle = clip.color || '#3b82f6';
      ctx.fillRect(w * 0.08, bannerY, 5, 66);

      ctx.fillStyle = '#f8fafc';
      ctx.font = `bold ${Math.round(h * 0.068)}px "Segoe UI", Inter, sans-serif`;
      ctx.shadowColor = 'rgba(0,0,0,0.75)';
      ctx.shadowBlur = 8;
      ctx.fillText(title, w * 0.1, bannerY + 32);

      ctx.fillStyle = '#93c5fd';
      ctx.font = `600 ${Math.round(h * 0.034)}px "JetBrains Mono", monospace`;
      ctx.fillText(sub, w * 0.1, bannerY + 54);
      ctx.shadowBlur = 0;
      return;
    }

    if (gen === 'creditRoll') {
      ctx.fillStyle = 'rgba(9, 11, 16, 0.85)';
      ctx.fillRect(0, 0, w, h);
      const scrollY = h - ((localT / Math.max(1, clip.duration)) * (h + 220));
      const lines = [
        'OPEN VEGAS PRO STUDIO',
        '────────────────────',
        'Core Timeline & DSP Engine: Rust (openvegas-core)',
        'Media Pipeline & Scripting API: Python + FFmpeg',
        'Studio NLE Interface: TypeScript + WebGL/Canvas',
        'Audio Engine: Multi-Bus Web Audio API',
        '5-Curve Auto-Crossfade Solver',
      ];
      ctx.textAlign = 'center';
      lines.forEach((line, idx) => {
        ctx.fillStyle = idx === 0 ? '#60a5fa' : '#e2e8f0';
        ctx.font = idx === 0 ? 'bold 20px Inter, sans-serif' : '13px Inter, sans-serif';
        ctx.fillText(line, w / 2, scrollY + idx * 32);
      });
      ctx.textAlign = 'left';
      return;
    }

    if (gen === 'cyberGrid' || clip.name.toLowerCase().includes('b-roll') || clip.name.toLowerCase().includes('cyber')) {
      // Dynamic Synthwave / Cyber Broadcast Scene
      const bgGrad = ctx.createLinearGradient(0, 0, 0, h);
      bgGrad.addColorStop(0, '#090d16');
      bgGrad.addColorStop(0.55, '#1e1b4b');
      bgGrad.addColorStop(0.56, '#0f172a');
      bgGrad.addColorStop(1, '#020617');
      ctx.fillStyle = bgGrad;
      ctx.fillRect(0, 0, w, h);

      // Sun
      const sunGrad = ctx.createLinearGradient(0, h * 0.15, 0, h * 0.55);
      sunGrad.addColorStop(0, '#f97316');
      sunGrad.addColorStop(1, '#ec4899');
      ctx.fillStyle = sunGrad;
      ctx.beginPath();
      ctx.arc(w / 2, h * 0.38, h * 0.18, 0, Math.PI * 2);
      ctx.fill();

      // Perspective horizon grid
      ctx.strokeStyle = 'rgba(56, 189, 248, 0.45)';
      ctx.lineWidth = 1.2;
      const horizon = h * 0.56;
      for (let i = -10; i <= 10; i++) {
        ctx.beginPath();
        ctx.moveTo(w / 2 + i * (w * 0.025), horizon);
        ctx.lineTo(w / 2 + i * (w * 0.14), h);
        ctx.stroke();
      }
      const speedOffset = (localT * 1.5) % 1;
      for (let row = 0; row < 8; row++) {
        const p = Math.pow((row + speedOffset) / 8, 1.8);
        const y = horizon + p * (h - horizon);
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }

      ctx.fillStyle = '#38bdf8';
      ctx.font = 'bold 12px monospace';
      ctx.fillText(`SCENE: ${clip.name.toUpperCase()}  |  T+${localT.toFixed(2)}s`, 16, 26);
      return;
    }

    if (gen === 'solidColor') {
      ctx.fillStyle = clip.color || '#2563eb';
      ctx.fillRect(0, 0, w, h);
      return;
    }

    if (gen === 'greenScreenActor') {
      // CorridorKey Green Screen Studio Subject Demo (Actor silhouette + fine hair strands on #00e020 backdrop)
      ctx.fillStyle = '#00d820';
      ctx.fillRect(0, 0, w, h);

      const cx = w / 2 + Math.sin(localT * 1.4) * 18;
      const cy = h * 0.52;

      // Shoulders
      ctx.fillStyle = '#1e293b';
      ctx.beginPath();
      ctx.ellipse(cx, h + 10, w * 0.22, h * 0.28, 0, Math.PI, 0, false);
      ctx.fill();

      // Fine hair strands (for testing CorridorKey sub-pixel alpha unmixing)
      ctx.strokeStyle = 'rgba(42, 28, 22, 0.72)';
      ctx.lineWidth = 2;
      for (let a = -12; a <= 12; a++) {
        ctx.beginPath();
        ctx.moveTo(cx + a * 4, cy - h * 0.16);
        ctx.lineTo(cx + a * 5.8 + Math.sin(localT * 3 + a) * 3, cy - h * 0.24);
        ctx.stroke();
      }

      // Head
      ctx.fillStyle = '#e8b496';
      ctx.beginPath();
      ctx.ellipse(cx, cy - h * 0.04, w * 0.085, h * 0.17, 0, 0, Math.PI * 2);
      ctx.fill();

      // Eyes & smile
      ctx.fillStyle = '#0f172a';
      ctx.beginPath();
      ctx.arc(cx - w * 0.03, cy - h * 0.06, 4, 0, Math.PI * 2);
      ctx.arc(cx + w * 0.03, cy - h * 0.06, 4, 0, Math.PI * 2);
      ctx.fill();

      ctx.fillStyle = 'rgba(0,0,0,0.65)';
      ctx.fillRect(14, 14, 320, 24);
      ctx.fillStyle = '#4ade80';
      ctx.font = 'bold 11px monospace';
      ctx.fillText('CORRIDORKEY GREEN SCREEN SOURCE (ACTIVATE FX)', 22, 30);
      return;
    }

    // Default: Broadcast SMPTE Color Bars + Animated Radar Slate
    const bars = ['#c0c0c0', '#c0c000', '#00c0c0', '#00c000', '#c000c0', '#c00000', '#0000c0'];
    const barW = w / bars.length;
    bars.forEach((col, i) => {
      ctx.fillStyle = col;
      ctx.fillRect(i * barW, 0, barW + 1, h * 0.72);
    });
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, h * 0.72, w, h * 0.28);

    // Animated sweep indicator
    const sweepX = ((localT * 160) % (w - 40)) + 20;
    ctx.fillStyle = '#3b82f6';
    ctx.fillRect(sweepX - 12, h * 0.76, 24, 8);

    ctx.fillStyle = '#f8fafc';
    ctx.font = 'bold 14px monospace';
    ctx.fillText(`VEGAS HD SMPTE TEST PATTERN — ${clip.name}`, 18, h * 0.87);
    ctx.fillStyle = '#94a3b8';
    ctx.font = '11px monospace';
    ctx.fillText(`TC: ${formatSMPTE(localT, 29.97, true)} | RATE: ${(clip.playbackRate || 1).toFixed(2)}x`, 18, h * 0.94);
  }

  private drawGridOverlay(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    ctx.save();
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.32)';
    ctx.lineWidth = 1;
    for (let i = 1; i <= 2; i++) {
      ctx.beginPath();
      ctx.moveTo((w * i) / 3, 0);
      ctx.lineTo((w * i) / 3, h);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(0, (h * i) / 3);
      ctx.lineTo(w, (h * i) / 3);
      ctx.stroke();
    }
    ctx.restore();
  }

  private drawSafeAreasOverlay(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    ctx.save();
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.45)';
    ctx.setLineDash([4, 4]);
    ctx.strokeRect(w * 0.05, h * 0.05, w * 0.9, h * 0.9); // 90% Action Safe
    ctx.strokeStyle = 'rgba(250, 204, 21, 0.45)';
    ctx.strokeRect(w * 0.1, h * 0.1, w * 0.8, h * 0.8); // 80% Title Safe
    ctx.setLineDash([]);
    ctx.restore();
  }

  private drawPanCropGizmo(ctx: CanvasRenderingContext2D, clip: ClipEvent, w: number, h: number): void {
    const pc = clip.panCrop || { x: 0, y: 0, scale: 1, rotation: 0, flipH: false, flipV: false };
    ctx.save();
    const cx = w / 2 + pc.x * (w * 0.5);
    const cy = h / 2 + pc.y * (h * 0.5);
    const boxW = w * 0.82 * (pc.scale || 1);
    const boxH = h * 0.82 * (pc.scale || 1);

    ctx.translate(cx, cy);
    ctx.rotate((pc.rotation * Math.PI) / 180);

    ctx.strokeStyle = '#38bdf8';
    ctx.lineWidth = 1.5;
    ctx.setLineDash([5, 3]);
    ctx.strokeRect(-boxW / 2, -boxH / 2, boxW, boxH);
    ctx.setLineDash([]);

    // Vegas iconic "F" orientation watermark inside Pan/Crop box
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.35)';
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(-14, 18);
    ctx.lineTo(-14, -18);
    ctx.lineTo(14, -18);
    ctx.moveTo(-14, 0);
    ctx.lineTo(8, 0);
    ctx.stroke();

    // Corner handles
    const corners = [
      [-boxW / 2, -boxH / 2],
      [boxW / 2, -boxH / 2],
      [boxW / 2, boxH / 2],
      [-boxW / 2, boxH / 2],
    ];
    ctx.fillStyle = '#38bdf8';
    for (const [hx, hy] of corners) {
      ctx.fillRect(hx - 4, hy - 4, 8, 8);
    }
    ctx.restore();
  }

  public updateVideoScopes(): void {
    if (!this.scopeCanvas) return;
    const sctx = this.scopeCanvas.getContext('2d');
    if (!sctx) return;

    const sw = this.scopeCanvas.width;
    const sh = this.scopeCanvas.height;
    sctx.fillStyle = '#080b11';
    sctx.fillRect(0, 0, sw, sh);

    const w = this.canvas.width;
    const h = this.canvas.height;
    if (w === 0 || h === 0) return;

    const img = this.ctx.getImageData(0, 0, w, h).data;
    const thirdW = sw / 3;

    // Horizontal IRE graticule lines (100, 75, 50, 25, 0 IRE) + vertical R | G | B dividers
    sctx.strokeStyle = 'rgba(148, 163, 184, 0.22)';
    sctx.lineWidth = 1;
    sctx.beginPath();
    for (const frac of [0.12, 0.34, 0.55, 0.76, 0.94]) {
      const gy = Math.round(sh * frac) + 0.5;
      sctx.moveTo(0, gy);
      sctx.lineTo(sw, gy);
    }
    sctx.moveTo(Math.round(thirdW) + 0.5, 0);
    sctx.lineTo(Math.round(thirdW) + 0.5, sh);
    sctx.moveTo(Math.round(thirdW * 2) + 0.5, 0);
    sctx.lineTo(Math.round(thirdW * 2) + 0.5, sh);
    sctx.stroke();

    // Channel badges R, G, B at top-left of each third
    sctx.font = 'bold 8px Consolas, monospace';
    sctx.fillStyle = '#f87171';
    sctx.fillText('R', 4, 9);
    sctx.fillStyle = '#4ade80';
    sctx.fillText('G', thirdW + 4, 9);
    sctx.fillStyle = '#60a5fa';
    sctx.fillText('B', thirdW * 2 + 4, 9);

    const stepX = Math.max(1, Math.floor(w / 54));
    const stepY = Math.max(1, Math.floor(h / 36));

    let maxLuma = 0;
    for (let x = 0; x < w; x += stepX) {
      const normX = x / w;
      for (let y = 0; y < h; y += stepY) {
        const idx = (y * w + x) * 4;
        const r = img[idx];
        const g = img[idx + 1];
        const b = img[idx + 2];
        if (r + g + b > maxLuma) maxLuma = r + g + b;

        const ry = sh - 5 - (r / 255) * (sh - 16);
        const gy = sh - 5 - (g / 255) * (sh - 16);
        const by = sh - 5 - (b / 255) * (sh - 16);

        sctx.fillStyle = 'rgba(248, 113, 113, 0.42)';
        sctx.fillRect(normX * (thirdW - 6) + 3, ry, 2, 2);

        sctx.fillStyle = 'rgba(74, 222, 128, 0.42)';
        sctx.fillRect(thirdW + normX * (thirdW - 6) + 3, gy, 2, 2);

        sctx.fillStyle = 'rgba(96, 165, 250, 0.42)';
        sctx.fillRect(thirdW * 2 + normX * (thirdW - 6) + 3, by, 2, 2);
      }
    }

    // If canvas is black/standby (empty timeline), draw subtle broadcast reference waveform so scope looks clean
    if (maxLuma < 15) {
      for (let i = 0; i < thirdW - 6; i += 2) {
        const phase = (i / (thirdW - 6)) * Math.PI * 2;
        const ry = sh * 0.56 + Math.sin(phase) * (sh * 0.22);
        const gy = sh * 0.54 + Math.cos(phase * 1.3) * (sh * 0.22);
        const by = sh * 0.58 + Math.sin(phase * 0.8 + 1.1) * (sh * 0.22);
        sctx.fillStyle = 'rgba(248, 113, 113, 0.55)';
        sctx.fillRect(3 + i, ry, 2, 2);
        sctx.fillStyle = 'rgba(74, 222, 128, 0.55)';
        sctx.fillRect(thirdW + 3 + i, gy, 2, 2);
        sctx.fillStyle = 'rgba(96, 165, 250, 0.55)';
        sctx.fillRect(thirdW * 2 + 3 + i, by, 2, 2);
      }
    }
  }
}
