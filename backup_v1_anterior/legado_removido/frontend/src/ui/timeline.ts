/**
 * OpenVegas Pro — Interactive Multi-Track Timeline Canvas & Event Manipulation Engine
 * Implements Vegas Pro's signature editing paradigm:
 * - Individual & Global Track Height Resizing (Drag track bottom border, +/- buttons, or Alt+Wheel)
 * - Direct Media / Generator / FX / Transition / OS File Drag-and-Drop onto Timeline with Ghost Preview
 * - Automatic Overlap Crossfades (X-curve)
 * - Top-corner interactive Fade-In / Fade-Out handles with 5 Vegas curve shapes
 * - Clip Opacity / Gain horizontal envelope bar
 * - Edge Trimming, Snap to Playhead/Events/Markers (F8), Auto-Ripple (Ctrl+L)
 * - On-clip FX & Pan/Crop badges, Track Automation Envelopes (V), Markers (M) & Loop Region (Q)
 */

import {
  ClipEvent,
  CrossfadeOverlap,
  EditToolMode,
  FadeCurveType,
  TimelineDragPayload,
  Track,
  VegasProject,
} from '../types';
import { evaluateFadeCurve, formatSMPTE } from '../engine/compositor';

export const RULER_HEIGHT = 38;
export const DEFAULT_TRACK_HEIGHT = 76;
export const MIN_TRACK_HEIGHT = 44;
export const MAX_TRACK_HEIGHT = 220;

type DragMode =
  | 'none'
  | 'scrub'
  | 'loopStart'
  | 'loopEnd'
  | 'moveClip'
  | 'trimLeft'
  | 'trimRight'
  | 'fadeIn'
  | 'fadeOut'
  | 'clipGain'
  | 'slipClip'
  | 'rateStretch'
  | 'envelopePoint'
  | 'resizeTrackHeight';

export interface TimelineCallbacks {
  onPlayheadChange: (newTime: number) => void;
  onSelectClip: (clip: ClipEvent | null, track: Track | null) => void;
  onProjectMutated: (reason: string) => void;
  onOpenClipFX: (clip: ClipEvent, track: Track) => void;
  onOpenClipPanCrop: (clip: ClipEvent, track: Track) => void;
  onContextMenuClip: (clip: ClipEvent, track: Track, clientX: number, clientY: number) => void;
  onRazorSplitAtTime?: (clip: ClipEvent, track: Track, splitTime: number) => void;
  onTrackHeightChanged?: () => void;
  onTimelineDrop?: (dropTime: number, targetTrack: Track | null, event: DragEvent) => void;
  getActiveDragPayload?: () => TimelineDragPayload | null;
}

export class VegasTimelineController {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private project: VegasProject;
  private callbacks: TimelineCallbacks;

  public pixelsPerSecond = 88;
  public scrollX = 0;
  public selectedClipId: string | null = null;
  public selectedTrackId: string | null = null;
  public editTool: EditToolMode = 'normal';
  public lightThemeCanvas = false;

  private dragMode: DragMode = 'none';
  private dragClip: ClipEvent | null = null;
  private dragTrack: Track | null = null;
  private dragEnvelopeIdx = -1;
  private dragStartX = 0;
  private dragStartY = 0;
  private origStart = 0;
  private origDuration = 0;
  private origTrimIn = 0;
  private origFadeIn = 0;
  private origFadeOut = 0;
  private origGain = 1;
  private origTrackHeight = DEFAULT_TRACK_HEIGHT;
  private hoverStatus = 'Ready';

  // Live Ghost Drop Preview when dragging media/FX/files over the timeline
  private dropPreview: {
    active: boolean;
    time: number;
    duration: number;
    trackIdx: number;
    label: string;
    color: string;
  } | null = null;

  constructor(
    canvas: HTMLCanvasElement,
    project: VegasProject,
    callbacks: TimelineCallbacks
  ) {
    this.canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to initialize timeline 2D context');
    this.ctx = ctx;
    this.project = project;
    this.callbacks = callbacks;

    this.bindEvents();
  }

  public setProject(project: VegasProject): void {
    this.project = project;
    this.recalculateAutoCrossfades();
    this.render();
  }

  public getTrackHeight(track: Track): number {
    return Math.max(MIN_TRACK_HEIGHT, Math.min(MAX_TRACK_HEIGHT, Math.round(track.height || DEFAULT_TRACK_HEIGHT)));
  }

  public getTrackTop(trackIdx: number): number {
    let y = RULER_HEIGHT;
    for (let i = 0; i < trackIdx && i < this.project.tracks.length; i++) {
      y += this.getTrackHeight(this.project.tracks[i]);
    }
    return y;
  }

  public getTotalTracksHeight(): number {
    let sum = 0;
    for (const tr of this.project.tracks) {
      sum += this.getTrackHeight(tr);
    }
    return sum;
  }

  public findTrackAtY(y: number): {
    track: Track;
    trackIdx: number;
    trackTop: number;
    trackH: number;
    nearBottomBorder: boolean;
  } | null {
    if (y <= RULER_HEIGHT) return null;
    let curTop = RULER_HEIGHT;
    for (let i = 0; i < this.project.tracks.length; i++) {
      const tr = this.project.tracks[i];
      const h = this.getTrackHeight(tr);
      const bottom = curTop + h;
      if (y >= curTop && y <= bottom + 4) {
        const nearBottomBorder = Math.abs(y - bottom) <= 5;
        return { track: tr, trackIdx: i, trackTop: curTop, trackH: h, nearBottomBorder };
      }
      curTop = bottom;
    }
    return null;
  }

  public adjustAllTrackHeights(deltaPx: number, filterType?: 'video' | 'audio'): void {
    for (const tr of this.project.tracks) {
      if (filterType && tr.type !== filterType) continue;
      tr.height = Math.max(
        MIN_TRACK_HEIGHT,
        Math.min(MAX_TRACK_HEIGHT, this.getTrackHeight(tr) + deltaPx)
      );
    }
    this.callbacks.onTrackHeightChanged?.();
    this.render();
  }

  public adjustSingleTrackHeight(trackId: string, deltaPx: number): void {
    const tr = this.project.tracks.find((t) => t.id === trackId);
    if (!tr) return;
    tr.height = Math.max(
      MIN_TRACK_HEIGHT,
      Math.min(MAX_TRACK_HEIGHT, this.getTrackHeight(tr) + deltaPx)
    );
    this.callbacks.onTrackHeightChanged?.();
    this.render();
  }

  public timeToX(sec: number): number {
    return (sec - this.scrollX) * this.pixelsPerSecond;
  }

  public xToTime(x: number): number {
    return Math.max(0, this.scrollX + x / this.pixelsPerSecond);
  }

  public snapTime(targetSec: number, excludeClipId?: string): number {
    if (!this.project.snapEnabled) return Math.max(0, targetSec);
    const thresholdSec = 8 / this.pixelsPerSecond;
    const candidates: number[] = [
      0,
      this.project.playhead,
      this.project.loopStart,
      this.project.loopEnd,
    ];
    for (const m of this.project.markers) candidates.push(m.time);
    for (const r of this.project.regions) {
      candidates.push(r.start, r.end);
    }
    for (const tr of this.project.tracks) {
      for (const c of tr.clips) {
        if (c.id === excludeClipId) continue;
        candidates.push(c.start, c.start + c.duration);
      }
    }
    const nearestGrid = Math.round(targetSec * 2) / 2;
    candidates.push(nearestGrid);

    let best = targetSec;
    let minDiff = thresholdSec;
    for (const cand of candidates) {
      const diff = Math.abs(cand - targetSec);
      if (diff < minDiff) {
        minDiff = diff;
        best = cand;
      }
    }
    return Math.max(0, Number(best.toFixed(3)));
  }

  public recalculateAutoCrossfades(): CrossfadeOverlap[] {
    const overlaps: CrossfadeOverlap[] = [];
    if (!this.project.autoCrossfadeEnabled) return overlaps;

    this.project.tracks.forEach((track, tIdx) => {
      track.clips.sort((a, b) => a.start - b.start);
      for (let i = 0; i < track.clips.length; i++) {
        for (let j = i + 1; j < track.clips.length; j++) {
          const outClip = track.clips[i];
          const inClip = track.clips[j];
          const outEnd = outClip.start + outClip.duration;
          if (inClip.start < outEnd - 0.008) {
            const overlapEnd = Math.min(outEnd, inClip.start + inClip.duration);
            const overlapDur = Math.max(0, overlapEnd - inClip.start);
            if (overlapDur > 0.01) {
              outClip.fadeOut = Number(overlapDur.toFixed(3));
              inClip.fadeIn = Number(overlapDur.toFixed(3));
              overlaps.push({
                trackId: track.id,
                trackIndex: tIdx,
                outgoingClipId: outClip.id,
                incomingClipId: inClip.id,
                start: inClip.start,
                end: overlapEnd,
                duration: overlapDur,
                outCurve: outClip.fadeOutCurve,
                inCurve: inClip.fadeInCurve,
              });
            }
          }
        }
      }
    });
    return overlaps;
  }

  private bindEvents(): void {
    window.addEventListener('resize', () => this.render());

    this.canvas.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault();
        if (e.altKey) {
          // Alt + Wheel: Increase / Decrease Track Heights vertically
          const delta = e.deltaY < 0 ? 10 : -10;
          this.adjustAllTrackHeights(delta);
          return;
        }
        if (e.ctrlKey || e.metaKey || Math.abs(e.deltaY) > 0) {
          if (e.shiftKey) {
            this.scrollX = Math.max(0, this.scrollX + (e.deltaY > 0 ? 0.8 : -0.8));
          } else {
            const mouseTime = this.xToTime(e.offsetX);
            const factor = e.deltaY < 0 ? 1.15 : 0.87;
            this.pixelsPerSecond = Math.max(24, Math.min(360, this.pixelsPerSecond * factor));
            this.scrollX = Math.max(0, mouseTime - e.offsetX / this.pixelsPerSecond);
          }
          this.render();
        }
      },
      { passive: false }
    );

    this.canvas.addEventListener('mousedown', (e) => this.handleMouseDown(e));
    window.addEventListener('mousemove', (e) => this.handleMouseMove(e));
    window.addEventListener('mouseup', () => this.handleMouseUp());

    // Direct Drag-and-Drop Media / Generators / FX / Files onto Timeline Canvas
    this.canvas.addEventListener('dragover', (e) => {
      e.preventDefault();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = 'copy';
      }
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const dropTime = this.snapTime(this.xToTime(x));
      const found = this.findTrackAtY(y);
      const trackIdx = found ? found.trackIdx : Math.max(0, this.project.tracks.length - 1);

      const payload = this.callbacks.getActiveDragPayload?.();
      const label = payload ? payload.name : 'Soltar Mídia / Efeito Aqui';
      const dur = payload?.duration || 5.0;
      const color = payload?.color || '#38bdf8';

      this.dropPreview = {
        active: true,
        time: dropTime,
        duration: dur,
        trackIdx,
        label,
        color,
      };
      this.render();
    });

    this.canvas.addEventListener('dragleave', () => {
      if (this.dropPreview) {
        this.dropPreview = null;
        this.render();
      }
    });

    this.canvas.addEventListener('drop', (e) => {
      e.preventDefault();
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const dropTime = this.snapTime(this.xToTime(x));
      const found = this.findTrackAtY(y);
      this.dropPreview = null;
      this.callbacks.onTimelineDrop?.(dropTime, found ? found.track : null, e);
      this.render();
    });

    this.canvas.addEventListener('dblclick', (e) => {
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const found = this.findTrackAtY(y);
      if (found && found.track.showEnvelope) {
        const relY = y - found.trackTop;
        const val = Math.max(0, Math.min(1, 1 - (relY - 12) / (found.trackH - 24)));
        found.track.envelopePoints.push({
          time: Number(this.xToTime(x).toFixed(3)),
          value: Number(val.toFixed(3)),
        });
        found.track.envelopePoints.sort((a, b) => a.time - b.time);
        this.callbacks.onProjectMutated('Added automation envelope point');
        this.render();
      }
    });

    this.canvas.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const hit = this.hitTestClip(x, y);
      if (hit) {
        this.selectedClipId = hit.clip.id;
        this.selectedTrackId = hit.track.id;
        this.callbacks.onSelectClip(hit.clip, hit.track);
        this.render();
        this.callbacks.onContextMenuClip(hit.clip, hit.track, e.clientX, e.clientY);
      }
    });
  }

  private hitTestClip(
    x: number,
    y: number
  ): {
    clip: ClipEvent;
    track: Track;
    trackIdx: number;
    clipX: number;
    clipW: number;
    trackTop: number;
    trackH: number;
  } | null {
    const found = this.findTrackAtY(y);
    if (!found) return null;
    const { track, trackIdx, trackTop, trackH } = found;

    for (let i = track.clips.length - 1; i >= 0; i--) {
      const clip = track.clips[i];
      const cx = this.timeToX(clip.start);
      const cw = Math.max(12, clip.duration * this.pixelsPerSecond);
      if (x >= cx && x <= cx + cw && y >= trackTop + 4 && y <= trackTop + trackH - 4) {
        return { clip, track, trackIdx, clipX: cx, clipW: cw, trackTop, trackH };
      }
    }
    return null;
  }

  private handleMouseDown(e: MouseEvent): void {
    if (e.button !== 0) return;
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    this.dragStartX = x;
    this.dragStartY = y;

    // 1. Check Ruler interaction (Loop handles or Playhead scrub)
    if (y <= RULER_HEIGHT) {
      const loopStartX = this.timeToX(this.project.loopStart);
      const loopEndX = this.timeToX(this.project.loopEnd);
      if (y <= 16 && Math.abs(x - loopStartX) <= 8) {
        this.dragMode = 'loopStart';
        return;
      }
      if (y <= 16 && Math.abs(x - loopEndX) <= 8) {
        this.dragMode = 'loopEnd';
        return;
      }
      this.dragMode = 'scrub';
      this.callbacks.onPlayheadChange(this.xToTime(x));
      this.render();
      return;
    }

    // 2. Check Track Bottom Divider Resize or Track Envelope Point
    const found = this.findTrackAtY(y);
    if (found) {
      const { track, trackTop, trackH, nearBottomBorder } = found;
      this.selectedTrackId = track.id;

      if (nearBottomBorder) {
        this.dragMode = 'resizeTrackHeight';
        this.dragTrack = track;
        this.origTrackHeight = trackH;
        return;
      }

      if (track.showEnvelope) {
        for (let i = 0; i < track.envelopePoints.length; i++) {
          const pt = track.envelopePoints[i];
          const px = this.timeToX(pt.time);
          const py = trackTop + 12 + (1 - pt.value) * (trackH - 24);
          if (Math.hypot(x - px, y - py) <= 8) {
            this.dragMode = 'envelopePoint';
            this.dragTrack = track;
            this.dragEnvelopeIdx = i;
            return;
          }
        }
      }
    }

    // 3. Check Clip hit test
    const hit = this.hitTestClip(x, y);
    if (!hit) {
      this.selectedClipId = null;
      this.callbacks.onSelectClip(null, found?.track || null);
      this.dragMode = 'scrub';
      this.callbacks.onPlayheadChange(this.xToTime(x));
      this.render();
      return;
    }

    const { clip, clipX, clipW, trackTop, trackH } = hit;
    this.selectedClipId = clip.id;
    this.selectedTrackId = hit.track.id;
    this.callbacks.onSelectClip(clip, hit.track);

    if (hit.track.locked || clip.locked) {
      this.render();
      return;
    }

    // Check On-Clip Mini Badges (Top Right of clip: Pan/Crop & Event FX buttons)
    const relY = y - (trackTop + 4);
    if (relY <= 18 && clipW > 70) {
      const fxBtnX = clipX + clipW - 22;
      const cropBtnX = clipX + clipW - 42;
      if (x >= fxBtnX && x <= fxBtnX + 18) {
        this.callbacks.onOpenClipFX(clip, hit.track);
        return;
      }
      if (hit.track.type === 'video' && x >= cropBtnX && x <= cropBtnX + 18) {
        this.callbacks.onOpenClipPanCrop(clip, hit.track);
        return;
      }
    }

    // If Razor Tool is active, split immediately at click point
    if (this.editTool === 'razor') {
      const clickTime = this.snapTime(this.xToTime(x));
      this.callbacks.onRazorSplitAtTime?.(clip, hit.track, clickTime);
      return;
    }

    this.dragClip = clip;
    this.dragTrack = hit.track;
    this.origStart = clip.start;
    this.origDuration = clip.duration;
    this.origTrimIn = clip.trimIn;
    this.origFadeIn = clip.fadeIn;
    this.origFadeOut = clip.fadeOut;
    this.origGain = clip.gain;

    if (this.editTool === 'slip') {
      this.dragMode = 'slipClip';
      this.render();
      return;
    }
    if (this.editTool === 'rateStretch' || e.ctrlKey) {
      this.dragMode = 'rateStretch';
      this.render();
      return;
    }

    const fadeInHandleX = clipX + clip.fadeIn * this.pixelsPerSecond;
    const fadeOutHandleX = clipX + clipW - clip.fadeOut * this.pixelsPerSecond;

    if (relY <= 18 && Math.abs(x - fadeInHandleX) <= 10) {
      this.dragMode = 'fadeIn';
    } else if (relY <= 18 && Math.abs(x - fadeOutHandleX) <= 10) {
      this.dragMode = 'fadeOut';
    } else if (Math.abs(x - clipX) <= 7) {
      this.dragMode = 'trimLeft';
    } else if (Math.abs(x - (clipX + clipW)) <= 7) {
      this.dragMode = 'trimRight';
    } else {
      const gainNorm = Math.max(0, Math.min(1, clip.gain));
      const gainLineY = trackTop + 22 + (1 - gainNorm) * (trackH - 30);
      if (Math.abs(y - gainLineY) <= 5 && relY > 18) {
        this.dragMode = 'clipGain';
      } else {
        this.dragMode = 'moveClip';
      }
    }

    this.render();
  }

  private handleMouseMove(e: MouseEvent): void {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (this.dragMode === 'none') {
      this.updateHoverCursor(x, y);
      return;
    }

    if (this.dragMode === 'resizeTrackHeight' && this.dragTrack) {
      const dy = y - this.dragStartY;
      this.dragTrack.height = Math.max(
        MIN_TRACK_HEIGHT,
        Math.min(MAX_TRACK_HEIGHT, Math.round(this.origTrackHeight + dy))
      );
      this.callbacks.onTrackHeightChanged?.();
      this.render();
      return;
    }

    const dxSec = (x - this.dragStartX) / this.pixelsPerSecond;

    if (this.dragMode === 'scrub') {
      this.callbacks.onPlayheadChange(this.xToTime(x));
      this.render();
      return;
    }

    if (this.dragMode === 'loopStart') {
      this.project.loopStart = Math.max(0, Math.min(this.project.loopEnd - 0.2, this.snapTime(this.xToTime(x))));
      this.render();
      return;
    }

    if (this.dragMode === 'loopEnd') {
      this.project.loopEnd = Math.max(this.project.loopStart + 0.2, this.snapTime(this.xToTime(x)));
      this.render();
      return;
    }

    if (this.dragMode === 'envelopePoint' && this.dragTrack) {
      const tIdx = this.project.tracks.indexOf(this.dragTrack);
      const trackTop = this.getTrackTop(tIdx);
      const trackH = this.getTrackHeight(this.dragTrack);
      const pt = this.dragTrack.envelopePoints[this.dragEnvelopeIdx];
      if (pt) {
        pt.time = Number(Math.max(0, this.xToTime(x)).toFixed(3));
        pt.value = Number(Math.max(0, Math.min(1, 1 - (y - trackTop - 12) / (trackH - 24))).toFixed(3));
        this.render();
      }
      return;
    }

    if (!this.dragClip || !this.dragTrack) return;
    const clip = this.dragClip;

    if (this.dragMode === 'moveClip') {
      const rawStart = Math.max(0, this.origStart + dxSec);
      const snappedStart = this.snapTime(rawStart, clip.id);
      const deltaStart = snappedStart - clip.start;
      clip.start = snappedStart;

      if (this.project.rippleEnabled && Math.abs(deltaStart) > 0.0001) {
        for (const other of this.dragTrack.clips) {
          if (other.id !== clip.id && other.start > this.origStart) {
            other.start = Math.max(0, Number((other.start + deltaStart).toFixed(3)));
          }
        }
      }

      const foundTarget = this.findTrackAtY(y);
      if (foundTarget) {
        const targetTrack = foundTarget.track;
        if (
          targetTrack &&
          targetTrack.id !== this.dragTrack.id &&
          targetTrack.type === this.dragTrack.type &&
          !targetTrack.locked
        ) {
          this.dragTrack.clips = this.dragTrack.clips.filter((c) => c.id !== clip.id);
          targetTrack.clips.push(clip);
          this.dragTrack = targetTrack;
          this.selectedTrackId = targetTrack.id;
        }
      }

      this.recalculateAutoCrossfades();
      this.render();
    } else if (this.dragMode === 'trimLeft') {
      const maxDelta = this.origDuration - 0.15;
      const clampedDelta = Math.max(-this.origStart, Math.min(maxDelta, dxSec));
      const newStart = this.snapTime(this.origStart + clampedDelta, clip.id);
      const realDelta = newStart - this.origStart;
      clip.start = Number(newStart.toFixed(3));
      clip.duration = Number(Math.max(0.15, this.origDuration - realDelta).toFixed(3));
      clip.trimIn = Number(Math.max(0, this.origTrimIn + realDelta).toFixed(3));
      this.recalculateAutoCrossfades();
      this.render();
    } else if (this.dragMode === 'trimRight') {
      const rawEnd = Math.max(clip.start + 0.15, this.origStart + this.origDuration + dxSec);
      const snappedEnd = this.snapTime(rawEnd, clip.id);
      clip.duration = Number(Math.max(0.15, snappedEnd - clip.start).toFixed(3));
      this.recalculateAutoCrossfades();
      this.render();
    } else if (this.dragMode === 'fadeIn') {
      clip.fadeIn = Number(Math.max(0, Math.min(clip.duration - 0.05, this.origFadeIn + dxSec)).toFixed(3));
      this.render();
    } else if (this.dragMode === 'fadeOut') {
      clip.fadeOut = Number(Math.max(0, Math.min(clip.duration - 0.05, this.origFadeOut - dxSec)).toFixed(3));
      this.render();
    } else if (this.dragMode === 'clipGain') {
      const dy = this.dragStartY - y;
      clip.gain = Number(Math.max(0, Math.min(1.5, this.origGain + dy / 50)).toFixed(2));
      this.render();
    } else if (this.dragMode === 'slipClip') {
      clip.trimIn = Number(Math.max(0, this.origTrimIn - dxSec).toFixed(3));
      this.render();
    } else if (this.dragMode === 'rateStretch') {
      const newDur = Math.max(0.25, this.origDuration + dxSec);
      const ratio = this.origDuration / newDur;
      clip.duration = Number(newDur.toFixed(3));
      clip.playbackRate = Number(Math.max(0.25, Math.min(4.0, ratio)).toFixed(2));
      this.recalculateAutoCrossfades();
      this.render();
    }
  }

  private handleMouseUp(): void {
    if (this.dragMode !== 'none') {
      const mode = this.dragMode;
      this.dragMode = 'none';
      this.dragClip = null;
      this.dragTrack = null;
      if (mode !== 'scrub' && mode !== 'resizeTrackHeight') {
        this.recalculateAutoCrossfades();
        this.callbacks.onProjectMutated(`Timeline edit (${mode})`);
      }
      this.render();
    }
  }

  private updateHoverCursor(x: number, y: number): void {
    if (y <= RULER_HEIGHT) {
      this.canvas.style.cursor = 'pointer';
      return;
    }
    const found = this.findTrackAtY(y);
    if (found && found.nearBottomBorder) {
      this.canvas.style.cursor = 'ns-resize';
      this.hoverStatus = 'Arraste para aumentar/diminuir a altura da faixa';
      return;
    }

    const hit = this.hitTestClip(x, y);
    if (!hit) {
      this.canvas.style.cursor = 'default';
      return;
    }
    const { clip, clipX, clipW, trackTop, trackH } = hit;
    const relY = y - (trackTop + 4);
    const fadeInX = clipX + clip.fadeIn * this.pixelsPerSecond;
    const fadeOutX = clipX + clipW - clip.fadeOut * this.pixelsPerSecond;

    if (relY <= 18 && (Math.abs(x - fadeInX) <= 10 || Math.abs(x - fadeOutX) <= 10)) {
      this.canvas.style.cursor = 'ew-resize';
      this.hoverStatus = 'Drag corner to adjust Vegas Fade-In / Fade-Out envelope';
    } else if (Math.abs(x - clipX) <= 7 || Math.abs(x - (clipX + clipW)) <= 7) {
      this.canvas.style.cursor = 'col-resize';
      this.hoverStatus = 'Drag edge to Trim event start/end';
    } else {
      const gainNorm = Math.max(0, Math.min(1, clip.gain));
      const gainLineY = trackTop + 22 + (1 - gainNorm) * (trackH - 30);
      if (Math.abs(y - gainLineY) <= 5 && relY > 18) {
        this.canvas.style.cursor = 'ns-resize';
      } else {
        this.canvas.style.cursor = 'grab';
      }
    }
  }

  public render(): void {
    const parent = this.canvas.parentElement;
    if (parent) {
      const dpr = window.devicePixelRatio || 1;
      const targetW = parent.clientWidth;
      const targetH = Math.max(
        parent.clientHeight,
        RULER_HEIGHT + this.getTotalTracksHeight() + 24
      );
      if (this.canvas.width !== Math.floor(targetW * dpr) || this.canvas.height !== Math.floor(targetH * dpr)) {
        this.canvas.width = Math.floor(targetW * dpr);
        this.canvas.height = Math.floor(targetH * dpr);
        this.canvas.style.width = `${targetW}px`;
        this.canvas.style.height = `${targetH}px`;
      }
      this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    const w = this.canvas.clientWidth || 900;
    const h = this.canvas.clientHeight || 360;
    const ctx = this.ctx;

    // 1. Background
    ctx.fillStyle = this.lightThemeCanvas ? '#d8d5ce' : '#14161b';
    ctx.fillRect(0, 0, w, h);

    // 2. Vertical Time Grid Lines
    this.drawTimeGrid(ctx, w, h);

    // 3. Track Lanes & Clips (with dynamic per-track height!)
    let curTop = RULER_HEIGHT;
    this.project.tracks.forEach((track, idx) => {
      const trackH = this.getTrackHeight(track);
      this.drawTrackRow(ctx, track, idx, curTop, trackH, w);
      curTop += trackH;
    });

    // 4. Ghost Drag-and-Drop Preview Box
    if (this.dropPreview && this.dropPreview.active) {
      const dp = this.dropPreview;
      const trIdx = Math.max(0, Math.min(this.project.tracks.length - 1, dp.trackIdx));
      const tr = this.project.tracks[trIdx];
      const top = this.getTrackTop(trIdx);
      const trH = tr ? this.getTrackHeight(tr) : DEFAULT_TRACK_HEIGHT;
      const gx = this.timeToX(dp.time);
      const gw = Math.max(60, dp.duration * this.pixelsPerSecond);

      ctx.save();
      ctx.fillStyle = 'rgba(56, 189, 248, 0.24)';
      ctx.fillRect(gx, top + 4, gw, trH - 8);
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 2;
      ctx.setLineDash([5, 3]);
      ctx.strokeRect(gx, top + 4, gw, trH - 8);
      ctx.setLineDash([]);
      ctx.fillStyle = '#ffffff';
      ctx.font = 'bold 11px "Segoe UI", sans-serif';
      ctx.fillText(`+ ${dp.label} [${formatSMPTE(dp.time, this.project.fps)}]`, gx + 6, top + 20);
      ctx.restore();
    }

    // 5. Loop Region Overlay across tracks
    const loopX1 = this.timeToX(this.project.loopStart);
    const loopX2 = this.timeToX(this.project.loopEnd);
    if (loopX2 > loopX1) {
      ctx.fillStyle = this.project.loopEnabled
        ? 'rgba(56, 189, 248, 0.07)'
        : 'rgba(148, 163, 184, 0.03)';
      ctx.fillRect(loopX1, RULER_HEIGHT, loopX2 - loopX1, h - RULER_HEIGHT);
      ctx.strokeStyle = this.project.loopEnabled ? 'rgba(56, 189, 248, 0.45)' : 'rgba(148, 163, 184, 0.25)';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(loopX1, RULER_HEIGHT);
      ctx.lineTo(loopX1, h);
      ctx.moveTo(loopX2, RULER_HEIGHT);
      ctx.lineTo(loopX2, h);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // 6. Top Timecode Ruler, Loop Bar, Regions & Markers
    this.drawRuler(ctx, w, h);

    // 7. Playhead Cursor
    const playX = this.timeToX(this.project.playhead);
    if (playX >= -10 && playX <= w + 10) {
      ctx.save();
      ctx.strokeStyle = '#f8fafc';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(playX, 12);
      ctx.lineTo(playX, h);
      ctx.stroke();

      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(playX - 7, 14);
      ctx.lineTo(playX + 7, 14);
      ctx.lineTo(playX, 25);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
    }
  }

  private drawTimeGrid(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    const stepSec = this.pixelsPerSecond > 140 ? 0.5 : this.pixelsPerSecond > 65 ? 1 : 2;
    const firstSec = Math.floor(this.scrollX / stepSec) * stepSec;
    const endSec = this.xToTime(w);

    ctx.lineWidth = 1;
    for (let s = firstSec; s <= endSec; s += stepSec) {
      const x = Math.round(this.timeToX(s)) + 0.5;
      if (x < 0) continue;
      const isMajor = Math.abs(s % (stepSec * 2)) < 0.001;
      ctx.strokeStyle = isMajor ? 'rgba(255,255,255,0.07)' : 'rgba(255,255,255,0.03)';
      ctx.beginPath();
      ctx.moveTo(x, RULER_HEIGHT);
      ctx.lineTo(x, h);
      ctx.stroke();
    }
  }

  private drawRuler(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    ctx.fillStyle = this.lightThemeCanvas ? '#c6c3bc' : '#1d2028';
    ctx.fillRect(0, 0, w, RULER_HEIGHT);
    ctx.strokeStyle = this.lightThemeCanvas ? '#8c8982' : '#2e3340';
    ctx.beginPath();
    ctx.moveTo(0, RULER_HEIGHT - 0.5);
    ctx.lineTo(w, RULER_HEIGHT - 0.5);
    ctx.stroke();

    const lx1 = this.timeToX(this.project.loopStart);
    const lx2 = this.timeToX(this.project.loopEnd);
    ctx.fillStyle = this.project.loopEnabled ? 'rgba(56, 189, 248, 0.35)' : 'rgba(100, 116, 139, 0.25)';
    ctx.fillRect(lx1, 2, Math.max(4, lx2 - lx1), 10);

    ctx.fillStyle = this.project.loopEnabled ? '#38bdf8' : '#94a3b8';
    ctx.fillRect(lx1 - 3, 1, 6, 12);
    ctx.fillRect(lx2 - 3, 1, 6, 12);

    const stepSec = this.pixelsPerSecond > 140 ? 1 : this.pixelsPerSecond > 60 ? 2 : 5;
    const firstSec = Math.floor(this.scrollX / stepSec) * stepSec;
    const endSec = this.xToTime(w);

    ctx.fillStyle = this.lightThemeCanvas ? '#222222' : '#94a3b8';
    ctx.font = '10px Consolas, monospace';
    for (let s = firstSec; s <= endSec; s += stepSec) {
      const x = Math.round(this.timeToX(s));
      if (x < 0) continue;
      ctx.strokeStyle = '#475569';
      ctx.beginPath();
      ctx.moveTo(x, RULER_HEIGHT - 10);
      ctx.lineTo(x, RULER_HEIGHT);
      ctx.stroke();
      ctx.fillText(formatSMPTE(s, this.project.fps, true), x + 4, RULER_HEIGHT - 4);
    }

    for (const reg of this.project.regions) {
      const rx1 = this.timeToX(reg.start);
      const rx2 = this.timeToX(reg.end);
      ctx.fillStyle = 'rgba(34, 197, 94, 0.22)';
      ctx.fillRect(rx1, 13, Math.max(4, rx2 - rx1), 11);
      ctx.strokeStyle = '#22c55e';
      ctx.strokeRect(rx1, 13, Math.max(4, rx2 - rx1), 11);
      ctx.fillStyle = '#86efac';
      ctx.font = 'bold 9px sans-serif';
      ctx.fillText(reg.label, rx1 + 4, 22);
    }

    for (const marker of this.project.markers) {
      const mx = this.timeToX(marker.time);
      if (mx < -20 || mx > w + 20) continue;
      ctx.strokeStyle = marker.color || '#f97316';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(mx, 14);
      ctx.lineTo(mx, h);
      ctx.stroke();

      const tagW = Math.min(90, marker.label.length * 6 + 14);
      ctx.fillStyle = marker.color || '#f97316';
      ctx.fillRect(mx, 14, tagW, 13);
      ctx.fillStyle = '#090b10';
      ctx.font = 'bold 9px sans-serif';
      ctx.fillText(marker.label, mx + 4, 24);
    }
  }

  private drawTrackRow(
    ctx: CanvasRenderingContext2D,
    track: Track,
    trackIdx: number,
    top: number,
    trackH: number,
    w: number
  ): void {
    ctx.fillStyle = trackIdx % 2 === 0 ? 'rgba(255,255,255,0.015)' : 'rgba(0,0,0,0.12)';
    ctx.fillRect(0, top, w, trackH);

    // Bottom divider
    ctx.strokeStyle = '#242832';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, top + trackH - 0.5);
    ctx.lineTo(w, top + trackH - 0.5);
    ctx.stroke();

    for (const clip of track.clips) {
      this.drawClipEvent(ctx, clip, track, top, trackH);
    }

    for (let i = 0; i < track.clips.length; i++) {
      for (let j = i + 1; j < track.clips.length; j++) {
        const c1 = track.clips[i];
        const c2 = track.clips[j];
        const c1End = c1.start + c1.duration;
        if (c2.start < c1End - 0.01) {
          const ovStart = c2.start;
          const ovEnd = Math.min(c1End, c2.start + c2.duration);
          this.drawCrossfadeXBox(ctx, ovStart, ovEnd, top, trackH, c1.fadeOutCurve, c2.fadeInCurve);
        }
      }
    }

    if (track.showEnvelope && track.envelopePoints.length > 0) {
      ctx.save();
      ctx.strokeStyle = track.type === 'video' ? '#c084fc' : '#38bdf8';
      ctx.lineWidth = 2;
      ctx.beginPath();
      const sorted = [...track.envelopePoints].sort((a, b) => a.time - b.time);
      sorted.forEach((pt, i) => {
        const px = this.timeToX(pt.time);
        const py = top + 12 + (1 - pt.value) * (trackH - 24);
        if (i === 0) {
          ctx.moveTo(0, py);
          ctx.lineTo(px, py);
        } else {
          ctx.lineTo(px, py);
        }
        if (i === sorted.length - 1) {
          ctx.lineTo(w, py);
        }
      });
      ctx.stroke();

      for (const pt of sorted) {
        const px = this.timeToX(pt.time);
        const py = top + 12 + (1 - pt.value) * (trackH - 24);
        ctx.fillStyle = '#f8fafc';
        ctx.fillRect(px - 4, py - 4, 8, 8);
        ctx.strokeStyle = '#9333ea';
        ctx.strokeRect(px - 4, py - 4, 8, 8);
      }
      ctx.restore();
    }
  }

  private drawClipEvent(
    ctx: CanvasRenderingContext2D,
    clip: ClipEvent,
    track: Track,
    trackTop: number,
    trackH: number
  ): void {
    const x = this.timeToX(clip.start);
    const w = Math.max(12, clip.duration * this.pixelsPerSecond);
    const y = trackTop + 3;
    const h = trackH - 6;
    const isSelected = this.selectedClipId === clip.id;

    ctx.save();
    ctx.beginPath();
    ctx.rect(x, y, w, h);
    ctx.clip();

    const baseColor = clip.color || (track.type === 'video' ? '#2563eb' : '#0891b2');
    const bodyGrad = ctx.createLinearGradient(x, y, x, y + h);
    bodyGrad.addColorStop(0, this.hexToRgba(baseColor, clip.muted ? 0.2 : 0.48));
    bodyGrad.addColorStop(1, this.hexToRgba(baseColor, clip.muted ? 0.12 : 0.24));
    ctx.fillStyle = bodyGrad;
    ctx.fillRect(x, y, w, h);

    ctx.fillStyle = this.hexToRgba(baseColor, 0.78);
    ctx.fillRect(x, y, w, 17);

    if (h > 24) {
      if (track.type === 'audio') {
        this.drawAudioWaveformInClip(ctx, clip, x, y + 18, w, h - 19);
      } else {
        this.drawVideoFilmstripInClip(ctx, clip, x, y + 18, w, h - 19);
      }
    }

    const gainNorm = Math.max(0, Math.min(1, clip.gain));
    const gainY = y + 17 + (1 - gainNorm) * Math.max(4, h - 21);
    ctx.strokeStyle = 'rgba(250, 204, 21, 0.75)';
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(x, gainY);
    ctx.lineTo(x + w, gainY);
    ctx.stroke();

    if (clip.fadeIn > 0.01) {
      const fadeW = Math.min(w, clip.fadeIn * this.pixelsPerSecond);
      ctx.beginPath();
      ctx.moveTo(x, y + h);
      const steps = 16;
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeInCurve, ratio) * h;
        ctx.lineTo(x + ratio * fadeW, fy);
      }
      ctx.lineTo(x, y);
      ctx.closePath();
      ctx.fillStyle = 'rgba(9, 11, 16, 0.52)';
      ctx.fill();

      ctx.beginPath();
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeInCurve, ratio) * h;
        if (s === 0) ctx.moveTo(x, fy);
        else ctx.lineTo(x + ratio * fadeW, fy);
      }
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 1.6;
      ctx.stroke();

      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(x + fadeW - 5, y);
      ctx.lineTo(x + fadeW + 5, y);
      ctx.lineTo(x + fadeW, y + 8);
      ctx.closePath();
      ctx.fill();
    } else {
      ctx.fillStyle = 'rgba(255,255,255,0.45)';
      ctx.fillRect(x + 1, y + 1, 5, 5);
    }

    if (clip.fadeOut > 0.01) {
      const fadeW = Math.min(w, clip.fadeOut * this.pixelsPerSecond);
      const startX = x + w - fadeW;
      const steps = 16;
      ctx.beginPath();
      ctx.moveTo(x + w, y + h);
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeOutCurve, 1 - ratio) * h;
        ctx.lineTo(startX + ratio * fadeW, fy);
      }
      ctx.lineTo(x + w, y);
      ctx.closePath();
      ctx.fillStyle = 'rgba(9, 11, 16, 0.52)';
      ctx.fill();

      ctx.beginPath();
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeOutCurve, 1 - ratio) * h;
        if (s === 0) ctx.moveTo(startX, fy);
        else ctx.lineTo(startX + ratio * fadeW, fy);
      }
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 1.6;
      ctx.stroke();

      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(startX - 5, y);
      ctx.lineTo(startX + 5, y);
      ctx.lineTo(startX, y + 8);
      ctx.closePath();
      ctx.fill();
    } else {
      ctx.fillStyle = 'rgba(255,255,255,0.45)';
      ctx.fillRect(x + w - 6, y + 1, 5, 5);
    }

    ctx.fillStyle = '#f8fafc';
    ctx.font = 'bold 10.5px "Segoe UI", sans-serif';
    const gainPct = Math.round(clip.gain * 100);
    const label = `${clip.name} (${gainPct}%)`;
    ctx.fillText(label, x + 7, y + 12);

    if (w > 75) {
      if (track.type === 'video') {
        const cropX = x + w - 42;
        ctx.fillStyle = 'rgba(15, 23, 42, 0.78)';
        ctx.fillRect(cropX, y + 2, 17, 13);
        ctx.fillStyle = '#bae6fd';
        ctx.font = 'bold 9px monospace';
        ctx.fillText('[]', cropX + 3, y + 11);
      }
      const fxX = x + w - 22;
      const hasActiveFx = clip.fx && clip.fx.some((f) => f.enabled);
      ctx.fillStyle = hasActiveFx ? '#2563eb' : 'rgba(15, 23, 42, 0.78)';
      ctx.fillRect(fxX, y + 2, 18, 13);
      ctx.fillStyle = hasActiveFx ? '#ffffff' : '#94a3b8';
      ctx.font = 'bold 9px monospace';
      ctx.fillText('fx', fxX + 3, y + 11);
    }

    ctx.restore();

    ctx.save();
    ctx.strokeStyle = isSelected ? '#facc15' : 'rgba(255,255,255,0.25)';
    ctx.lineWidth = isSelected ? 2 : 1;
    ctx.strokeRect(x, y, w, h);
    ctx.restore();
  }

  private drawAudioWaveformInClip(
    ctx: CanvasRenderingContext2D,
    clip: ClipEvent,
    x: number,
    y: number,
    w: number,
    h: number
  ): void {
    const midY = y + h / 2;
    ctx.strokeStyle = 'rgba(255,255,255,0.12)';
    ctx.beginPath();
    ctx.moveTo(x, midY);
    ctx.lineTo(x + w, midY);
    ctx.stroke();

    const peaks = clip.peaks && clip.peaks.length > 0 ? clip.peaks : null;
    const barStep = 3;
    const count = Math.max(1, Math.floor(w / barStep));

    ctx.fillStyle = 'rgba(103, 232, 249, 0.72)';
    for (let i = 0; i < count; i++) {
      const ratio = i / count;
      let amp = 0.4;
      if (peaks) {
        const pIdx = Math.min(peaks.length - 1, Math.floor(ratio * peaks.length));
        amp = peaks[pIdx] * clip.gain;
      } else {
        amp = (0.25 + 0.55 * Math.abs(Math.sin(i * 0.42 + clip.trimIn * 4))) * clip.gain;
      }
      const barH = Math.max(2, Math.min(h - 2, amp * (h - 2)));
      ctx.fillRect(x + i * barStep, midY - barH / 2, 2, barH);
    }
  }

  private drawVideoFilmstripInClip(
    ctx: CanvasRenderingContext2D,
    clip: ClipEvent,
    x: number,
    y: number,
    w: number,
    h: number
  ): void {
    const frameW = 54;
    const count = Math.ceil(w / frameW);
    for (let i = 0; i < count; i++) {
      const fx = x + i * frameW;
      ctx.strokeStyle = 'rgba(255,255,255,0.12)';
      ctx.strokeRect(fx, y + 2, Math.min(frameW - 2, x + w - fx), h - 4);
      ctx.fillStyle = 'rgba(255,255,255,0.18)';
      ctx.fillRect(fx + 4, y + 4, 6, 3);
      ctx.fillRect(fx + 4, y + h - 7, 6, 3);
    }
  }

  private drawCrossfadeXBox(
    ctx: CanvasRenderingContext2D,
    startSec: number,
    endSec: number,
    trackTop: number,
    trackH: number,
    outCurve: FadeCurveType,
    inCurve: FadeCurveType
  ): void {
    const x1 = this.timeToX(startSec);
    const x2 = this.timeToX(endSec);
    const w = x2 - x1;
    if (w < 3) return;
    const y = trackTop + 3;
    const h = trackH - 6;

    ctx.save();
    ctx.fillStyle = 'rgba(250, 204, 21, 0.14)';
    ctx.fillRect(x1, y, w, h);
    ctx.strokeStyle = '#facc15';
    ctx.lineWidth = 1.5;

    const steps = 14;
    ctx.beginPath();
    for (let s = 0; s <= steps; s++) {
      const r = s / steps;
      const py = y + h - evaluateFadeCurve(outCurve, 1 - r) * h;
      if (s === 0) ctx.moveTo(x1, py);
      else ctx.lineTo(x1 + r * w, py);
    }
    ctx.stroke();

    ctx.beginPath();
    for (let s = 0; s <= steps; s++) {
      const r = s / steps;
      const py = y + h - evaluateFadeCurve(inCurve, r) * h;
      if (s === 0) ctx.moveTo(x1, py);
      else ctx.lineTo(x1 + r * w, py);
    }
    ctx.stroke();

    if (w > 28) {
      ctx.fillStyle = 'rgba(15, 23, 42, 0.85)';
      ctx.fillRect(x1 + w / 2 - 18, y + h / 2 - 8, 36, 16);
      ctx.fillStyle = '#fde047';
      ctx.font = 'bold 9px monospace';
      ctx.fillText(`${(endSec - startSec).toFixed(1)}s`, x1 + w / 2 - 13, y + h / 2 + 3);
    }
    ctx.restore();
  }

  private hexToRgba(hex: string, alpha: number): string {
    const clean = hex.replace('#', '');
    if (clean.length !== 6) return `rgba(59, 130, 246, ${alpha})`;
    const r = parseInt(clean.slice(0, 2), 16);
    const g = parseInt(clean.slice(2, 4), 16);
    const b = parseInt(clean.slice(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
}
