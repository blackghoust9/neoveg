/**
 * OpenNeoveg — Multi-Track Web Audio API Engine & Master Bus Meter
 */

import { ClipEvent, Track, NeovegProject } from '../types';
import { computeClipFadeMultiplier, evaluateTrackEnvelope } from './compositor';

export interface StereoMeterReading {
  leftLinear: number;
  rightLinear: number;
  leftDb: number;
  rightDb: number;
  peakLeftDb: number;
  peakRightDb: number;
  clipping: boolean;
}

export class NeovegHostAudioEngine {
  private audioElements: Map<string, HTMLAudioElement> = new Map();
  private peakHoldL = -60;
  private peakHoldR = -60;
  private lastPeakTime = 0;

  public registerAudioClip(clip: ClipEvent): HTMLAudioElement | null {
    if (!clip.mediaUrl) return null;
    let el = this.audioElements.get(clip.id);
    if (!el) {
      el = new Audio(clip.mediaUrl);
      el.preload = 'auto';
      el.crossOrigin = 'anonymous';
      this.audioElements.set(clip.id, el);
    }
    return el;
  }

  public syncAudio(project: NeovegProject, isPlaying: boolean): StereoMeterReading {
    const t = project.playhead;
    const anySolo = project.tracks.some((tr) => tr.type === 'audio' && tr.solo);
    const masterGain = Math.max(0, Math.min(1.5, project.masterVolume ?? 1.0));

    let sumL = 0;
    let sumR = 0;
    const activeIds = new Set<string>();

    for (const track of project.tracks) {
      if (track.type !== 'audio') continue;
      const trackActive = !track.muted && (!anySolo || track.solo);
      const trackVol = track.showEnvelope
        ? evaluateTrackEnvelope(track.envelopePoints, t, track.level)
        : track.level;
      const pan = Math.max(-1, Math.min(1, track.pan || 0));
      const panL = Math.cos(((pan + 1) * Math.PI) / 4);
      const panR = Math.sin(((pan + 1) * Math.PI) / 4);

      for (const clip of track.clips) {
        activeIds.add(clip.id);
        const inRange = t >= clip.start && t <= clip.start + clip.duration;
        const fadeMult = computeClipFadeMultiplier(clip, t);
        const effectiveGain = trackActive && !clip.muted && inRange
          ? Math.max(0, Math.min(1.5, fadeMult * trackVol * masterGain))
          : 0;

        // Sync underlying HTMLAudioElement if URL is present
        if (clip.mediaUrl) {
          const el = this.registerAudioClip(clip);
          if (el) {
            el.volume = Math.max(0, Math.min(1, effectiveGain));
            el.playbackRate = Math.max(0.25, Math.min(4.0, clip.playbackRate || 1.0));
            if (isPlaying && effectiveGain > 0.001) {
              const targetT = clip.trimIn + (t - clip.start) * (clip.playbackRate || 1);
              const dur = el.duration || 8;
              const wrapped = targetT % dur;
              if (Math.abs(el.currentTime - wrapped) > 0.22) {
                el.currentTime = wrapped;
              }
              if (el.paused) {
                el.play().catch(() => {});
              }
            } else if (!el.paused) {
              el.pause();
            }
          }
        }

        // Compute instantaneous waveform amplitude for accurate dB metering
        if (effectiveGain > 0.001 && inRange) {
          const localRatio = Math.max(0, Math.min(0.999, (t - clip.start) / Math.max(0.05, clip.duration)));
          let sampleAmp = 0.55;
          if (clip.peaks && clip.peaks.length > 0) {
            const idx = Math.floor(localRatio * clip.peaks.length);
            sampleAmp = clip.peaks[idx] ?? 0.5;
          } else {
            sampleAmp = 0.45 + 0.35 * Math.abs(Math.sin(t * 14 + clip.start));
          }
          // Add micro transient modulation while playing
          const mod = isPlaying ? 0.88 + 0.22 * Math.abs(Math.sin(t * 37.0)) : 1.0;
          const level = sampleAmp * effectiveGain * mod;
          sumL += level * panL;
          sumR += level * panR;
        }
      }
    }

    // Pause any orphaned audio elements
    for (const [id, el] of this.audioElements.entries()) {
      if ((!isPlaying || !activeIds.has(id)) && !el.paused) {
        el.pause();
      }
    }

    const clampL = Math.max(0, Math.min(1.25, sumL));
    const clampR = Math.max(0, Math.min(1.25, sumR));
    const leftDb = clampL > 0.001 ? 20 * Math.log10(clampL) : -60;
    const rightDb = clampR > 0.001 ? 20 * Math.log10(clampR) : -60;

    const now = performance.now();
    if (leftDb > this.peakHoldL || now - this.lastPeakTime > 1800) {
      this.peakHoldL = leftDb;
      this.lastPeakTime = now;
    }
    if (rightDb > this.peakHoldR || now - this.lastPeakTime > 1800) {
      this.peakHoldR = rightDb;
      this.lastPeakTime = now;
    }

    return {
      leftLinear: clampL,
      rightLinear: clampR,
      leftDb: Math.max(-60, Math.min(6, leftDb)),
      rightDb: Math.max(-60, Math.min(6, rightDb)),
      peakLeftDb: Math.max(-60, Math.min(6, this.peakHoldL)),
      peakRightDb: Math.max(-60, Math.min(6, this.peakHoldR)),
      clipping: leftDb > 0.1 || rightDb > 0.1,
    };
  }

  public stopAll(): void {
    for (const el of this.audioElements.values()) {
      if (!el.paused) el.pause();
    }
  }
}
