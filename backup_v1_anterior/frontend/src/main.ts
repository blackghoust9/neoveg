/**
 * OpenVegas Pro 21 — Master Studio Application Controller (TypeScript / Qt Design System)
 * Integrates:
 * - Welcome Project Hub (Starts with Empty Project + Keeps Demo Project + Advanced Vegas Project Properties)
 * - Full Interactive VEGAS Event Pan/Crop Workspace with Keyframe Timeline & Draggable 'F' Crop Frame
 * - Video FX, Frei0r & CorridorKey Catalog with Live Human Face Portrait Demo Thumbnails
 * - LADSPA Audio DSP Plugins + Fairlight LUFS / Premiere Essential Sound Auto-Ducking
 * - DaVinci Resolve 3-Way Color Wheels, Node Graph & Premiere Pro Edit Tools (Razor, Slip, Slide, Rate Stretch)
 * - Classic Sony Vegas Themes (Vegas 4.0 XP Gray, Vegas 7.0 Silver, Vegas 13 Charcoal, Vegas 18, Vegas 21 Qt, Resolve, Premiere)
 * - VEGAS Pro Frequent Problems Diagnostic & Auto-Fixer + `log_erros.txt` Live Inspector
 */

import {
  BlendModeType,
  ClipEvent,
  EditToolMode,
  FadeCurveType,
  MediaAsset,
  TimelineDragPayload,
  Track,
  VegasProject,
  VideoFXType,
} from './types';
import { formatSMPTE, VegasCompositor } from './engine/compositor';
import { VegasAudioEngine } from './engine/audioEngine';
import { VegasTimelineController } from './ui/timeline';
import { VegasDockManager } from './ui/dockManager';
import { VEGAS_ICONS } from './ui/icons';
import { renderHumanFaceEffectThumbnail } from './ui/faceThumbnails';

interface SavedProjectMeta {
  fileName: string;
  name: string;
  isDemo: boolean;
  description: string;
  width: number;
  height: number;
  fps: number;
  pixelFormat: string;
  sampleRate: number;
  tracksCount: number;
  clipsCount: number;
  modifiedAt: string;
}

class OpenVegasStudioApp {
  public project: VegasProject;
  public mediaBin: MediaAsset[] = [];
  public savedProjects: SavedProjectMeta[] = [];
  public scriptPresets: Record<string, { name: string; description: string; code: string }> = {};
  public dockManager!: VegasDockManager;
  public activeDragPayload: TimelineDragPayload | null = null;

  private compositor!: VegasCompositor;
  private audioEngine!: VegasAudioEngine;
  private timeline!: VegasTimelineController;
  private multiMonitorBus: BroadcastChannel | null = null;

  private isPlaying = false;
  private shuttleSpeed = 1.0;
  private lastFrameTs = 0;
  private activeLeftTab: 'media' | 'transitions' | 'fx' | 'audiofx' | 'generators' = 'fx';
  private activeCenterTab: 'pancrop' | 'fxchain' | 'resolve' | 'trimmer' | 'settings' = 'pancrop';
  private showPanCropGizmo = true;

  constructor() {
    // Starts with an EMPTY project by default (Demo project is preserved in projects/ & Welcome Hub)
    this.project = this.createEmptyProjectState();
  }

  public createEmptyProjectState(custom?: Partial<VegasProject>): VegasProject {
    return {
      name: custom?.name || 'Novo_Projeto_Vazio.veg',
      isDemo: false,
      width: custom?.width || 1920,
      height: custom?.height || 1080,
      fps: custom?.fps || 29.97,
      pixelAspect: custom?.pixelAspect || 1.0,
      fieldOrder: custom?.fieldOrder || 'none',
      pixelFormat: custom?.pixelFormat || '8bit-rec709',
      colorSpace: custom?.colorSpace || 'Rec.709',
      motionBlurType: custom?.motionBlurType || 'gaussian',
      deinterlaceMethod: custom?.deinterlaceMethod || 'blend',
      resampleMode: custom?.resampleMode || 'disable',
      sampleRate: custom?.sampleRate || 48000,
      audioBitDepth: custom?.audioBitDepth || 24,
      masterBusMode: custom?.masterBusMode || 'stereo',
      bpm: custom?.bpm || 120,
      playhead: 0.0,
      loopStart: 0.0,
      loopEnd: 8.0,
      loopEnabled: false,
      snapEnabled: true,
      rippleEnabled: false,
      autoCrossfadeEnabled: true,
      previewQuality: 'best',
      splitScreenBypass: false,
      showSafeAreas: false,
      showGrid: false,
      masterVolume: 1.0,
      markers: [],
      regions: [],
      tracks: [
        {
          id: 'track_v1',
          name: 'Video Overlay / Titles',
          type: 'video',
          color: '#3daee9',
          level: 1.0,
          pan: 0,
          blendMode: 'sourceAlpha',
          muted: false,
          solo: false,
          locked: false,
          armed: false,
          showEnvelope: false,
          envelopePoints: [
            { time: 0, value: 1.0 },
            { time: 8, value: 1.0 },
          ],
          fx: [],
          clips: [],
        },
        {
          id: 'track_v2',
          name: 'Video Main Program',
          type: 'video',
          color: '#2563eb',
          level: 1.0,
          pan: 0,
          blendMode: 'sourceAlpha',
          muted: false,
          solo: false,
          locked: false,
          armed: false,
          showEnvelope: false,
          envelopePoints: [
            { time: 0, value: 1.0 },
            { time: 8, value: 1.0 },
          ],
          fx: [],
          clips: [],
        },
        {
          id: 'track_a1',
          name: 'Audio Stereo Master',
          type: 'audio',
          color: '#0891b2',
          level: 1.0,
          pan: 0,
          blendMode: 'sourceAlpha',
          muted: false,
          solo: false,
          locked: false,
          armed: false,
          showEnvelope: false,
          envelopePoints: [
            { time: 0, value: 1.0 },
            { time: 8, value: 1.0 },
          ],
          fx: [],
          clips: [],
        },
      ],
    };
  }

  public async init(): Promise<void> {
    const previewCanvas = document.getElementById('vegas-preview-canvas') as HTMLCanvasElement;
    const scopeCanvas = document.getElementById('vegas-scope-canvas') as HTMLCanvasElement;
    const timelineCanvas = document.getElementById('vegas-timeline-canvas') as HTMLCanvasElement;

    this.compositor = new VegasCompositor(previewCanvas, scopeCanvas);
    this.audioEngine = new VegasAudioEngine();
    this.timeline = new VegasTimelineController(timelineCanvas, this.project, {
      onPlayheadChange: (newTime) => {
        this.project.playhead = Math.max(0, newTime);
        this.updateTimecodeReadouts();
        this.compositor.syncVideos(this.project, this.isPlaying);
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.updateMasterMeters();
        this.broadcastMultiMonitorSync('PLAYHEAD');
      },
      onSelectClip: () => {
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      },
      onProjectMutated: (reason) => {
        this.setStatusMessage(reason);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.broadcastMultiMonitorSync('PROJECT');
      },
      onOpenClipFX: (clip) => {
        this.timeline.selectedClipId = clip.id;
        this.activeCenterTab = 'fxchain';
        this.syncCenterTabButtons();
        this.renderInspectorPanel();
      },
      onOpenClipPanCrop: (clip) => {
        this.timeline.selectedClipId = clip.id;
        this.activeCenterTab = 'pancrop';
        this.showPanCropGizmo = true;
        this.syncCenterTabButtons();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, true);
      },
      onContextMenuClip: (clip, track, x, y) => {
        this.showClipContextMenu(clip, track, x, y);
      },
      onRazorSplitAtTime: (clip, _track, splitTime) => {
        this.project.playhead = splitTime;
        this.timeline.selectedClipId = clip.id;
        this.splitAtPlayhead();
      },
      onTrackHeightChanged: () => {
        this.renderTrackHeaders();
      },
      onTimelineDrop: (dropTime, targetTrack, event) => {
        this.handleTimelineDrop(dropTime, targetTrack, event);
      },
      getActiveDragPayload: () => this.activeDragPayload,
    });

    this.dockManager = new VegasDockManager(() => {
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    this.dockManager.initDOM();

    this.initMultiMonitorSyncBus();
    this.bindToolbarAndShortcuts();
    this.renderLeftDock();
    this.renderTrackHeaders();
    this.renderInspectorPanel();
    this.timeline.recalculateAutoCrossfades();
    this.timeline.render();
    this.updateTimecodeReadouts();
    this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);

    await this.fetchBackendTelemetryAndAssets();

    // Apply initial theme or tab if passed in URL (for Multi-Monitor Popout Windows)
    const params = new URLSearchParams(window.location.search);
    const urlTheme = params.get('theme');
    if (urlTheme) {
      this.applyTheme(urlTheme);
    }
    const urlTab = params.get('tab');
    if (urlTab === 'settings') {
      this.activeCenterTab = 'settings';
      this.syncCenterTabButtons();
      this.renderInspectorPanel();
    }

    // Only open Welcome Modal on the Main Studio Window (not on Secondary Monitor Popout Windows)
    if (!this.dockManager.activePopoutPanelId) {
      this.openWelcomeModal();
    } else {
      this.multiMonitorBus?.postMessage({ type: 'REQUEST_FULL_STATE' });
    }

    requestAnimationFrame((ts) => this.studioLoop(ts));
  }

  private initMultiMonitorSyncBus(): void {
    try {
      this.multiMonitorBus = new BroadcastChannel('openvegas_studio_sync');
      this.multiMonitorBus.addEventListener('message', (ev) => {
        const msg = ev.data;
        if (!msg || typeof msg !== 'object') return;
        if (msg.type === 'REQUEST_FULL_STATE' && !this.dockManager.activePopoutPanelId) {
          this.broadcastMultiMonitorSync('PROJECT');
        } else if (msg.type === 'SYNC_PROJECT' && msg.project) {
          this.project = msg.project;
          this.timeline.setProject(this.project);
          this.renderTrackHeaders();
          this.renderInspectorPanel();
          this.updateTimecodeReadouts();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        } else if (msg.type === 'SYNC_PLAYHEAD' && typeof msg.playhead === 'number') {
          this.project.playhead = msg.playhead;
          this.updateTimecodeReadouts();
          this.timeline.render();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        } else if (msg.type === 'SYNC_DRAG_PAYLOAD') {
          this.activeDragPayload = msg.payload || null;
        }
      });
    } catch {
      // BroadcastChannel optional fallback
    }
  }

  private broadcastMultiMonitorSync(kind: 'PROJECT' | 'PLAYHEAD'): void {
    if (!this.multiMonitorBus) return;
    try {
      if (kind === 'PLAYHEAD') {
        this.multiMonitorBus.postMessage({ type: 'SYNC_PLAYHEAD', playhead: this.project.playhead });
      } else {
        this.multiMonitorBus.postMessage({ type: 'SYNC_PROJECT', project: this.project });
      }
    } catch {
      // Ignore
    }
  }

  public openSettingsTab(): void {
    document.getElementById('modal-welcome')?.classList.add('hidden');
    this.activeCenterTab = 'settings';
    this.syncCenterTabButtons();
    this.renderInspectorPanel();
    this.setStatusMessage('Aba de Configurações Avançadas do Projeto e Preferências aberta.');
  }

  private syncCenterTabButtons(): void {
    document.querySelectorAll('[data-center-tab]').forEach((b) => {
      b.classList.toggle('active', (b as HTMLElement).dataset.centerTab === this.activeCenterTab);
    });
  }

  private async fetchBackendTelemetryAndAssets(): Promise<void> {
    try {
      const [statusRes, mediaRes, scriptsRes, projRes] = await Promise.all([
        fetch('/api/status').then((r) => r.json()),
        fetch('/api/media/list').then((r) => r.json()),
        fetch('/api/scripts/presets').then((r) => r.json()),
        fetch('/api/projects/list').then((r) => r.json()),
      ]);

      if (statusRes.ok) {
        const badge = document.getElementById('engine-status-pill');
        if (badge) {
          badge.textContent = `Rust Core (${statusRes.rust_core?.version || '1.0'}) • Python ${statusRes.python_version} • FFmpeg/Frei0r/LADSPA • CorridorKey Ready`;
        }
      }

      if (mediaRes.ok && Array.isArray(mediaRes.media)) {
        this.mediaBin = mediaRes.media.map((m: MediaAsset) => ({
          ...m,
          color: m.type === 'audio' ? '#0891b2' : '#2563eb',
        }));
        this.renderLeftDock();
      }

      if (scriptsRes.ok && scriptsRes.presets) {
        this.scriptPresets = scriptsRes.presets;
      }

      if (projRes.ok && Array.isArray(projRes.projects)) {
        this.savedProjects = projRes.projects;
        this.renderWelcomeProjectsList();
      }
    } catch (err) {
      this.logErrorToBackend('STARTUP_FETCH', String(err));
    }
  }

  private async logErrorToBackend(category: string, message: string, detail = ''): Promise<void> {
    try {
      await fetch('/api/diagnostics/log-error', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ category, message, detail }),
      });
    } catch {
      // Ignore network log fallback
    }
  }

  public async loadSavedProjectByFile(fileName: string): Promise<void> {
    try {
      const res = await fetch('/api/projects/load', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ fileName }),
      }).then((r) => r.json());

      if (res.ok && res.project) {
        this.project = res.project;
        // Attach real PCM peaks to matching audio clips
        for (const asset of this.mediaBin) {
          if (asset.peaks && asset.peaks.length > 0) {
            for (const tr of this.project.tracks) {
              for (const c of tr.clips) {
                if (c.mediaFile === asset.fileName) {
                  c.peaks = asset.peaks;
                }
              }
            }
          }
        }
        const firstVideoClip = this.project.tracks
          .find((t) => t.type === 'video' && t.clips.length > 0)
          ?.clips[0];
        this.timeline.selectedClipId = firstVideoClip?.id || null;
        this.timeline.selectedTrackId = this.project.tracks[0]?.id || null;

        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.updateTimecodeReadouts();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        document.getElementById('modal-welcome')?.classList.add('hidden');
        this.setStatusMessage(`Projeto carregado: ${this.project.name} (${this.project.width}x${this.project.height} @ ${this.project.fps}fps)`);
      }
    } catch (err) {
      this.logErrorToBackend('PROJECT_LOAD', String(err));
    }
  }

  public createEmptyProjectFlow(): void {
    this.project = this.createEmptyProjectState();
    this.timeline.selectedClipId = null;
    this.timeline.setProject(this.project);
    this.renderTrackHeaders();
    this.renderInspectorPanel();
    this.updateTimecodeReadouts();
    this.compositor.renderFrame(this.project, null, false);
    document.getElementById('modal-welcome')?.classList.add('hidden');
    this.setStatusMessage('Projeto Vazio inicializado e pronto para edição.');
  }

  public loadDemoProjectFlow(): void {
    this.loadSavedProjectByFile('OpenVegas_Broadcast_Demo.veg');
  }

  public async saveCurrentProjectFlow(): Promise<void> {
    try {
      const res = await fetch('/api/projects/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project: this.project, fileName: this.project.name }),
      }).then((r) => r.json());
      if (res.ok) {
        if (Array.isArray(res.projects)) this.savedProjects = res.projects;
        this.setStatusMessage(`Projeto salvo em projects/${res.fileName}`);
      }
    } catch (err) {
      this.logErrorToBackend('PROJECT_SAVE', String(err));
    }
  }

  public openWelcomeModal(): void {
    const modal = document.getElementById('modal-welcome');
    if (!modal) return;
    document.getElementById('modal-advanced-settings')?.classList.add('hidden');
    modal.classList.remove('hidden');
    this.renderWelcomeProjectsList();

    const readSimpleSettings = () => {
      const resVal = (document.getElementById('welcome-res-preset') as HTMLSelectElement)?.value || '1920x1080';
      const [wStr, hStr] = resVal.split('x');
      const isVertical = wStr === '1080' && hStr === '1920';
      return {
        name:
          (document.getElementById('welcome-project-name') as HTMLInputElement)?.value || 'Novo_Projeto_Studio.veg',
        width: Number(wStr || 1920),
        height: Number(hStr || 1080),
        fps: isVertical ? 60 : 29.97,
        pixelAspect: 1.0,
        fieldOrder: 'none (progressive scan)',
        pixelFormat: '8-bit',
        colorSpace: 'Rec.709 Standard',
        motionBlurType: 'Gaussian',
        deinterlaceMethod: 'Blend Fields',
        resampleMode: 'Disable Resample',
        sampleRate: 48000,
        audioBitDepth: 24,
        masterBusMode: 'Stereo',
      };
    };

    // Button: Open Advanced Settings in Separate Window
    const btnOpenAdvWin = document.getElementById('btn-welcome-open-advanced-win');
    if (btnOpenAdvWin) {
      btnOpenAdvWin.onclick = () => {
        this.openAdvancedSettingsWindow();
      };
    }

    // Button: Continue with Empty Project
    const btnEmpty = document.getElementById('btn-welcome-empty-project');
    if (btnEmpty) {
      btnEmpty.onclick = () => {
        modal.classList.add('hidden');
        this.setStatusMessage(`Editando Projeto Vazio (${this.project.name}) — Arraste mídias ou geradores para a Timeline.`);
      };
    }

    // Button: Open Demo Project
    const btnDemo = document.getElementById('btn-welcome-demo-project');
    if (btnDemo) {
      btnDemo.onclick = () => this.loadSavedProjectByFile('OpenVegas_Broadcast_Demo.veg');
    }

    // Button: Create & Save New Project (.veg)
    const btnCreate = document.getElementById('btn-welcome-create-project');
    if (btnCreate) {
      btnCreate.onclick = async () => {
        const settings = readSimpleSettings();
        try {
          const res = await fetch('/api/projects/new', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ settings, saveImmediately: true }),
          }).then((r) => r.json());

          if (res.ok && res.project) {
            this.project = res.project;
            if (Array.isArray(res.projects)) this.savedProjects = res.projects;
          } else {
            this.project = this.createEmptyProjectState(settings);
          }
        } catch {
          this.project = this.createEmptyProjectState(settings);
        }

        this.timeline.selectedClipId = null;
        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.updateTimecodeReadouts();
        this.compositor.renderFrame(this.project, null, false);
        modal.classList.add('hidden');
        this.setStatusMessage(
          `Novo projeto criado: ${this.project.name} (${this.project.width}x${this.project.height} @ ${this.project.fps}fps)`
        );
      };
    }
  }

  public openAdvancedSettingsWindow(): void {
    document.getElementById('modal-welcome')?.classList.add('hidden');
    const advModal = document.getElementById('modal-advanced-settings');
    if (!advModal) return;
    advModal.classList.remove('hidden');

    const btnBack = document.getElementById('btn-adv-win-back-welcome');
    if (btnBack) {
      btnBack.onclick = () => {
        advModal.classList.add('hidden');
        this.openWelcomeModal();
      };
    }

    const btnPopoutOs = document.getElementById('btn-popout-advanced-os-win');
    if (btnPopoutOs) {
      btnPopoutOs.onclick = () => {
        advModal.classList.add('hidden');
        const currentTheme = document.documentElement.getAttribute('data-theme') || 'vegas21-dark';
        const url = new URL(window.location.href);
        url.searchParams.set('popout', 'pane-inspector');
        url.searchParams.set('tab', 'settings');
        url.searchParams.set('theme', currentTheme);
        window.open(
          url.toString(),
          'OpenVegas_AdvancedSettings_Window',
          'width=900,height=680,left=140,top=90,resizable=yes,scrollbars=yes'
        );
        this.setStatusMessage('Configurações Avançadas abertas em uma nova janela nativa independente do Windows!');
      };
    }

    const btnApply = document.getElementById('btn-adv-win-apply');
    if (btnApply) {
      btnApply.onclick = () => {
        const resVal = (document.getElementById('adv-win-res') as HTMLSelectElement)?.value || '1920x1080';
        const [wStr, hStr] = resVal.split('x');
        this.project.width = Number(wStr || 1920);
        this.project.height = Number(hStr || 1080);
        this.project.fps = Number((document.getElementById('adv-win-fps') as HTMLSelectElement)?.value || 29.97);
        this.project.pixelFormat =
          (document.getElementById('adv-win-pixel-format') as HTMLSelectElement)?.value || '8-bit';
        this.project.colorSpace =
          (document.getElementById('adv-win-color-space') as HTMLSelectElement)?.value || 'Rec.709 Standard';
        this.project.resampleMode =
          (document.getElementById('adv-win-resample') as HTMLSelectElement)?.value || 'Disable Resample';
        this.project.motionBlurType =
          (document.getElementById('adv-win-motion-blur') as HTMLSelectElement)?.value || 'Gaussian';
        this.project.sampleRate = Number(
          (document.getElementById('adv-win-sample-rate') as HTMLSelectElement)?.value || 48000
        );
        this.project.audioBitDepth = Number(
          (document.getElementById('adv-win-audio-bit') as HTMLSelectElement)?.value || 24
        );

        this.updateTimecodeReadouts();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.broadcastMultiMonitorSync('PROJECT');
        advModal.classList.add('hidden');
        this.setStatusMessage(
          `Configurações Avançadas aplicadas: ${this.project.width}x${this.project.height} @ ${this.project.fps}fps • ${this.project.colorSpace} • ${this.project.sampleRate}Hz`
        );
      };
    }
  }

  private renderWelcomeProjectsList(): void {
    const listEl = document.getElementById('welcome-projects-list');
    if (!listEl) return;

    if (this.savedProjects.length === 0) {
      listEl.innerHTML = `<div class="empty-note">Carregando projetos salvos (.veg)...</div>`;
      return;
    }

    listEl.innerHTML = this.savedProjects
      .map(
        (p) => `
      <div class="qt-project-card ${p.isDemo ? 'demo-highlight' : ''}">
        <div class="proj-card-top">
          <span class="proj-card-title">${p.name}</span>
          ${p.isDemo ? '<span class="qt-badge-demo">DEMO</span>' : '<span class="qt-badge-user">.VEG</span>'}
        </div>
        <div class="proj-card-desc">${p.description}</div>
        <div class="proj-card-meta">
          <span>${p.width}x${p.height} @ ${p.fps}fps</span>
          <span>• ${p.pixelFormat}</span>
          <span>• ${p.sampleRate}Hz</span>
        </div>
        <div class="proj-card-actions">
          <button class="qt-btn qt-btn-primary" data-open-veg="${p.fileName}">Abrir Projeto</button>
        </div>
      </div>`
      )
      .join('');

    listEl.querySelectorAll('[data-open-veg]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const fname = (btn as HTMLElement).dataset.openVeg!;
        this.loadSavedProjectByFile(fname);
      });
    });
  }

  public applyTheme(themeId: string): void {
    document.documentElement.setAttribute('data-theme', themeId);
    document.body.setAttribute('data-theme', themeId);
    const isLight = themeId === 'vegas4-xp' || themeId === 'vegas7-sony';
    this.timeline.lightThemeCanvas = isLight;
    this.timeline.render();
    this.setStatusMessage(`Tema visual clássico/NLE aplicado: ${themeId.toUpperCase()}`);
  }

  private studioLoop(ts: number): void {
    const dt = this.lastFrameTs ? Math.min(0.1, (ts - this.lastFrameTs) / 1000) : 0.016;
    this.lastFrameTs = ts;

    if (this.isPlaying) {
      this.project.playhead += dt * this.shuttleSpeed;
      const maxEnd = Math.max(this.project.loopEnd, this.getProjectDuration());
      if (this.project.loopEnabled && this.project.playhead >= this.project.loopEnd) {
        this.project.playhead = this.project.loopStart;
      } else if (this.project.playhead >= maxEnd) {
        this.project.playhead = 0;
        this.togglePlayback(false);
      } else if (this.project.playhead < 0) {
        this.project.playhead = 0;
      }

      this.updateTimecodeReadouts();
      this.compositor.syncVideos(this.project, true);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      this.timeline.render();
    }

    this.updateMasterMeters();
    requestAnimationFrame((nextTs) => this.studioLoop(nextTs));
  }

  public getProjectDuration(): number {
    let maxT = 10.0;
    for (const tr of this.project.tracks) {
      for (const c of tr.clips) {
        if (c.start + c.duration > maxT) maxT = c.start + c.duration;
      }
    }
    return maxT;
  }

  public getSelectedClip(): ClipEvent | null {
    if (!this.timeline?.selectedClipId) return null;
    for (const tr of this.project.tracks) {
      const found = tr.clips.find((c) => c.id === this.timeline.selectedClipId);
      if (found) return found;
    }
    return null;
  }

  public getSelectedTrack(): Track | null {
    if (this.timeline?.selectedTrackId) {
      const found = this.project.tracks.find((t) => t.id === this.timeline.selectedTrackId);
      if (found) return found;
    }
    return this.project.tracks[0] || null;
  }

  public togglePlayback(force?: boolean): void {
    this.isPlaying = force !== undefined ? force : !this.isPlaying;
    if (!this.isPlaying) {
      this.audioEngine.stopAll();
    }
    this.compositor.syncVideos(this.project, this.isPlaying);
    const playBtn = document.getElementById('btn-transport-play');
    if (playBtn) {
      playBtn.innerHTML = this.isPlaying
        ? `${VEGAS_ICONS.pause}<span>Pause</span>`
        : `${VEGAS_ICONS.play}<span>Play</span>`;
      playBtn.classList.toggle('active', this.isPlaying);
    }
  }

  public splitAtPlayhead(): void {
    const t = this.project.playhead;
    const selClip = this.getSelectedClip();
    let splitCount = 0;

    for (const track of this.project.tracks) {
      if (track.locked) continue;
      const newClips: ClipEvent[] = [];
      for (const clip of track.clips) {
        const shouldSplit = selClip ? clip.id === selClip.id : true;
        if (shouldSplit && !clip.locked && t > clip.start + 0.08 && t < clip.start + clip.duration - 0.08) {
          const leftDur = Number((t - clip.start).toFixed(3));
          const rightDur = Number((clip.start + clip.duration - t).toFixed(3));
          const rightClip: ClipEvent = JSON.parse(JSON.stringify(clip));
          rightClip.id = `clip_${Date.now()}_${Math.floor(Math.random() * 1000)}`;
          rightClip.start = Number(t.toFixed(3));
          rightClip.duration = rightDur;
          rightClip.trimIn = Number((clip.trimIn + leftDur * clip.playbackRate).toFixed(3));
          rightClip.fadeIn = 0.04;

          clip.duration = leftDur;
          clip.fadeOut = 0.04;

          newClips.push(clip, rightClip);
          this.timeline.selectedClipId = rightClip.id;
          splitCount++;
        } else {
          newClips.push(clip);
        }
      }
      track.clips = newClips;
    }

    if (splitCount > 0) {
      this.setStatusMessage(`Split ${splitCount} event(s) at ${formatSMPTE(t, this.project.fps)}`);
      this.timeline.render();
      this.renderInspectorPanel();
    } else {
      this.setStatusMessage('Posicione a agulha sobre um evento desbloqueado para dividir (Atalho: S)');
    }
  }

  public deleteSelectedClip(): void {
    const selId = this.timeline.selectedClipId;
    if (!selId) return;
    for (const track of this.project.tracks) {
      if (track.locked) continue;
      const idx = track.clips.findIndex((c) => c.id === selId);
      if (idx !== -1) {
        const removed = track.clips[idx];
        track.clips.splice(idx, 1);
        if (this.project.rippleEnabled) {
          for (const c of track.clips) {
            if (c.start > removed.start) {
              c.start = Math.max(0, Number((c.start - removed.duration).toFixed(3)));
            }
          }
        }
        this.timeline.selectedClipId = null;
        this.timeline.recalculateAutoCrossfades();
        this.timeline.render();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, null, false);
        this.setStatusMessage(`Evento removido: "${removed.name}"`);
        return;
      }
    }
  }

  public insertMarkerAtPlayhead(): void {
    const idx = this.project.markers.length + 1;
    const label = `Marker ${idx}`;
    this.project.markers.push({
      id: `mk_${Date.now()}`,
      time: Number(this.project.playhead.toFixed(3)),
      label,
      color: '#f97316',
    });
    this.project.markers.sort((a, b) => a.time - b.time);
    this.timeline.render();
    this.setStatusMessage(`Marker "${label}" inserido em ${formatSMPTE(this.project.playhead, this.project.fps)}`);
  }

  public insertRegionAtLoop(): void {
    const idx = this.project.regions.length + 1;
    this.project.regions.push({
      id: `rg_${Date.now()}`,
      start: this.project.loopStart,
      end: this.project.loopEnd,
      label: `Region ${idx}`,
    });
    this.timeline.render();
    this.setStatusMessage(`Region criada [${this.project.loopStart.toFixed(1)}s - ${this.project.loopEnd.toFixed(1)}s]`);
  }

  public addTrack(type: 'video' | 'audio'): void {
    const count = this.project.tracks.filter((t) => t.type === type).length + 1;
    const newTrack: Track = {
      id: `track_${type}_${Date.now()}`,
      name: type === 'video' ? `Video Track ${count}` : `Audio Track ${count}`,
      type,
      color: type === 'video' ? '#2563eb' : '#0891b2',
      level: 1.0,
      pan: 0,
      blendMode: 'sourceAlpha',
      muted: false,
      solo: false,
      locked: false,
      armed: false,
      showEnvelope: false,
      envelopePoints: [
        { time: 0, value: 1.0 },
        { time: 10, value: 1.0 },
      ],
      fx: [],
      clips: [],
    };
    if (type === 'video') {
      this.project.tracks.unshift(newTrack);
    } else {
      this.project.tracks.push(newTrack);
    }
    this.renderTrackHeaders();
    this.timeline.render();
    this.setStatusMessage(`Faixa adicionada: ${newTrack.name}`);
  }

  public setEditTool(tool: EditToolMode): void {
    this.timeline.editTool = tool;
    document.querySelectorAll('[data-edit-tool]').forEach((btn) => {
      btn.classList.toggle('active', (btn as HTMLElement).dataset.editTool === tool);
    });
    this.setStatusMessage(`Ferramenta de Edição ativa: ${tool.toUpperCase()}`);
  }

  private bindToolbarAndShortcuts(): void {
    // Welcome Hub, Save Project, Advanced Settings Tab & Theme Selector
    document.getElementById('btn-open-welcome')?.addEventListener('click', () => this.openWelcomeModal());
    document.getElementById('btn-save-project')?.addEventListener('click', () => this.saveCurrentProjectFlow());
    document.getElementById('btn-open-settings-tab')?.addEventListener('click', () => this.openSettingsTab());
    document.getElementById('select-ui-theme')?.addEventListener('change', (e) => {
      this.applyTheme((e.target as HTMLSelectElement).value);
    });

    // Edit Tools (Normal, Razor, Slip, Rate Stretch)
    document.querySelectorAll('[data-edit-tool]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const t = (btn as HTMLElement).dataset.editTool as EditToolMode;
        this.setEditTool(t);
      });
    });

    // Transport buttons
    document.getElementById('btn-transport-play')?.addEventListener('click', () => this.togglePlayback());
    document.getElementById('btn-transport-stop')?.addEventListener('click', () => {
      this.togglePlayback(false);
      this.project.playhead = this.project.loopEnabled ? this.project.loopStart : 0;
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-start')?.addEventListener('click', () => {
      this.project.playhead = 0;
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-end')?.addEventListener('click', () => {
      this.project.playhead = this.getProjectDuration();
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-prev-frame')?.addEventListener('click', () => {
      this.project.playhead = Math.max(0, this.project.playhead - 1 / this.project.fps);
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-next-frame')?.addEventListener('click', () => {
      this.project.playhead += 1 / this.project.fps;
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-loop')?.addEventListener('click', (e) => {
      this.project.loopEnabled = !this.project.loopEnabled;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.loopEnabled);
      this.timeline.render();
    });

    // Main Toolbar buttons
    document.getElementById('btn-tool-split')?.addEventListener('click', () => this.splitAtPlayhead());
    document.getElementById('btn-tool-marker')?.addEventListener('click', () => this.insertMarkerAtPlayhead());
    document.getElementById('btn-tool-region')?.addEventListener('click', () => this.insertRegionAtLoop());
    document.getElementById('btn-tool-delete')?.addEventListener('click', () => this.deleteSelectedClip());

    document.getElementById('btn-tool-snap')?.addEventListener('click', (e) => {
      this.project.snapEnabled = !this.project.snapEnabled;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.snapEnabled);
      this.setStatusMessage(`Snap (F8): ${this.project.snapEnabled ? 'ON' : 'OFF'}`);
    });

    document.getElementById('btn-tool-ripple')?.addEventListener('click', (e) => {
      this.project.rippleEnabled = !this.project.rippleEnabled;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.rippleEnabled);
      this.setStatusMessage(`Auto-Ripple (Ctrl+L): ${this.project.rippleEnabled ? 'ON' : 'OFF'}`);
    });

    document.getElementById('btn-tool-xfade')?.addEventListener('click', (e) => {
      this.project.autoCrossfadeEnabled = !this.project.autoCrossfadeEnabled;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.autoCrossfadeEnabled);
      this.timeline.recalculateAutoCrossfades();
      this.timeline.render();
    });

    document.getElementById('btn-add-video-track')?.addEventListener('click', () => this.addTrack('video'));
    document.getElementById('btn-add-audio-track')?.addEventListener('click', () => this.addTrack('audio'));
    document.getElementById('btn-tracks-shrink')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(-12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura das faixas de Áudio/Vídeo reduzida.');
    });
    document.getElementById('btn-tracks-expand')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura das faixas de Áudio/Vídeo aumentada.');
    });
    document.getElementById('btn-group-av-tracks')?.addEventListener('click', () => {
      this.groupAVTracksFlow();
    });
    document.getElementById('btn-ungroup-tracks')?.addEventListener('click', () => {
      this.ungroupTracksFlow();
    });

    // Preview Header Controls
    document.getElementById('select-preview-quality')?.addEventListener('change', (e) => {
      this.project.previewQuality = (e.target as HTMLSelectElement).value as VegasProject['previewQuality'];
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-split-bypass')?.addEventListener('click', (e) => {
      this.project.splitScreenBypass = !this.project.splitScreenBypass;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.splitScreenBypass);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-grid')?.addEventListener('click', (e) => {
      this.project.showGrid = !this.project.showGrid;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.showGrid);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-safe')?.addEventListener('click', (e) => {
      this.project.showSafeAreas = !this.project.showSafeAreas;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.project.showSafeAreas);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-gizmo')?.addEventListener('click', (e) => {
      this.showPanCropGizmo = !this.showPanCropGizmo;
      (e.currentTarget as HTMLElement).classList.toggle('active', this.showPanCropGizmo);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-snapshot')?.addEventListener('click', () => {
      const canvas = document.getElementById('vegas-preview-canvas') as HTMLCanvasElement;
      const a = document.createElement('a');
      a.href = canvas.toDataURL('image/png');
      a.download = `Vegas_Snapshot_${formatSMPTE(this.project.playhead).replace(/[:;]/g, '-')}.png`;
      a.click();
      this.setStatusMessage('Snapshot PNG salvo.');
    });

    // Left Dock Tabs
    document.querySelectorAll('[data-left-tab]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeLeftTab = (btn as HTMLElement).dataset.leftTab as typeof this.activeLeftTab;
        document.querySelectorAll('[data-left-tab]').forEach((b) => b.classList.remove('active'));
        btn.classList.add('active');
        this.renderLeftDock();
      });
    });

    // Center Dock Tabs
    document.querySelectorAll('[data-center-tab]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeCenterTab = (btn as HTMLElement).dataset.centerTab as typeof this.activeCenterTab;
        this.syncCenterTabButtons();
        this.renderInspectorPanel();
      });
    });

    // Master Fader
    const masterSlider = document.getElementById('master-volume-slider') as HTMLInputElement | null;
    masterSlider?.addEventListener('input', () => {
      this.project.masterVolume = Number(masterSlider.value);
      const dbLabel = document.getElementById('master-db-readout');
      if (dbLabel) {
        const db = this.project.masterVolume > 0.001 ? 20 * Math.log10(this.project.masterVolume) : -60;
        dbLabel.textContent = `${db >= 0 ? '+' : ''}${db.toFixed(1)} dB`;
      }
    });

    // Modals: Render As, Python Scripting, VEGAS Diagnostics & Welcome
    document.getElementById('btn-open-render-modal')?.addEventListener('click', () => this.openRenderModal());
    document.getElementById('btn-open-script-modal')?.addEventListener('click', () => this.openScriptingModal());
    document.getElementById('btn-open-diagnostics')?.addEventListener('click', () => this.openDiagnosticsModal());
    document.getElementById('btn-open-diagnostics-modal')?.addEventListener('click', () => this.openDiagnosticsModal());

    document.getElementById('btn-float-all-windows')?.addEventListener('click', () => {
      this.dockManager.floatAllPanels();
      this.setStatusMessage('Janelas soltas em modo flutuante livre — arraste pelo topo (⋮⋮) ou até a borda para abrir em outro monitor.');
    });
    document.getElementById('btn-popout-preview-monitor')?.addEventListener('click', () => {
      this.dockManager.popoutPanelToMonitor('pane-preview');
      this.setStatusMessage('Janela Video Preview destacada em janela OS nativa para segundo monitor!');
    });
    document.getElementById('btn-reset-layout')?.addEventListener('click', () => {
      this.dockManager.resetDefaultLayout();
      this.setStatusMessage('Layout acoplado padrão restaurado.');
    });

    document.querySelectorAll('[data-close-modal]').forEach((btn) => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.vegas-modal-backdrop').forEach((m) => m.classList.add('hidden'));
      });
    });

    // Import local media file
    const fileInput = document.getElementById('input-import-media') as HTMLInputElement | null;
    document.getElementById('btn-import-media')?.addEventListener('click', () => fileInput?.click());
    fileInput?.addEventListener('change', () => {
      const files = fileInput.files;
      if (!files || files.length === 0) return;
      for (const file of Array.from(files)) {
        const blobUrl = URL.createObjectURL(file);
        const isAud = file.type.startsWith('audio/') || /\.(wav|mp3|ogg|flac|m4a)$/i.test(file.name);
        const newAsset: MediaAsset = {
          id: `local_${Date.now()}_${Math.floor(Math.random() * 100)}`,
          name: file.name,
          fileName: file.name,
          url: blobUrl,
          type: isAud ? 'audio' : 'video',
          duration: 6.0,
          width: isAud ? 0 : 1280,
          height: isAud ? 0 : 720,
          fps: 30,
          codec: isAud ? 'PCM/AUDIO' : 'H264/VIDEO',
          color: isAud ? '#0891b2' : '#2563eb',
        };
        this.mediaBin.unshift(newAsset);
        this.addMediaAssetToTimeline(newAsset);
      }
      this.renderLeftDock();
    });

    // Global Vegas Pro & Premiere Keyboard Shortcuts
    window.addEventListener('keydown', (e) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

      if (e.code === 'Space') {
        e.preventDefault();
        this.togglePlayback();
      } else if (e.key === 's' || e.key === 'S') {
        if (!e.ctrlKey) {
          e.preventDefault();
          this.splitAtPlayhead();
        }
      } else if (e.key === 'd' || e.key === 'D') {
        this.setEditTool('normal');
      } else if (e.key === 'c' || e.key === 'C') {
        this.setEditTool('razor');
      } else if (e.key === 'y' || e.key === 'Y') {
        this.setEditTool('slip');
      } else if (e.key === 'x' || e.key === 'X') {
        this.setEditTool('rateStretch');
      } else if (e.key === 'm' || e.key === 'M') {
        e.preventDefault();
        this.insertMarkerAtPlayhead();
      } else if (e.key === 'r' || e.key === 'R') {
        e.preventDefault();
        this.insertRegionAtLoop();
      } else if (e.key === 'q' || e.key === 'Q') {
        e.preventDefault();
        this.project.loopEnabled = !this.project.loopEnabled;
        document.getElementById('btn-transport-loop')?.classList.toggle('active', this.project.loopEnabled);
        this.timeline.render();
      } else if (e.key === 'v' || e.key === 'V') {
        e.preventDefault();
        const tr = this.getSelectedTrack();
        if (tr) {
          tr.showEnvelope = !tr.showEnvelope;
          this.renderTrackHeaders();
          this.timeline.render();
        }
      } else if (e.key === 'F8') {
        e.preventDefault();
        this.project.snapEnabled = !this.project.snapEnabled;
        document.getElementById('btn-tool-snap')?.classList.toggle('active', this.project.snapEnabled);
      } else if ((e.ctrlKey || e.metaKey) && (e.key === 'l' || e.key === 'L')) {
        e.preventDefault();
        this.project.rippleEnabled = !this.project.rippleEnabled;
        document.getElementById('btn-tool-ripple')?.classList.toggle('active', this.project.rippleEnabled);
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        this.deleteSelectedClip();
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        this.project.playhead = Math.max(0, this.project.playhead - 1 / this.project.fps);
        this.updateTimecodeReadouts();
        this.timeline.render();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        this.project.playhead += 1 / this.project.fps;
        this.updateTimecodeReadouts();
        this.timeline.render();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      }
    });

    window.addEventListener('click', () => {
      document.getElementById('clip-context-menu')?.classList.add('hidden');
    });
  }

  public addMediaAssetToTimeline(asset: MediaAsset, dropTime?: number, targetTrackOverride?: Track | null): void {
    const targetType = asset.type === 'audio' ? 'audio' : 'video';
    let targetTrack: Track | undefined = undefined;

    if (targetTrackOverride && !targetTrackOverride.locked && targetTrackOverride.type === targetType) {
      targetTrack = targetTrackOverride;
    }
    if (!targetTrack) {
      targetTrack = this.project.tracks.find(
        (t) => t.id === this.timeline.selectedTrackId && t.type === targetType && !t.locked
      );
    }
    if (!targetTrack) {
      targetTrack = this.project.tracks.find((t) => t.type === targetType && !t.locked);
    }
    if (!targetTrack) return;

    const startTime = dropTime !== undefined ? Math.max(0, Number(dropTime.toFixed(3))) : Number(this.project.playhead.toFixed(3));

    const newClip: ClipEvent = {
      id: `clip_${Date.now()}_${Math.floor(Math.random() * 999)}`,
      name: asset.name,
      mediaId: asset.id,
      mediaFile: asset.fileName,
      mediaUrl: asset.url,
      mediaType: asset.type,
      generatorType: asset.generatorType || (asset.type === 'video' ? 'smpteBars' : undefined),
      color: asset.color || (targetType === 'video' ? '#2563eb' : '#0891b2'),
      start: startTime,
      duration: asset.duration || 5.0,
      trimIn: 0,
      fadeIn: 0.25,
      fadeOut: 0.25,
      fadeInCurve: 'smooth',
      fadeOutCurve: 'smooth',
      gain: 1.0,
      playbackRate: 1.0,
      muted: false,
      locked: false,
      panCrop: {
        x: 0,
        y: 0,
        scale: 1.0,
        rotation: 0,
        flipH: false,
        flipV: false,
        lockAspect: true,
        sizeAboutCenter: true,
        keyframes: [{ time: 0, x: 0, y: 0, scale: 1.0, rotation: 0 }],
      },
      fx: [],
      peaks: asset.peaks,
    };

    targetTrack.clips.push(newClip);
    this.timeline.selectedClipId = newClip.id;
    this.timeline.selectedTrackId = targetTrack.id;
    this.timeline.recalculateAutoCrossfades();
    this.timeline.render();
    this.renderTrackHeaders();
    this.renderInspectorPanel();
    this.compositor.renderFrame(this.project, newClip, this.showPanCropGizmo);
    this.broadcastMultiMonitorSync('PROJECT');
    this.setStatusMessage(
      `Inserido "${newClip.name}" na faixa ${targetTrack.name} em ${formatSMPTE(startTime, this.project.fps)}`
    );
  }

  public handleTimelineDrop(dropTime: number, targetTrack: Track | null, event: DragEvent): void {
    // 1. Check if external OS files were dragged from Windows Explorer directly onto the Timeline
    const files = event.dataTransfer?.files;
    if (files && files.length > 0 && !this.activeDragPayload) {
      let currentOffset = dropTime;
      for (const file of Array.from(files)) {
        const blobUrl = URL.createObjectURL(file);
        const isAud = file.type.startsWith('audio/') || /\.(wav|mp3|ogg|flac|m4a)$/i.test(file.name);
        const newAsset: MediaAsset = {
          id: `local_${Date.now()}_${Math.floor(Math.random() * 1000)}`,
          name: file.name,
          fileName: file.name,
          url: blobUrl,
          type: isAud ? 'audio' : 'video',
          duration: 6.0,
          width: isAud ? 0 : 1280,
          height: isAud ? 0 : 720,
          fps: 30,
          codec: isAud ? 'PCM/AUDIO' : 'H264/VIDEO',
          color: isAud ? '#0891b2' : '#2563eb',
        };
        this.mediaBin.unshift(newAsset);
        this.addMediaAssetToTimeline(newAsset, currentOffset, targetTrack);
        currentOffset += newAsset.duration;
      }
      this.renderLeftDock();
      return;
    }

    // 2. Internal Studio Drag Payload (Media, Generators, Video FX, LADSPA Audio FX, Transitions)
    let payload: TimelineDragPayload | null = this.activeDragPayload;
    if (!payload && event.dataTransfer) {
      const raw = event.dataTransfer.getData('application/x-openvegas-drag');
      if (raw) {
        try {
          payload = JSON.parse(raw) as TimelineDragPayload;
        } catch {
          payload = null;
        }
      }
    }
    this.activeDragPayload = null;
    this.multiMonitorBus?.postMessage({ type: 'SYNC_DRAG_PAYLOAD', payload: null });
    if (!payload) return;

    if ((payload.kind === 'media' || payload.kind === 'generator') && payload.asset) {
      this.addMediaAssetToTimeline(payload.asset, dropTime, targetTrack);
      return;
    }

    // Find clip under drop coordinates (or fall back to selected clip)
    let targetClip: ClipEvent | null = null;
    if (targetTrack) {
      targetClip =
        targetTrack.clips.find((c) => dropTime >= c.start && dropTime <= c.start + c.duration) ||
        targetTrack.clips[targetTrack.clips.length - 1] ||
        null;
    }
    if (!targetClip) {
      targetClip = this.getSelectedClip();
    }

    if ((payload.kind === 'fx' || payload.kind === 'audiofx') && payload.fxType) {
      if (!targetClip) {
        // Automatically create a demo clip at dropTime if dropped onto an empty timeline!
        const fallbackAsset: MediaAsset =
          payload.fxType === 'corridorKey'
            ? {
                id: 'gen_ck_drop',
                name: 'CorridorKey Green Screen Subject',
                type: 'generator',
                generatorType: 'greenScreenActor',
                duration: 6.0,
                width: 1280,
                height: 720,
                fps: 30,
                codec: 'CORRIDORKEY',
                color: '#16a34a',
              }
            : this.mediaBin[0] || {
                id: 'gen_auto_drop',
                name: 'VEGAS Synthwave Horizon Scene',
                type: 'generator',
                generatorType: 'cyberGrid',
                duration: 5.0,
                width: 1280,
                height: 720,
                fps: 30,
                codec: 'GENERATOR',
                color: '#2563eb',
              };
        this.addMediaAssetToTimeline(fallbackAsset, dropTime, targetTrack);
        targetClip = this.getSelectedClip();
      }
      if (targetClip) {
        targetClip.fx.push({
          id: `fx_${Date.now()}`,
          type: payload.fxType,
          name: payload.fxName || payload.name,
          enabled: true,
          params: { ...(payload.fxDefaults || {}) },
        });
        this.timeline.selectedClipId = targetClip.id;
        this.activeCenterTab = 'fxchain';
        this.syncCenterTabButtons();
        this.timeline.render();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, targetClip, this.showPanCropGizmo);
        this.broadcastMultiMonitorSync('PROJECT');
        this.setStatusMessage(`Efeito "${payload.name}" solto sobre o clipe "${targetClip.name}"`);
      }
      return;
    }

    if (payload.kind === 'transition' && targetClip) {
      targetClip.fadeIn = Math.max(0.8, targetClip.fadeIn);
      targetClip.fadeOut = Math.max(0.8, targetClip.fadeOut);
      if (payload.transitionCurve) {
        targetClip.fadeInCurve = payload.transitionCurve;
        targetClip.fadeOutCurve = payload.transitionCurve;
      }
      if (payload.transitionType) {
        targetClip.transitionType = payload.transitionType;
      }
      this.timeline.selectedClipId = targetClip.id;
      this.timeline.render();
      this.renderInspectorPanel();
      this.compositor.renderFrame(this.project, targetClip, this.showPanCropGizmo);
      this.broadcastMultiMonitorSync('PROJECT');
      this.setStatusMessage(`Transição "${payload.name}" solta sobre "${targetClip.name}"`);
    }
  }

  private bindDraggableCard(cardEl: HTMLElement, payload: TimelineDragPayload): void {
    cardEl.setAttribute('draggable', 'true');
    cardEl.addEventListener('dragstart', (ev) => {
      this.activeDragPayload = payload;
      this.multiMonitorBus?.postMessage({ type: 'SYNC_DRAG_PAYLOAD', payload });
      cardEl.classList.add('is-dragging-card');
      if (ev.dataTransfer) {
        ev.dataTransfer.effectAllowed = 'copy';
        ev.dataTransfer.setData('application/x-openvegas-drag', JSON.stringify(payload));
        ev.dataTransfer.setData('text/plain', payload.name);
      }
    });
    cardEl.addEventListener('dragend', () => {
      cardEl.classList.remove('is-dragging-card');
      this.activeDragPayload = null;
      this.multiMonitorBus?.postMessage({ type: 'SYNC_DRAG_PAYLOAD', payload: null });
    });
  }

  public renderLeftDock(): void {
    const container = document.getElementById('left-dock-body');
    if (!container) return;

    if (this.activeLeftTab === 'media') {
      const cards = this.mediaBin
        .map(
          (m) => `
        <div class="media-card" draggable="true" data-media-id="${m.id}" title="Arraste diretamente para qualquer faixa na Timeline ou clique em + Track">
          <div class="media-badge ${m.type}">${m.type === 'audio' ? 'WAV' : 'MP4'}</div>
          <div class="media-info">
            <div class="media-title">${m.name}</div>
            <div class="media-meta">
              ${m.type === 'audio' ? `${m.sampleRate || 48000}Hz • Peak ${m.peakDb ?? -1.2} dB` : `${m.width}x${m.height} • ${m.fps}fps`}
              • ${m.duration.toFixed(1)}s • <span class="drag-hint-pill">⋮⋮ Arraste p/ Timeline</span>
            </div>
          </div>
          <button class="qt-btn qt-btn-primary" data-insert-media="${m.id}">+ Track</button>
        </div>`
        )
        .join('');

      container.innerHTML = `
        <div class="dock-section-header">
          <span>Project Media Bin (${this.mediaBin.length}) — Arraste para a Timeline</span>
          <button class="qt-btn" id="btn-quick-load-demo">Carregar Demo</button>
        </div>
        <div class="media-grid">${cards || '<div class="empty-note">Carregando mídias...</div>'}</div>
      `;

      document.getElementById('btn-quick-load-demo')?.addEventListener('click', () => {
        this.loadSavedProjectByFile('OpenVegas_Broadcast_Demo.veg');
      });

      container.querySelectorAll('.media-card[data-media-id]').forEach((card) => {
        const id = (card as HTMLElement).dataset.mediaId;
        const asset = this.mediaBin.find((x) => x.id === id);
        if (asset) {
          this.bindDraggableCard(card as HTMLElement, {
            kind: 'media',
            id: asset.id,
            name: asset.name,
            mediaType: asset.type,
            duration: asset.duration || 5.0,
            color: asset.color || (asset.type === 'audio' ? '#0891b2' : '#2563eb'),
            asset,
          });
        }
      });

      container.querySelectorAll('[data-insert-media]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const id = (btn as HTMLElement).dataset.insertMedia;
          const asset = this.mediaBin.find((x) => x.id === id);
          if (asset) this.addMediaAssetToTimeline(asset);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'transitions') {
      const transitions: Array<{
        name: string;
        curve: FadeCurveType;
        trans: ClipEvent['transitionType'];
        desc: string;
      }> = [
        { name: 'VEGAS Crossfade (Smooth S-Curve)', curve: 'smooth', trans: 'crossfade', desc: 'Dissolução clássica igual-potência em S' },
        { name: 'VEGAS Crossfade (Fast Attack)', curve: 'fast', trans: 'crossfade', desc: 'Curva logarítmica de ataque rápido' },
        { name: 'VEGAS Crossfade (Slow Exponential)', curve: 'slow', trans: 'crossfade', desc: 'Curva exponencial progressiva' },
        { name: 'VEGAS Linear Wipe Left-to-Right', curve: 'linear', trans: 'wipeLeft', desc: 'Cortina horizontal broadcast' },
        { name: 'VEGAS Iris Circle Reveal', curve: 'smooth', trans: 'irisCircle', desc: 'Máscara circular expansiva' },
        { name: 'VEGAS Flash White Cut', curve: 'fast', trans: 'flashWhite', desc: 'Flash branco de exposição' },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>VEGAS Transitions & 5 Fade Curves (Arraste sobre um Clipe)</span>
        </div>
        <div class="media-grid">
          ${transitions
            .map(
              (t, idx) => `
            <div class="media-card" draggable="true" data-trans-card="${idx}" title="Arraste para cima de um clipe na Timeline">
              <div class="media-badge trans">X-FADE</div>
              <div class="media-info">
                <div class="media-title">${t.name}</div>
                <div class="media-meta">${t.desc} • <span class="drag-hint-pill">⋮⋮ Arraste</span></div>
              </div>
              <button class="qt-btn qt-btn-primary" data-apply-trans="${idx}">Aplicar</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      container.querySelectorAll('[data-trans-card]').forEach((card) => {
        const idx = Number((card as HTMLElement).dataset.transCard);
        const item = transitions[idx];
        this.bindDraggableCard(card as HTMLElement, {
          kind: 'transition',
          id: `trans_${idx}`,
          name: item.name,
          mediaType: 'video',
          duration: 1.5,
          color: '#f59e0b',
          transitionCurve: item.curve,
          transitionType: item.trans,
        });
      });

      container.querySelectorAll('[data-apply-trans]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn as HTMLElement).dataset.applyTrans);
          const item = transitions[idx];
          const clip = this.getSelectedClip();
          if (!clip) {
            this.setStatusMessage('Selecione um evento na timeline primeiro.');
            return;
          }
          clip.fadeIn = Math.max(0.8, clip.fadeIn);
          clip.fadeOut = Math.max(0.8, clip.fadeOut);
          clip.fadeInCurve = item.curve;
          clip.fadeOutCurve = item.curve;
          clip.transitionType = item.trans;
          this.timeline.render();
          this.renderInspectorPanel();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'fx') {
      // Video FX + Frei0r Plugins + CorridorKey AI with Human Face Portrait Demo Thumbnails!
      const fxCatalog: Array<{
        type: VideoFXType;
        name: string;
        badge: string;
        desc: string;
        defaults: Record<string, number>;
      }> = [
        {
          type: 'corridorKey',
          name: 'CorridorKey AI Green Screen Unmixer',
          badge: 'CORRIDORKEY',
          desc: 'nikopueringer/CorridorKey: Desmixagem de cor & alpha linear sem spill verde',
          defaults: { similarity: 0.28, despill: 0.9, blend: 0.08 },
        },
        {
          type: 'colorGrade',
          name: 'VEGAS / Resolve 3-Wheel Color Grade',
          badge: 'COLOR',
          desc: 'Lift / Gamma / Gain Teal & Orange cinematográfico',
          defaults: { liftR: -0.05, liftB: 0.08, gainR: 1.14, gainB: 0.92, saturation: 1.22 },
        },
        {
          type: 'brightnessContrast',
          name: 'VEGAS Brightness & Contrast',
          badge: 'VEGAS FX',
          desc: 'Exposição, contraste e saturação broadcast',
          defaults: { brightness: 0.08, contrast: 1.2, saturation: 1.15 },
        },
        {
          type: 'frei0r.glow',
          name: 'Frei0r Glow (Optical Bloom)',
          badge: 'FREI0R',
          desc: 'Plugin Frei0r de difusão luminosa em altas-luzes',
          defaults: { blur: 0.45, intensity: 0.65 },
        },
        {
          type: 'frei0r.cartoon',
          name: 'Frei0r Cartoon (Cel-Shaded)',
          badge: 'FREI0R',
          desc: 'Plugin Frei0r de quantização tonal e contorno',
          defaults: { edge: 0.7, quantize: 0.5 },
        },
        {
          type: 'frei0r.pixeliz0r',
          name: 'Frei0r Pixeliz0r (Mosaic)',
          badge: 'FREI0R',
          desc: 'Plugin Frei0r de mosaico pixelado',
          defaults: { blockSize: 8.0 },
        },
        {
          type: 'frei0r.sobel',
          name: 'Frei0r Sobel (Edge Contour)',
          badge: 'FREI0R',
          desc: 'Detector espacial de bordas Sobel 2D',
          defaults: { threshold: 0.5 },
        },
        {
          type: 'frei0r.coloradj_RGB',
          name: 'Frei0r RGB Channel Balance',
          badge: 'FREI0R',
          desc: 'Multiplicador R/G/B de calibração de câmera',
          defaults: { r: 1.15, g: 1.0, b: 0.88 },
        },
        {
          type: 'vignette',
          name: 'VEGAS Soft Lens Vignette',
          badge: 'VEGAS FX',
          desc: 'Escurecimento óptico radial de bordas',
          defaults: { strength: 0.65 },
        },
        {
          type: 'blur',
          name: 'VEGAS Gaussian Blur',
          badge: 'VEGAS FX',
          desc: 'Desfoque gaussiano de profundidade de campo',
          defaults: { radius: 4.5 },
        },
        {
          type: 'vhsGlitch',
          name: 'VEGAS TV Simulator / VHS',
          badge: 'VEGAS FX',
          desc: 'Scanlines analógicas e desvio cromático',
          defaults: { intensity: 0.6 },
        },
        {
          type: 'sepia',
          name: 'VEGAS Archival Sepia Tone',
          badge: 'VEGAS FX',
          desc: 'Viragem sépia fotográfica clássica',
          defaults: { amount: 0.85 },
        },
        {
          type: 'invert',
          name: 'VEGAS Negative Invert',
          badge: 'VEGAS FX',
          desc: 'Inversão RGB completa',
          defaults: { amount: 1.0 },
        },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>Video FX • Frei0r • CorridorKey (Demo Rosto Humano — Arraste p/ Clipe)</span>
        </div>
        <div class="media-grid">
          ${fxCatalog
            .map(
              (fx, idx) => `
            <div class="media-card fx-thumb-card" draggable="true" data-fx-card="${idx}" title="Arraste este efeito diretamente para um clipe na Timeline">
              <canvas class="fx-face-thumb" width="68" height="50" data-face-fx="${fx.type}"></canvas>
              <div class="media-info">
                <div class="fx-card-title-row">
                  <span class="qt-tag-mini">${fx.badge}</span>
                  <span class="media-title">${fx.name}</span>
                </div>
                <div class="media-meta">${fx.desc}</div>
              </div>
              <button class="qt-btn qt-btn-primary" data-add-fx="${idx}">+ FX</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      // Render human face portrait demonstration on every thumbnail canvas!
      container.querySelectorAll('canvas[data-face-fx]').forEach((canv) => {
        const fxType = (canv as HTMLElement).dataset.faceFx || '';
        renderHumanFaceEffectThumbnail(canv as HTMLCanvasElement, fxType);
      });

      container.querySelectorAll('[data-fx-card]').forEach((card) => {
        const idx = Number((card as HTMLElement).dataset.fxCard);
        const item = fxCatalog[idx];
        this.bindDraggableCard(card as HTMLElement, {
          kind: 'fx',
          id: `fx_${item.type}`,
          name: item.name,
          mediaType: 'video',
          duration: 5.0,
          color: '#10b981',
          fxType: item.type,
          fxName: item.name,
          fxDefaults: item.defaults,
        });
      });

      container.querySelectorAll('[data-add-fx]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn as HTMLElement).dataset.addFx);
          const item = fxCatalog[idx];
          let clip = this.getSelectedClip();
          if (!clip) {
            const genAsset: MediaAsset =
              item.type === 'corridorKey'
                ? {
                    id: 'gen_ck_actor',
                    name: 'CorridorKey Green Screen Actor',
                    type: 'generator',
                    generatorType: 'greenScreenActor',
                    duration: 6.0,
                    width: 1280,
                    height: 720,
                    fps: 29.97,
                    codec: 'GENERATOR',
                    color: '#16a34a',
                  }
                : this.mediaBin[0] || {
                    id: 'gen_cyber_auto',
                    name: 'Cyber Horizon Scene',
                    type: 'generator',
                    generatorType: 'cyberGrid',
                    duration: 5.0,
                    width: 1280,
                    height: 720,
                    fps: 29.97,
                    codec: 'GENERATOR',
                    color: '#2563eb',
                  };
            this.addMediaAssetToTimeline(genAsset);
            clip = this.getSelectedClip();
          }
          if (!clip) return;
          clip.fx.push({
            id: `fx_${Date.now()}`,
            type: item.type,
            name: item.name,
            enabled: true,
            params: { ...item.defaults },
          });
          this.activeCenterTab = 'fxchain';
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
          this.setStatusMessage(`Efeito "${item.name}" adicionado a ${clip.name}`);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'audiofx') {
      const ladspaList: Array<{
        type: VideoFXType;
        name: string;
        desc: string;
        defaults: Record<string, number>;
      }> = [
        {
          type: 'ladspa.sc4_1882',
          name: 'LADSPA SC4 Stereo Compressor (1882)',
          desc: 'Compressor dinâmico RMS/Peak para voz e master',
          defaults: { thresholdDb: -16, ratio: 4.0, makeupDb: 3.0 },
        },
        {
          type: 'ladspa.fast_lookahead_limiter_1913',
          name: 'LADSPA Fast Lookahead Limiter (1913)',
          desc: 'Limiter brickwall True-Peak (-1.0 dBTP)',
          defaults: { limitDb: -1.0, inputGainDb: 2.5 },
        },
        {
          type: 'ladspa.mbeq_1197',
          name: 'LADSPA Multiband Equalizer (1197)',
          desc: 'Equalizador harmônico de 15 bandas',
          defaults: { lowBoostDb: 3.0, highAirDb: 2.5 },
        },
        {
          type: 'ladspa.plate_1423',
          name: 'LADSPA Studio Plate Reverb (1423)',
          desc: 'Reverberação acústica de placa metálica',
          defaults: { reverbTime: 1.8, wetMix: 0.25 },
        },
        {
          type: 'ladspa.gate_1410',
          name: 'LADSPA Noise Gate (1410)',
          desc: 'Corte automático de ruído de fundo entre falas',
          defaults: { thresholdDb: -38.0 },
        },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>LADSPA Audio DSP • Fairlight LUFS (Arraste p/ Clipe de Áudio)</span>
        </div>
        <div class="media-grid">
          ${ladspaList
            .map(
              (af, idx) => `
            <div class="media-card" draggable="true" data-ladspa-card="${idx}" title="Arraste este plugin LADSPA para um clipe de áudio na Timeline">
              <div class="media-badge audio">LADSPA</div>
              <div class="media-info">
                <div class="media-title">${af.name}</div>
                <div class="media-meta">${af.desc} • <span class="drag-hint-pill">⋮⋮ Arraste</span></div>
              </div>
              <button class="qt-btn qt-btn-primary" data-add-ladspa="${idx}">+ Audio FX</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      container.querySelectorAll('[data-ladspa-card]').forEach((card) => {
        const idx = Number((card as HTMLElement).dataset.ladspaCard);
        const item = ladspaList[idx];
        this.bindDraggableCard(card as HTMLElement, {
          kind: 'audiofx',
          id: `ladspa_${item.type}`,
          name: item.name,
          mediaType: 'audio',
          duration: 5.0,
          color: '#0891b2',
          fxType: item.type,
          fxName: item.name,
          fxDefaults: item.defaults,
        });
      });

      container.querySelectorAll('[data-add-ladspa]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn as HTMLElement).dataset.addLadspa);
          const item = ladspaList[idx];
          const clip = this.getSelectedClip();
          if (!clip) {
            this.setStatusMessage('Selecione um evento de áudio ou vídeo na timeline.');
            return;
          }
          clip.fx.push({
            id: `ladspa_${Date.now()}`,
            type: item.type,
            name: item.name,
            enabled: true,
            params: { ...item.defaults },
          });
          this.activeCenterTab = 'fxchain';
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
          this.setStatusMessage(`Plugin LADSPA "${item.name}" inserido em ${clip.name}`);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'generators') {
      const gens: MediaAsset[] = [
        {
          id: 'gen_ck_greenscreen',
          name: 'CorridorKey Green Screen Subject (Ator em Fundo Verde)',
          type: 'generator',
          generatorType: 'greenScreenActor',
          duration: 6.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'CORRIDORKEY',
          color: '#16a34a',
        },
        {
          id: 'gen_title_card',
          name: 'VEGAS Titles & Text Overlay',
          type: 'title',
          generatorType: 'titleText',
          duration: 4.5,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#6366f1',
        },
        {
          id: 'gen_cyber_scene',
          name: 'VEGAS Synthwave Horizon Grid',
          type: 'generator',
          generatorType: 'cyberGrid',
          duration: 5.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#ec4899',
        },
        {
          id: 'gen_credits_roll',
          name: 'VEGAS Scrolling Credit Roll',
          type: 'generator',
          generatorType: 'creditRoll',
          duration: 6.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#0284c7',
        },
        {
          id: 'gen_smpte_bars',
          name: 'VEGAS SMPTE HD Color Bars',
          type: 'generator',
          generatorType: 'smpteBars',
          duration: 4.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#2563eb',
        },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>VEGAS Media Generators (Arraste diretamente para a Timeline)</span>
        </div>
        <div class="media-grid">
          ${gens
            .map(
              (g, idx) => `
            <div class="media-card" draggable="true" data-gen-card="${idx}" title="Arraste este Gerador diretamente para qualquer faixa na Timeline">
              <div class="media-badge gen">GEN</div>
              <div class="media-info">
                <div class="media-title">${g.name}</div>
                <div class="media-meta">Gerador Procedural 60fps • ${g.duration}s • <span class="drag-hint-pill">⋮⋮ Arraste p/ Timeline</span></div>
              </div>
              <button class="qt-btn qt-btn-primary" data-insert-gen="${idx}">+ Inserir</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      container.querySelectorAll('[data-gen-card]').forEach((card) => {
        const idx = Number((card as HTMLElement).dataset.genCard);
        const genAsset = gens[idx];
        this.bindDraggableCard(card as HTMLElement, {
          kind: 'generator',
          id: genAsset.id,
          name: genAsset.name,
          mediaType: 'generator',
          duration: genAsset.duration,
          color: genAsset.color,
          asset: genAsset,
        });
      });

      container.querySelectorAll('[data-insert-gen]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn as HTMLElement).dataset.insertGen);
          this.addMediaAssetToTimeline(gens[idx]);
        });
      });
    }
  }

  public async renameTrackById(trackId: string, newName: string): Promise<void> {
    const clean = newName.trim().slice(0, 64) || 'Untitled Track';
    const tr = this.project.tracks.find((t) => t.id === trackId);
    if (!tr) return;
    tr.name = clean;
    this.timeline.render();
    this.broadcastMultiMonitorSync('PROJECT');
    this.setStatusMessage(`Faixa renomeada para: "${clean}"`);

    try {
      const res = await fetch('/api/tracks/rename', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project: this.project, trackId, newName: clean }),
      }).then((r) => r.json());
      if (res.ok && res.project) {
        this.project = res.project;
        this.timeline.setProject(this.project);
      }
    } catch {
      // Fallback already applied locally
    }
  }

  public async groupAVTracksFlow(specificTrackIds?: string[], customGroupName?: string): Promise<void> {
    const palette = ['#f59e0b', '#10b981', '#a855f7', '#ec4899', '#38bdf8'];
    let targetIds = specificTrackIds || [];

    if (targetIds.length === 0) {
      // Pair ungrouped video + audio tracks, or group all tracks if <= 2
      const ungroupedVideo = this.project.tracks.find((t) => t.type === 'video' && !t.groupId);
      const ungroupedAudio = this.project.tracks.find((t) => t.type === 'audio' && !t.groupId);
      if (ungroupedVideo && ungroupedAudio) {
        targetIds = [ungroupedVideo.id, ungroupedAudio.id];
      } else {
        targetIds = this.project.tracks.map((t) => t.id);
      }
    }

    const existingGroups = new Set(this.project.tracks.map((t) => t.groupId).filter(Boolean));
    const groupIdx = existingGroups.size + 1;
    const groupId = `grp_av_${groupIdx}`;
    const groupName = customGroupName || `Grupo V+A ${groupIdx}`;
    const groupColor = palette[(groupIdx - 1) % palette.length];

    for (const tr of this.project.tracks) {
      if (targetIds.includes(tr.id)) {
        tr.groupId = groupId;
        tr.groupName = groupName;
        tr.groupColor = groupColor;
      }
    }

    this.renderTrackHeaders();
    this.timeline.render();
    this.broadcastMultiMonitorSync('PROJECT');
    this.setStatusMessage(`Faixas de Vídeo e Áudio agrupadas em "${groupName}" (Mute/Solo sincronizados).`);

    try {
      const res = await fetch('/api/tracks/group', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'group',
          project: this.project,
          trackIds: targetIds,
          groupId,
          groupName,
          groupColor,
        }),
      }).then((r) => r.json());
      if (res.ok && res.project) {
        this.project = res.project;
        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
      }
    } catch {
      // Local state already updated
    }
  }

  public async ungroupTracksFlow(trackId?: string): Promise<void> {
    const targetId = trackId || this.timeline.selectedTrackId || undefined;
    const targetTrack = targetId ? this.project.tracks.find((t) => t.id === targetId) : undefined;

    if (targetTrack && targetTrack.groupId) {
      const gid = targetTrack.groupId;
      for (const tr of this.project.tracks) {
        if (tr.groupId === gid) {
          delete tr.groupId;
          delete tr.groupName;
          delete tr.groupColor;
        }
      }
      this.setStatusMessage(`Grupo de faixas desfeito para "${targetTrack.name}".`);
    } else {
      for (const tr of this.project.tracks) {
        delete tr.groupId;
        delete tr.groupName;
        delete tr.groupColor;
      }
      this.setStatusMessage('Todas as faixas foram desagrupadas.');
    }

    this.renderTrackHeaders();
    this.timeline.render();
    this.broadcastMultiMonitorSync('PROJECT');

    try {
      await fetch('/api/tracks/group', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'ungroup',
          project: this.project,
          trackId: targetTrack?.id,
        }),
      });
    } catch {
      // Local state already updated
    }
  }

  public renderTrackHeaders(): void {
    const container = document.getElementById('vegas-track-headers');
    if (!container) return;

    container.innerHTML = this.project.tracks
      .map((track, idx) => {
        const isSelected = this.timeline?.selectedTrackId === track.id;
        const levelPct = Math.round(track.level * 100);
        const th = this.timeline ? this.timeline.getTrackHeight(track) : track.height || 74;
        const hasGroup = Boolean(track.groupId);
        const grpColor = track.groupColor || '#f59e0b';
        const grpBorderStyle = hasGroup ? `border-left: 4px solid ${grpColor};` : '';
        return `
        <div class="vegas-track-header ${track.type} ${isSelected ? 'selected' : ''} ${hasGroup ? 'grouped-track' : ''}" data-track-id="${track.id}" style="height:${th}px; ${grpBorderStyle}">
          <div class="track-hdr-top">
            <span class="track-num-badge" style="background:${track.color}" title="${track.type === 'video' ? 'Faixa de Vídeo' : 'Faixa de Áudio'}">${idx + 1}</span>
            <input class="track-name-input" data-track-name="${track.id}" value="${track.name}" title="Clique para renomear esta faixa de ${track.type === 'video' ? 'Vídeo' : 'Áudio'} (Enter para confirmar)" />
            <button class="track-btn" data-track-rename="${track.id}" title="Renomear Faixa (${track.name})">${VEGAS_ICONS.renameEdit}</button>
            <button class="track-btn ${hasGroup ? 'active' : ''}" data-track-group="${track.id}" style="${hasGroup ? `border-color:${grpColor};color:${grpColor};` : ''}" title="${hasGroup ? `Agrupada em: ${track.groupName} (Clique para desagrupar)` : 'Agrupar esta faixa com Vídeo+Áudio (G)'}">G</button>
            <button class="track-btn track-h-btn" data-track-h-dec="${track.id}" title="Diminuir altura desta faixa (-)">${VEGAS_ICONS.minus}</button>
            <button class="track-btn track-h-btn" data-track-h-inc="${track.id}" title="Aumentar altura desta faixa (+)">${VEGAS_ICONS.plus}</button>
            <button class="track-btn ${track.showEnvelope ? 'active' : ''}" data-track-env="${track.id}" title="Automation Envelope (V)">V</button>
            <button class="track-btn mute ${track.muted ? 'active' : ''}" data-track-mute="${track.id}" title="Mute Track (Sincronizado se agrupada)">M</button>
            <button class="track-btn solo ${track.solo ? 'active' : ''}" data-track-solo="${track.id}" title="Solo Track (Sincronizado se agrupada)">S</button>
          </div>
          <div class="track-hdr-bottom">
            ${
              hasGroup
                ? `<span class="track-group-pill" style="border-color:${grpColor};color:${grpColor}" title="Grupo de Faixas Ativo">${track.groupName || 'V+A'}</span>`
                : ''
            }
            <label class="track-slider-lbl">
              <span>${track.type === 'video' ? 'Opac' : 'Vol'}</span>
              <input type="range" min="0" max="${track.type === 'video' ? '1' : '1.4'}" step="0.02" value="${track.level}" data-track-level="${track.id}" />
              <span class="track-val">${levelPct}%</span>
            </label>
            ${
              track.type === 'video'
                ? `<select class="track-blend-select" data-track-blend="${track.id}" title="Vegas Compositing Mode">
                    <option value="sourceAlpha" ${track.blendMode === 'sourceAlpha' ? 'selected' : ''}>Src Alpha</option>
                    <option value="add" ${track.blendMode === 'add' ? 'selected' : ''}>Add</option>
                    <option value="multiply" ${track.blendMode === 'multiply' ? 'selected' : ''}>Multiply</option>
                    <option value="screen" ${track.blendMode === 'screen' ? 'selected' : ''}>Screen</option>
                    <option value="overlay" ${track.blendMode === 'overlay' ? 'selected' : ''}>Overlay</option>
                    <option value="difference" ${track.blendMode === 'difference' ? 'selected' : ''}>Difference</option>
                  </select>`
                : `<label class="track-slider-lbl pan-lbl">
                    <span>Pan</span>
                    <input type="range" min="-1" max="1" step="0.05" value="${track.pan}" data-track-pan="${track.id}" />
                  </label>`
            }
          </div>
          <div class="track-resize-grip" data-track-resize="${track.id}" title="Arraste a borda inferior para aumentar ou diminuir a altura desta faixa (${th}px)"></div>
        </div>
      `;
      })
      .join('');

    container.querySelectorAll('.vegas-track-header').forEach((row) => {
      row.addEventListener('click', (e) => {
        const target = e.target as HTMLElement;
        if (target && (target.tagName === 'INPUT' || target.tagName === 'SELECT' || target.closest('button'))) {
          return;
        }
        this.timeline.selectedTrackId = (row as HTMLElement).dataset.trackId || null;
        this.renderTrackHeaders();
      });
    });

    container.querySelectorAll('[data-track-name]').forEach((inp) => {
      const inputEl = inp as HTMLInputElement;
      inputEl.addEventListener('click', (e) => e.stopPropagation());
      inputEl.addEventListener('mousedown', (e) => e.stopPropagation());
      inputEl.addEventListener('dblclick', (e) => {
        e.stopPropagation();
        inputEl.select();
      });
      inputEl.addEventListener('keydown', (e) => {
        e.stopPropagation();
        if (e.key === 'Enter') {
          inputEl.blur();
        }
      });
      inputEl.addEventListener('change', (e) => {
        e.stopPropagation();
        const tid = inputEl.dataset.trackName;
        if (tid) {
          this.renameTrackById(tid, inputEl.value);
        }
      });
    });

    container.querySelectorAll('[data-track-rename]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackRename!;
        const inp = container.querySelector(`.track-name-input[data-track-name="${tid}"]`) as HTMLInputElement | null;
        if (inp) {
          inp.focus();
          inp.select();
          this.setStatusMessage(`Digite o novo nome para a faixa "${inp.value}" e pressione Enter.`);
        }
      });
    });

    container.querySelectorAll('[data-track-group]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackGroup!;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (!tr) return;
        if (tr.groupId) {
          this.ungroupTracksFlow(tid);
        } else {
          // Pair this track with a companion track of opposite type (or adjacent track)
          const companion =
            this.project.tracks.find((t) => t.id !== tid && t.type !== tr.type && !t.groupId) ||
            this.project.tracks.find((t) => t.id !== tid);
          const ids = companion ? [tid, companion.id] : [tid];
          this.groupAVTracksFlow(ids);
        }
      });
    });

    container.querySelectorAll('[data-track-h-dec]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackHDec!;
        this.timeline.adjustSingleTrackHeight(tid, -14);
        this.renderTrackHeaders();
      });
    });

    container.querySelectorAll('[data-track-h-inc]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackHInc!;
        this.timeline.adjustSingleTrackHeight(tid, 14);
        this.renderTrackHeaders();
      });
    });

    container.querySelectorAll('[data-track-resize]').forEach((grip) => {
      grip.addEventListener('mousedown', (e) => {
        e.stopPropagation();
        e.preventDefault();
        const tid = (grip as HTMLElement).dataset.trackResize!;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (!tr) return;
        const startY = (e as MouseEvent).clientY;
        const startH = this.timeline.getTrackHeight(tr);
        const onMove = (ev: MouseEvent) => {
          const dy = ev.clientY - startY;
          tr.height = Math.max(46, Math.min(220, Math.round(startH + dy)));
          const hdrEl = container.querySelector(`.vegas-track-header[data-track-id="${tid}"]`) as HTMLElement | null;
          if (hdrEl) hdrEl.style.height = `${tr.height}px`;
          this.timeline.render();
        };
        const onUp = () => {
          window.removeEventListener('mousemove', onMove);
          window.removeEventListener('mouseup', onUp);
          this.renderTrackHeaders();
        };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
      });
    });

    container.querySelectorAll('[data-track-mute]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackMute;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          const nextMuted = !tr.muted;
          tr.muted = nextMuted;
          if (tr.groupId) {
            for (const member of this.project.tracks) {
              if (member.groupId === tr.groupId) member.muted = nextMuted;
            }
            fetch('/api/tracks/group', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                action: 'sync',
                project: this.project,
                groupId: tr.groupId,
                muted: nextMuted,
              }),
            }).catch(() => {});
          }
          this.renderTrackHeaders();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-solo]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackSolo;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          const nextSolo = !tr.solo;
          tr.solo = nextSolo;
          if (tr.groupId) {
            for (const member of this.project.tracks) {
              if (member.groupId === tr.groupId) member.solo = nextSolo;
            }
            fetch('/api/tracks/group', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                action: 'sync',
                project: this.project,
                groupId: tr.groupId,
                solo: nextSolo,
              }),
            }).catch(() => {});
          }
          this.renderTrackHeaders();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-env]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn as HTMLElement).dataset.trackEnv;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          tr.showEnvelope = !tr.showEnvelope;
          this.renderTrackHeaders();
          this.timeline.render();
        }
      });
    });

    container.querySelectorAll('[data-track-level]').forEach((rng) => {
      rng.addEventListener('input', (e) => {
        e.stopPropagation();
        const tid = (rng as HTMLElement).dataset.trackLevel;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          tr.level = Number((rng as HTMLInputElement).value);
          const span = rng.parentElement?.querySelector('.track-val');
          if (span) span.textContent = `${Math.round(tr.level * 100)}%`;
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-blend]').forEach((sel) => {
      sel.addEventListener('change', (e) => {
        e.stopPropagation();
        const tid = (sel as HTMLElement).dataset.trackBlend;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          tr.blendMode = (sel as HTMLSelectElement).value as BlendModeType;
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-pan]').forEach((rng) => {
      rng.addEventListener('input', (e) => {
        e.stopPropagation();
        const tid = (rng as HTMLElement).dataset.trackPan;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) tr.pan = Number((rng as HTMLInputElement).value);
      });
    });
  }

  private renderSettingsTab(container: HTMLElement): void {
    const curRes = `${this.project.width}x${this.project.height}`;
    container.innerHTML = `
      <div class="settings-tab-grid">
        <div class="insp-header">
          <strong>Configurações Avançadas do Projeto & Preferências do Estúdio (Qt / VEGAS Pro 21)</strong>
          <button class="qt-btn qt-btn-primary" id="btn-settings-apply-all">Aplicar e Salvar Configurações</button>
        </div>

        <!-- 1. VIDEO, COLOR SCIENCE & ACEScct 32-BIT FLOAT -->
        <div class="settings-group-box">
          <div class="settings-group-title">1. Vídeo, Ciência de Cor & Pipeline ACEScct 32-Bit Floating Point</div>
          <div class="insp-row-2col">
            <div>
              <label>Nome do Projeto (.veg)</label>
              <input type="text" id="cfg-proj-name" class="vegas-input" value="${this.project.name}" />
            </div>
            <div>
              <label>Resolução Master</label>
              <select id="cfg-res" class="vegas-select">
                <option value="1920x1080" ${curRes === '1920x1080' ? 'selected' : ''}>1920 x 1080 (Full HD 1080p)</option>
                <option value="3840x2160" ${curRes === '3840x2160' ? 'selected' : ''}>3840 x 2160 (4K UHD 2160p)</option>
                <option value="2560x1440" ${curRes === '2560x1440' ? 'selected' : ''}>2560 x 1440 (2K QHD)</option>
                <option value="1280x720" ${curRes === '1280x720' ? 'selected' : ''}>1280 x 720 (HD 720p)</option>
                <option value="1080x1920" ${curRes === '1080x1920' ? 'selected' : ''}>1080 x 1920 (Vertical 9:16)</option>
              </select>
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>Frame Rate (FPS)</label>
              <select id="cfg-fps" class="vegas-select">
                <option value="23.976" ${this.project.fps === 23.976 ? 'selected' : ''}>23.976 fps (IVTC Film)</option>
                <option value="24" ${this.project.fps === 24 ? 'selected' : ''}>24.000 fps (DCI Cinema)</option>
                <option value="25" ${this.project.fps === 25 ? 'selected' : ''}>25.000 fps (PAL Broadcast)</option>
                <option value="29.97" ${this.project.fps === 29.97 ? 'selected' : ''}>29.970 fps (NTSC SMPTE)</option>
                <option value="30" ${this.project.fps === 30 ? 'selected' : ''}>30.000 fps (Web HD)</option>
                <option value="59.94" ${this.project.fps === 59.94 ? 'selected' : ''}>59.940 fps (Double NTSC)</option>
                <option value="60" ${this.project.fps === 60 ? 'selected' : ''}>60.000 fps (High Frame Rate)</option>
              </select>
            </div>
            <div>
              <label>Pixel Format & Bit Depth</label>
              <select id="cfg-pixfmt" class="vegas-select">
                <option value="8bit_full" ${this.project.pixelFormat.includes('8bit') ? 'selected' : ''}>8-bit Unsigned Integer (Full Range)</option>
                <option value="32bit_float_video" ${this.project.pixelFormat === '32bit_float_video' ? 'selected' : ''}>32-bit Floating Point (Video Levels)</option>
                <option value="32bit_float_aces" ${this.project.pixelFormat === '32bit_float_aces' ? 'selected' : ''}>32-bit Floating Point (Full Linear ACES 1.3)</option>
              </select>
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>View Transform / Color Space</label>
              <select id="cfg-colorspace" class="vegas-select">
                <option value="Rec.709" ${this.project.colorSpace === 'Rec.709' ? 'selected' : ''}>Rec.709 Standard sRGB</option>
                <option value="ACEScct" ${this.project.colorSpace === 'ACEScct' ? 'selected' : ''}>ACEScct Academy Color Encoding</option>
                <option value="Rec.2020_HDR10" ${this.project.colorSpace === 'Rec.2020_HDR10' ? 'selected' : ''}>Rec.2020 ST.2084 PQ (HDR10)</option>
                <option value="DaVinci_WG" ${this.project.colorSpace === 'DaVinci_WG' ? 'selected' : ''}>DaVinci Wide Gamut / Intermediate</option>
              </select>
            </div>
            <div>
              <label>Motion Blur & Resample Mode</label>
              <select id="cfg-resample" class="vegas-select">
                <option value="disable" ${this.project.resampleMode === 'disable' ? 'selected' : ''}>Disable Resample (Sem Ghosting)</option>
                <option value="optical_flow" ${this.project.resampleMode === 'optical_flow' ? 'selected' : ''}>Optical Flow Warp Interpolation</option>
                <option value="smart_resample" ${this.project.resampleMode === 'smart_resample' ? 'selected' : ''}>Smart Resample (VEGAS Classic)</option>
              </select>
            </div>
          </div>
        </div>

        <!-- 2. AUDIO ENGINE, ASIO / WASAPI & MASTER BUS -->
        <div class="settings-group-box">
          <div class="settings-group-title">2. Motor de Áudio, Driver ASIO/WASAPI, LADSPA & Fairlight Loudness</div>
          <div class="insp-row-2col">
            <div>
              <label>Sample Rate (Hz)</label>
              <select id="cfg-aud-rate" class="vegas-select">
                <option value="44100" ${this.project.sampleRate === 44100 ? 'selected' : ''}>44,100 Hz (CD Audio)</option>
                <option value="48000" ${this.project.sampleRate === 48000 ? 'selected' : ''}>48,000 Hz (Broadcast DVD/TV)</option>
                <option value="96000" ${this.project.sampleRate === 96000 ? 'selected' : ''}>96,000 Hz (Studio Hi-Res)</option>
                <option value="192000" ${this.project.sampleRate === 192000 ? 'selected' : ''}>192,000 Hz (Ultra Mastering)</option>
              </select>
            </div>
            <div>
              <label>Bit Depth & Master Bus</label>
              <select id="cfg-aud-bus" class="vegas-select">
                <option value="stereo" ${this.project.masterBusMode === 'stereo' ? 'selected' : ''}>Stereo 2.0 Master Bus (${this.project.audioBitDepth}-bit)</option>
                <option value="surround_51" ${this.project.masterBusMode === 'surround_51' ? 'selected' : ''}>5.1 Surround Cinema (${this.project.audioBitDepth}-bit)</option>
                <option value="dolby_71" ${this.project.masterBusMode === 'dolby_71' ? 'selected' : ''}>7.1 Dolby Atmos Bed (32-bit Float)</option>
              </select>
            </div>
          </div>
          <div class="insp-row-2col">
            <div>
              <label>Audio Device Driver (Windows)</label>
              <select id="cfg-aud-driver" class="vegas-select">
                <option value="wasapi_shared">Windows WASAPI Low-Latency (Shared/Exclusive)</option>
                <option value="asio_studio">ASIO 2.3 Studio Hardware Driver (64 samples)</option>
                <option value="directsound">Microsoft DirectSound Mapper</option>
              </select>
            </div>
            <div>
              <label>Alvo de Normalização LUFS</label>
              <select id="cfg-aud-lufs" class="vegas-select">
                <option value="-14">-14.0 LUFS (YouTube / Spotify / Streaming)</option>
                <option value="-23">-23.0 LUFS (EBU R128 Broadcast TV)</option>
                <option value="-24">-24.0 LUFS (ATSC A/85 Cinema/US TV)</option>
              </select>
            </div>
          </div>
        </div>

        <!-- 3. MULTI-MONITOR WORKSPACE & HARDWARE / CORRIDORKEY AI -->
        <div class="settings-group-box">
          <div class="settings-group-title">3. Multi-Monitor (Arrastar para Outros Monitores) & GPU / CorridorKey AI</div>
          <div class="settings-multimonitor-bar">
            <button class="qt-btn qt-btn-primary" data-popout-panel="pane-preview">${VEGAS_ICONS.popoutMonitor}<span>Destacar Preview p/ Outro Monitor</span></button>
            <button class="qt-btn" data-popout-panel="pane-media">${VEGAS_ICONS.popoutMonitor}<span>Destacar Media/FX p/ Monitor 2</span></button>
            <button class="qt-btn" data-popout-panel="pane-mixer">${VEGAS_ICONS.popoutMonitor}<span>Destacar Mixer p/ Monitor 2</span></button>
            <button class="qt-btn" data-popout-panel="pane-timeline">${VEGAS_ICONS.popoutMonitor}<span>Destacar Timeline p/ Monitor 2</span></button>
          </div>
          <div class="insp-row-2col" style="margin-top:6px">
            <div>
              <label>Aceleração de Hardware & Decoder</label>
              <select id="cfg-gpu-mode" class="vegas-select">
                <option value="safe_nvenc">Hardware NVENC/QSV + Safe Mode (so4compoundplug Fix Ativo)</option>
                <option value="rust_simd">Rust Core SIMD + FFmpeg Software Pipeline</option>
                <option value="corridorkey_cuda">CorridorKey ViT-L Greenscreen Neural Engine (FP16)</option>
              </select>
            </div>
            <div>
              <label>Altura Global das Faixas (Áudio/Vídeo)</label>
              <div style="display:flex;gap:4px">
                <button class="qt-btn" id="btn-cfg-track-shrink" style="flex:1">${VEGAS_ICONS.minus}<span>Diminuir Faixas</span></button>
                <button class="qt-btn" id="btn-cfg-track-expand" style="flex:1">${VEGAS_ICONS.plus}<span>Aumentar Faixas</span></button>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    container.querySelectorAll('[data-popout-panel]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const pid = (btn as HTMLElement).dataset.popoutPanel!;
        this.dockManager.popoutPanelToMonitor(pid);
        this.setStatusMessage(`Painel ${pid} aberto em janela OS nativa — arraste livremente para outro monitor!`);
      });
    });

    document.getElementById('btn-cfg-track-shrink')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(-12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura de todas as faixas de Áudio/Vídeo reduzida.');
    });

    document.getElementById('btn-cfg-track-expand')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura de todas as faixas de Áudio/Vídeo aumentada.');
    });

    document.getElementById('btn-settings-apply-all')?.addEventListener('click', () => {
      const nameVal = (document.getElementById('cfg-proj-name') as HTMLInputElement)?.value || this.project.name;
      const resVal = (document.getElementById('cfg-res') as HTMLSelectElement)?.value || '1920x1080';
      const [wStr, hStr] = resVal.split('x');
      this.project.name = nameVal;
      this.project.width = Number(wStr || 1920);
      this.project.height = Number(hStr || 1080);
      this.project.fps = Number((document.getElementById('cfg-fps') as HTMLSelectElement)?.value || 29.97);
      this.project.pixelFormat = (document.getElementById('cfg-pixfmt') as HTMLSelectElement)?.value || '8bit_full';
      this.project.colorSpace = (document.getElementById('cfg-colorspace') as HTMLSelectElement)?.value || 'Rec.709';
      this.project.resampleMode = (document.getElementById('cfg-resample') as HTMLSelectElement)?.value || 'disable';
      this.project.sampleRate = Number((document.getElementById('cfg-aud-rate') as HTMLSelectElement)?.value || 48000);
      this.project.masterBusMode = (document.getElementById('cfg-aud-bus') as HTMLSelectElement)?.value || 'stereo';

      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      this.broadcastMultiMonitorSync('PROJECT');
      this.saveCurrentProjectFlow();
      this.setStatusMessage(
        `Configurações Avançadas salvas: ${this.project.width}x${this.project.height} @ ${this.project.fps}fps • ${this.project.colorSpace} • ${this.project.sampleRate}Hz`
      );
    });
  }

  public renderInspectorPanel(): void {
    const container = document.getElementById('center-dock-body');
    if (!container) return;

    if (this.activeCenterTab === 'settings') {
      this.renderSettingsTab(container);
      return;
    }

    const clip = this.getSelectedClip();
    if (!clip) {
      container.innerHTML = `
        <div class="empty-note">
          <div>Projeto ativo: <strong>${this.project.name}</strong> (${this.project.width}x${this.project.height} @ ${this.project.fps}fps)</div>
          <div style="margin-top:6px">Arraste qualquer mídia, gerador ou efeito diretamente para a Timeline abaixo, ou escolha uma opção:</div>
          <div style="margin-top:10px;display:flex;gap:6px;justify-content:center;flex-wrap:wrap">
            <button class="qt-btn qt-btn-primary" id="btn-empty-load-demo">Carregar Projeto de Demonstração (.veg)</button>
            <button class="qt-btn" id="btn-empty-open-settings">Aba de Configurações Avançadas</button>
            <button class="qt-btn" id="btn-empty-open-welcome">Janela de Boas-Vindas</button>
          </div>
        </div>
      `;
      document.getElementById('btn-empty-load-demo')?.addEventListener('click', () => {
        this.loadSavedProjectByFile('OpenVegas_Broadcast_Demo.veg');
      });
      document.getElementById('btn-empty-open-settings')?.addEventListener('click', () => {
        this.openSettingsTab();
      });
      document.getElementById('btn-empty-open-welcome')?.addEventListener('click', () => {
        this.openWelcomeModal();
      });
      return;
    }

    if (this.activeCenterTab === 'pancrop') {
      const pc = clip.panCrop;
      if (!pc.keyframes || pc.keyframes.length === 0) {
        pc.keyframes = [{ time: 0, x: pc.x, y: pc.y, scale: pc.scale, rotation: pc.rotation }];
      }

      container.innerHTML = `
        <div class="inspector-grid">
          <div class="insp-header">
            <strong>VEGAS Event Pan/Crop: ${clip.name}</strong>
            <span class="insp-sub">Pos: ${(pc.x * 100).toFixed(0)}, ${(pc.y * 100).toFixed(0)} | Zoom: ${Math.round((1 / Math.max(0.2, pc.scale)) * 100)}%</span>
          </div>

          <!-- INTERACTIVE VEGAS PAN/CROP WORKSPACE CANVAS -->
          <div class="pancrop-canvas-wrap">
            <canvas id="pancrop-interactive-canvas" width="280" height="145" title="Arraste o quadro 'F' para mover, o canto para escalar ou o anel externo para girar"></canvas>
            <div class="pancrop-side-tools">
              <button class="qt-btn" id="btn-pc-reset" title="Restaurar Quadro 100%">Reset 100%</button>
              <button class="qt-btn" id="btn-pc-169" title="Predefinição 16:9 Widescreen">16:9 Wide</button>
              <button class="qt-btn" id="btn-pc-pip-tr" title="Picture-in-Picture Canto Superior">PiP Sup-Dir</button>
              <button class="qt-btn" id="btn-pc-pip-bl" title="Picture-in-Picture Canto Inferior">PiP Inf-Esq</button>
              <button class="qt-btn ${pc.flipH ? 'active' : ''}" id="btn-pc-fliph">Espelhar H</button>
              <button class="qt-btn ${pc.flipV ? 'active' : ''}" id="btn-pc-flipv">Espelhar V</button>
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>Center X (${pc.x.toFixed(2)})</label>
              <input type="range" min="-0.8" max="0.8" step="0.02" value="${pc.x}" id="inp-pc-x" />
            </div>
            <div>
              <label>Center Y (${pc.y.toFixed(2)})</label>
              <input type="range" min="-0.8" max="0.8" step="0.02" value="${pc.y}" id="inp-pc-y" />
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>Frame Size / Zoom (${Math.round(pc.scale * 100)}%)</label>
              <input type="range" min="0.25" max="2.5" step="0.05" value="${pc.scale}" id="inp-pc-scale" />
            </div>
            <div>
              <label>Rotation Angle (${pc.rotation}°)</label>
              <input type="range" min="-180" max="180" step="1" value="${pc.rotation}" id="inp-pc-rot" />
            </div>
          </div>

          <!-- VEGAS PAN/CROP KEYFRAME CONTROLLER -->
          <div class="pancrop-kf-bar">
            <div class="kf-hdr">
              <span>Keyframes Pan/Crop (${pc.keyframes.length})</span>
              <div style="display:flex;gap:4px">
                <button class="qt-btn qt-btn-primary" id="btn-pc-add-kf">+ Keyframe no Cursor</button>
                <button class="qt-btn" id="btn-pc-clear-kf">Limpar</button>
              </div>
            </div>
            <div class="kf-track-strip" id="pancrop-kf-strip">
              ${pc.keyframes
                .map((kf, idx) => {
                  const leftPct = Math.max(2, Math.min(96, (kf.time / Math.max(0.2, clip.duration)) * 100));
                  return `<div class="kf-diamond" style="left:${leftPct}%" data-kf-idx="${idx}" title="Keyframe ${idx + 1} @ +${kf.time.toFixed(2)}s">◆</div>`;
                })
                .join('')}
            </div>
          </div>
        </div>
      `;

      this.initInteractivePanCropCanvas(clip);

      const syncActiveKeyframe = () => {
        if (clip.panCrop.keyframes && clip.panCrop.keyframes.length > 0) {
          const localT = Math.max(0, Math.min(clip.duration, this.project.playhead - clip.start));
          const nearest = clip.panCrop.keyframes.find((k) => Math.abs(k.time - localT) < 0.25);
          if (nearest) {
            nearest.x = clip.panCrop.x;
            nearest.y = clip.panCrop.y;
            nearest.scale = clip.panCrop.scale;
            nearest.rotation = clip.panCrop.rotation;
          } else if (clip.panCrop.keyframes.length === 1) {
            clip.panCrop.keyframes[0].x = clip.panCrop.x;
            clip.panCrop.keyframes[0].y = clip.panCrop.y;
            clip.panCrop.keyframes[0].scale = clip.panCrop.scale;
            clip.panCrop.keyframes[0].rotation = clip.panCrop.rotation;
          }
        }
      };

      const bindRange = (id: string, fn: (val: number) => void) => {
        const el = document.getElementById(id) as HTMLInputElement | null;
        el?.addEventListener('input', () => {
          fn(Number(el.value));
          syncActiveKeyframe();
          this.drawPanCropInteractiveCanvas(clip);
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
        el?.addEventListener('change', () => this.renderInspectorPanel());
      };

      bindRange('inp-pc-x', (v) => (clip.panCrop.x = v));
      bindRange('inp-pc-y', (v) => (clip.panCrop.y = v));
      bindRange('inp-pc-scale', (v) => (clip.panCrop.scale = v));
      bindRange('inp-pc-rot', (v) => (clip.panCrop.rotation = v));

      document.getElementById('btn-pc-reset')?.addEventListener('click', () => {
        clip.panCrop = {
          x: 0,
          y: 0,
          scale: 1,
          rotation: 0,
          flipH: false,
          flipV: false,
          keyframes: [{ time: 0, x: 0, y: 0, scale: 1, rotation: 0 }],
        };
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-169')?.addEventListener('click', () => {
        clip.panCrop.scale = 0.78;
        clip.panCrop.x = 0;
        clip.panCrop.y = 0;
        syncActiveKeyframe();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-pip-tr')?.addEventListener('click', () => {
        clip.panCrop.x = -0.35;
        clip.panCrop.y = 0.32;
        clip.panCrop.scale = 1.65;
        syncActiveKeyframe();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-pip-bl')?.addEventListener('click', () => {
        clip.panCrop.x = 0.35;
        clip.panCrop.y = -0.32;
        clip.panCrop.scale = 1.65;
        syncActiveKeyframe();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-fliph')?.addEventListener('click', () => {
        clip.panCrop.flipH = !clip.panCrop.flipH;
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-flipv')?.addEventListener('click', () => {
        clip.panCrop.flipV = !clip.panCrop.flipV;
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });

      document.getElementById('btn-pc-add-kf')?.addEventListener('click', () => {
        const localT = Number(Math.max(0, Math.min(clip.duration, this.project.playhead - clip.start)).toFixed(2));
        clip.panCrop.keyframes = clip.panCrop.keyframes || [];
        clip.panCrop.keyframes.push({
          time: localT,
          x: clip.panCrop.x,
          y: clip.panCrop.y,
          scale: clip.panCrop.scale,
          rotation: clip.panCrop.rotation,
        });
        clip.panCrop.keyframes.sort((a, b) => a.time - b.time);
        this.renderInspectorPanel();
        this.setStatusMessage(`Keyframe Pan/Crop adicionado em +${localT}s`);
      });

      document.getElementById('btn-pc-clear-kf')?.addEventListener('click', () => {
        clip.panCrop.keyframes = [
          { time: 0, x: clip.panCrop.x, y: clip.panCrop.y, scale: clip.panCrop.scale, rotation: clip.panCrop.rotation },
        ];
        this.renderInspectorPanel();
      });
      return;
    }

    if (this.activeCenterTab === 'resolve') {
      // DaVinci Resolve 3-Way Color Wheels + Node Graph + Premiere Lumetri / Text-Based Cut
      container.innerHTML = `
        <div class="inspector-grid">
          <div class="insp-header">
            <strong>Resolve Color Wheels • Node Graph • Premiere Lumetri</strong>
            <button class="qt-btn qt-btn-primary" id="btn-apply-resolve-grade">Aplicar Grade ao Clipe</button>
          </div>

          <div class="color-wheels-row">
            <div class="wheel-box">
              <span>LIFT (Shadows)</span>
              <canvas class="resolve-wheel-canvas" width="72" height="72" data-wheel="lift"></canvas>
            </div>
            <div class="wheel-box">
              <span>GAMMA (Midtones)</span>
              <canvas class="resolve-wheel-canvas" width="72" height="72" data-wheel="gamma"></canvas>
            </div>
            <div class="wheel-box">
              <span>GAIN (Highlights)</span>
              <canvas class="resolve-wheel-canvas" width="72" height="72" data-wheel="gain"></canvas>
            </div>
          </div>

          <!-- FUSION / RESOLVE NODE GRAPH PREVIEW -->
          <div class="node-graph-box">
            <div class="fg-label">DaVinci Resolve / Fusion Node Pipeline:</div>
            <canvas id="resolve-node-canvas" width="350" height="58"></canvas>
          </div>

          <!-- PREMIERE PRO TEXT-BASED TRANSCRIPT EDITING -->
          <div class="transcript-box">
            <div class="fg-label">Premiere Pro Text-Based Transcript Editing (Clique na frase para posicionar/cortar):</div>
            <div class="transcript-tokens">
              <span class="tr-word" data-seek="0.2">[00:00] "Bem-vindos ao OpenVegas</span>
              <span class="tr-word" data-seek="1.5">Pro 21 Studio</span>
              <span class="tr-word" data-seek="2.8">com motor Rust,</span>
              <span class="tr-word" data-seek="4.2">Frei0r, LADSPA e CorridorKey!"</span>
            </div>
          </div>
        </div>
      `;

      this.drawResolveWheelsAndNodes();

      container.querySelectorAll('.tr-word').forEach((w) => {
        w.addEventListener('click', () => {
          const t = Number((w as HTMLElement).dataset.seek || 0);
          this.project.playhead = clip.start + t;
          this.updateTimecodeReadouts();
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });

      document.getElementById('btn-apply-resolve-grade')?.addEventListener('click', () => {
        clip.fx.push({
          id: `fx_resolve_${Date.now()}`,
          type: 'colorGrade',
          name: 'DaVinci Resolve 3-Wheel Cinema Grade',
          enabled: true,
          params: { liftR: -0.06, liftB: 0.09, gainR: 1.15, gainB: 0.92, saturation: 1.24 },
        });
        this.timeline.render();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        this.setStatusMessage(`DaVinci Resolve Cinema Grade aplicado em ${clip.name}`);
      });
      return;
    }

    if (this.activeCenterTab === 'fxchain') {
      const fxItems = clip.fx || [];
      container.innerHTML = `
        <div class="inspector-grid">
          <div class="insp-header">
            <strong>Event FX Chain: ${clip.name} (${fxItems.length})</strong>
            <div style="display:flex;gap:4px">
              <button class="qt-btn qt-btn-primary" id="btn-fx-quick-ck">+ CorridorKey</button>
              <button class="qt-btn" id="btn-fx-quick-grade">+ Color Grade</button>
            </div>
          </div>
          ${
            fxItems.length === 0
              ? `<div class="empty-note">Nenhum efeito ativo neste evento. Use a aba "Video FX" à esquerda ou clique nos botões acima.</div>`
              : fxItems
                  .map(
                    (fx, idx) => `
                  <div class="fx-chain-card">
                    <div class="fx-chain-hdr">
                      <label style="display:flex;align-items:center;gap:6px">
                        <input type="checkbox" data-fx-toggle="${idx}" ${fx.enabled ? 'checked' : ''} />
                        <strong>${fx.name}</strong>
                      </label>
                      <button class="qt-btn" data-fx-del="${idx}">Remover</button>
                    </div>
                    <div class="fx-params-row">
                      ${Object.entries(fx.params)
                        .map(
                          ([k, v]) => `
                        <label class="fx-param-lbl">
                          <span>${k}: ${Number(v).toFixed(2)}</span>
                          <input type="range" min="-0.5" max="2.5" step="0.04" value="${v}" data-fx-idx="${idx}" data-fx-key="${k}" />
                        </label>`
                        )
                        .join('')}
                    </div>
                  </div>`
                  )
                  .join('')
          }
        </div>
      `;

      document.getElementById('btn-fx-quick-ck')?.addEventListener('click', () => {
        clip.fx.push({
          id: `fx_ck_${Date.now()}`,
          type: 'corridorKey',
          name: 'CorridorKey AI Green Screen Unmixer',
          enabled: true,
          params: { similarity: 0.28, despill: 0.9, blend: 0.08 },
        });
        this.renderInspectorPanel();
        this.timeline.render();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });

      document.getElementById('btn-fx-quick-grade')?.addEventListener('click', () => {
        clip.fx.push({
          id: `fx_${Date.now()}`,
          type: 'colorGrade',
          name: 'VEGAS Color Corrector (Teal/Orange)',
          enabled: true,
          params: { liftR: -0.05, liftB: 0.08, gainR: 1.14, gainB: 0.92, saturation: 1.2 },
        });
        this.renderInspectorPanel();
        this.timeline.render();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });

      container.querySelectorAll('[data-fx-toggle]').forEach((chk) => {
        chk.addEventListener('change', () => {
          const idx = Number((chk as HTMLElement).dataset.fxToggle);
          clip.fx[idx].enabled = (chk as HTMLInputElement).checked;
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });

      container.querySelectorAll('[data-fx-del]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn as HTMLElement).dataset.fxDel);
          clip.fx.splice(idx, 1);
          this.renderInspectorPanel();
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });

      container.querySelectorAll('[data-fx-key]').forEach((rng) => {
        rng.addEventListener('input', () => {
          const idx = Number((rng as HTMLElement).dataset.fxIdx);
          const key = (rng as HTMLElement).dataset.fxKey!;
          const val = Number((rng as HTMLInputElement).value);
          clip.fx[idx].params[key] = val;
          const span = rng.parentElement?.querySelector('span');
          if (span) span.textContent = `${key}: ${val.toFixed(2)}`;
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });
      return;
    }

    // Trimmer & Velocity Inspector
    container.innerHTML = `
      <div class="inspector-grid">
        <div class="insp-header">
          <strong>VEGAS Event Trimmer & Velocity: ${clip.name}</strong>
        </div>
        ${
          clip.generatorType === 'titleText'
            ? `
          <div class="insp-row">
            <label>Texto Principal do Título</label>
            <input type="text" class="vegas-input" id="inp-clip-title" value="${clip.titleText || ''}" />
          </div>
          <div class="insp-row">
            <label>Subtítulo</label>
            <input type="text" class="vegas-input" id="inp-clip-sub" value="${clip.subtitleText || ''}" />
          </div>`
            : ''
        }
        <div class="insp-row-2col">
          <div>
            <label>Início na Timeline (s)</label>
            <input type="number" step="0.1" class="vegas-input" id="inp-trim-start" value="${clip.start.toFixed(2)}" />
          </div>
          <div>
            <label>Duração (s)</label>
            <input type="number" step="0.1" class="vegas-input" id="inp-trim-dur" value="${clip.duration.toFixed(2)}" />
          </div>
        </div>
        <div class="insp-row-2col">
          <div>
            <label>Velocity / Playback Rate (${Math.round(clip.playbackRate * 100)}%)</label>
            <input type="range" min="0.25" max="3.0" step="0.05" value="${clip.playbackRate}" id="inp-trim-rate" />
          </div>
          <div>
            <label>Ganho / Opacidade (${Math.round(clip.gain * 100)}%)</label>
            <input type="range" min="0" max="1.5" step="0.05" value="${clip.gain}" id="inp-trim-gain" />
          </div>
        </div>
        <div class="insp-row-2col">
          <div>
            <label>Curva Fade-In (${clip.fadeIn.toFixed(2)}s)</label>
            <select class="vegas-select" id="sel-fadein-curve">
              <option value="smooth" ${clip.fadeInCurve === 'smooth' ? 'selected' : ''}>Smooth (S-Curve)</option>
              <option value="fast" ${clip.fadeInCurve === 'fast' ? 'selected' : ''}>Fast (Logarithmic)</option>
              <option value="slow" ${clip.fadeInCurve === 'slow' ? 'selected' : ''}>Slow (Exponential)</option>
              <option value="linear" ${clip.fadeInCurve === 'linear' ? 'selected' : ''}>Linear</option>
              <option value="sharp" ${clip.fadeInCurve === 'sharp' ? 'selected' : ''}>Sharp</option>
            </select>
          </div>
          <div>
            <label>Curva Fade-Out (${clip.fadeOut.toFixed(2)}s)</label>
            <select class="vegas-select" id="sel-fadeout-curve">
              <option value="smooth" ${clip.fadeOutCurve === 'smooth' ? 'selected' : ''}>Smooth (S-Curve)</option>
              <option value="fast" ${clip.fadeOutCurve === 'fast' ? 'selected' : ''}>Fast (Logarithmic)</option>
              <option value="slow" ${clip.fadeOutCurve === 'slow' ? 'selected' : ''}>Slow (Exponential)</option>
              <option value="linear" ${clip.fadeOutCurve === 'linear' ? 'selected' : ''}>Linear</option>
              <option value="sharp" ${clip.fadeOutCurve === 'sharp' ? 'selected' : ''}>Sharp</option>
            </select>
          </div>
        </div>
      </div>
    `;

    document.getElementById('inp-clip-title')?.addEventListener('input', (e) => {
      clip.titleText = (e.target as HTMLInputElement).value;
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-clip-sub')?.addEventListener('input', (e) => {
      clip.subtitleText = (e.target as HTMLInputElement).value;
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-trim-start')?.addEventListener('change', (e) => {
      clip.start = Math.max(0, Number((e.target as HTMLInputElement).value));
      this.timeline.recalculateAutoCrossfades();
      this.timeline.render();
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-trim-dur')?.addEventListener('change', (e) => {
      clip.duration = Math.max(0.2, Number((e.target as HTMLInputElement).value));
      this.timeline.recalculateAutoCrossfades();
      this.timeline.render();
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-trim-rate')?.addEventListener('input', (e) => {
      clip.playbackRate = Number((e.target as HTMLInputElement).value);
      this.renderInspectorPanel();
    });
    document.getElementById('inp-trim-gain')?.addEventListener('input', (e) => {
      clip.gain = Number((e.target as HTMLInputElement).value);
      this.timeline.render();
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('sel-fadein-curve')?.addEventListener('change', (e) => {
      clip.fadeInCurve = (e.target as HTMLSelectElement).value as FadeCurveType;
      this.timeline.render();
    });
    document.getElementById('sel-fadeout-curve')?.addEventListener('change', (e) => {
      clip.fadeOutCurve = (e.target as HTMLSelectElement).value as FadeCurveType;
      this.timeline.render();
    });
  }

  private initInteractivePanCropCanvas(clip: ClipEvent): void {
    const canvas = document.getElementById('pancrop-interactive-canvas') as HTMLCanvasElement | null;
    if (!canvas) return;
    this.drawPanCropInteractiveCanvas(clip);

    let dragMode: 'none' | 'move' | 'scale' | 'rotate' = 'none';
    let startX = 0;
    let startY = 0;
    let origX = 0;
    let origY = 0;
    let origScale = 1;
    let origRot = 0;

    canvas.addEventListener('mousedown', (e) => {
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const w = canvas.width;
      const h = canvas.height;
      const pc = clip.panCrop;
      const cx = w / 2 + pc.x * (w * 0.35);
      const cy = h / 2 + pc.y * (h * 0.35);
      const dist = Math.hypot(x - cx, y - cy);
      const boxRadius = Math.hypot((w * 0.55 * pc.scale) / 2, (h * 0.55 * pc.scale) / 2);

      startX = x;
      startY = y;
      origX = pc.x;
      origY = pc.y;
      origScale = pc.scale;
      origRot = pc.rotation;

      if (Math.abs(dist - boxRadius) < 12) {
        dragMode = 'scale';
      } else if (dist > boxRadius + 10) {
        dragMode = 'rotate';
      } else {
        dragMode = 'move';
      }
    });

    window.addEventListener('mousemove', (e) => {
      if (dragMode === 'none') return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const dx = x - startX;
      const dy = y - startY;

      if (dragMode === 'move') {
        clip.panCrop.x = Number(Math.max(-0.8, Math.min(0.8, origX + dx / 95)).toFixed(2));
        clip.panCrop.y = Number(Math.max(-0.8, Math.min(0.8, origY + dy / 55)).toFixed(2));
      } else if (dragMode === 'scale') {
        clip.panCrop.scale = Number(Math.max(0.25, Math.min(2.4, origScale + (dx + dy) / 140)).toFixed(2));
      } else if (dragMode === 'rotate') {
        clip.panCrop.rotation = Math.round(Math.max(-180, Math.min(180, origRot + dx)));
      }

      if (clip.panCrop.keyframes && clip.panCrop.keyframes.length > 0) {
        clip.panCrop.keyframes[clip.panCrop.keyframes.length - 1] = {
          time: clip.panCrop.keyframes[clip.panCrop.keyframes.length - 1].time,
          x: clip.panCrop.x,
          y: clip.panCrop.y,
          scale: clip.panCrop.scale,
          rotation: clip.panCrop.rotation,
        };
      }

      this.drawPanCropInteractiveCanvas(clip);
      this.compositor.renderFrame(this.project, clip, true);
    });

    window.addEventListener('mouseup', () => {
      if (dragMode !== 'none') {
        dragMode = 'none';
        this.renderInspectorPanel();
      }
    });
  }

  private drawPanCropInteractiveCanvas(clip: ClipEvent): void {
    const canvas = document.getElementById('pancrop-interactive-canvas') as HTMLCanvasElement | null;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const w = canvas.width;
    const h = canvas.height;
    ctx.fillStyle = '#0d1016';
    ctx.fillRect(0, 0, w, h);

    // Coordinate grid (Vegas Pan/Crop style)
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.16)';
    ctx.lineWidth = 1;
    for (let gx = 0; gx < w; gx += 20) {
      ctx.beginPath();
      ctx.moveTo(gx, 0);
      ctx.lineTo(gx, h);
      ctx.stroke();
    }
    for (let gy = 0; gy < h; gy += 20) {
      ctx.beginPath();
      ctx.moveTo(0, gy);
      ctx.lineTo(w, gy);
      ctx.stroke();
    }

    // Source frame reference rectangle
    const refW = w * 0.62;
    const refH = h * 0.62;
    ctx.strokeStyle = '#475569';
    ctx.strokeRect((w - refW) / 2, (h - refH) / 2, refW, refH);

    // Vegas interactive 'F' crop frame
    const pc = clip.panCrop;
    const cx = w / 2 + pc.x * (w * 0.35);
    const cy = h / 2 + pc.y * (h * 0.35);
    const bw = refW * pc.scale;
    const bh = refH * pc.scale;

    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate((pc.rotation * Math.PI) / 180);
    ctx.scale(pc.flipH ? -1 : 1, pc.flipV ? -1 : 1);

    // Rotation ring
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.32)';
    ctx.beginPath();
    ctx.arc(0, 0, Math.hypot(bw / 2, bh / 2), 0, Math.PI * 2);
    ctx.stroke();

    // Dotted crop rectangle
    ctx.strokeStyle = '#38bdf8';
    ctx.lineWidth = 1.6;
    ctx.setLineDash([4, 3]);
    ctx.strokeRect(-bw / 2, -bh / 2, bw, bh);
    ctx.setLineDash([]);

    // Iconic 'F' letter inside crop frame
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.75)';
    ctx.lineWidth = 2.4;
    ctx.beginPath();
    ctx.moveTo(-10, 14);
    ctx.lineTo(-10, -14);
    ctx.lineTo(10, -14);
    ctx.moveTo(-10, 0);
    ctx.lineTo(6, 0);
    ctx.stroke();

    // 4 Corner handles
    ctx.fillStyle = '#38bdf8';
    const corners = [
      [-bw / 2, -bh / 2],
      [bw / 2, -bh / 2],
      [bw / 2, bh / 2],
      [-bw / 2, bh / 2],
    ];
    for (const [hx, hy] of corners) {
      ctx.fillRect(hx - 3.5, hy - 3.5, 7, 7);
    }

    ctx.restore();
  }

  private drawResolveWheelsAndNodes(): void {
    document.querySelectorAll('canvas.resolve-wheel-canvas').forEach((canv) => {
      const c = canv as HTMLCanvasElement;
      const ctx = c.getContext('2d')!;
      const r = c.width / 2 - 4;
      const cx = c.width / 2;
      const cy = c.height / 2;

      for (let angle = 0; angle < 360; angle += 6) {
        const rad = (angle * Math.PI) / 180;
        const nextRad = ((angle + 7) * Math.PI) / 180;
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.arc(cx, cy, r, rad, nextRad);
        ctx.closePath();
        ctx.fillStyle = `hsla(${angle}, 75%, 45%, 0.65)`;
        ctx.fill();
      }
      ctx.strokeStyle = '#94a3b8';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.stroke();

      // Center crosshair puck
      ctx.fillStyle = '#f8fafc';
      ctx.fillRect(cx - 2, cy - 2, 4, 4);
    });

    const nodeCanv = document.getElementById('resolve-node-canvas') as HTMLCanvasElement | null;
    if (nodeCanv) {
      const nctx = nodeCanv.getContext('2d')!;
      nctx.fillStyle = '#0c0f15';
      nctx.fillRect(0, 0, nodeCanv.width, nodeCanv.height);
      const nodes = [
        { x: 12, label: 'MediaIn', col: '#2563eb' },
        { x: 96, label: 'CorridorKey', col: '#16a34a' },
        { x: 186, label: '3-Wheel Grade', col: '#d97706' },
        { x: 278, label: 'MediaOut', col: '#0891b2' },
      ];
      nctx.strokeStyle = '#64748b';
      nctx.lineWidth = 1.5;
      nctx.beginPath();
      nctx.moveTo(30, 29);
      nctx.lineTo(310, 29);
      nctx.stroke();

      for (const nd of nodes) {
        nctx.fillStyle = '#1e2430';
        nctx.fillRect(nd.x, 14, 66, 30);
        nctx.strokeStyle = nd.col;
        nctx.strokeRect(nd.x, 14, 66, 30);
        nctx.fillStyle = '#f1f5f9';
        nctx.font = 'bold 9px sans-serif';
        nctx.fillText(nd.label, nd.x + 5, 32);
      }
    }
  }

  private showClipContextMenu(clip: ClipEvent, track: Track, clientX: number, clientY: number): void {
    const menu = document.getElementById('clip-context-menu');
    if (!menu) return;
    menu.style.left = `${Math.min(window.innerWidth - 220, clientX)}px`;
    menu.style.top = `${Math.min(window.innerHeight - 260, clientY)}px`;
    menu.classList.remove('hidden');

    menu.innerHTML = `
      <div class="ctx-title">${clip.name}</div>
      <button class="ctx-item" data-ctx="split">Split at Playhead (S)</button>
      <button class="ctx-item" data-ctx="pancrop">Event Pan/Crop...</button>
      <button class="ctx-item" data-ctx="fx">Event Video FX / Frei0r...</button>
      <div class="ctx-sep"></div>
      <div class="ctx-sub">Vegas Fade Curve Type:</div>
      <button class="ctx-item" data-curve="smooth">Smooth S-Curve ${clip.fadeInCurve === 'smooth' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="fast">Fast Logarithmic ${clip.fadeInCurve === 'fast' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="slow">Slow Exponential ${clip.fadeInCurve === 'slow' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="linear">Linear ${clip.fadeInCurve === 'linear' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="sharp">Sharp ${clip.fadeInCurve === 'sharp' ? '✓' : ''}</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" data-ctx="dup">Duplicate Event</button>
      <button class="ctx-item danger" data-ctx="del">Delete Event (Del)</button>
    `;

    menu.querySelectorAll('[data-curve]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const c = (btn as HTMLElement).dataset.curve as FadeCurveType;
        clip.fadeInCurve = c;
        clip.fadeOutCurve = c;
        this.timeline.render();
        this.renderInspectorPanel();
      });
    });

    menu.querySelectorAll('[data-ctx]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const act = (btn as HTMLElement).dataset.ctx;
        if (act === 'split') this.splitAtPlayhead();
        else if (act === 'pancrop') {
          this.activeCenterTab = 'pancrop';
          this.showPanCropGizmo = true;
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
          this.compositor.renderFrame(this.project, clip, true);
        } else if (act === 'fx') {
          this.activeCenterTab = 'fxchain';
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
        } else if (act === 'dup') {
          const clone: ClipEvent = JSON.parse(JSON.stringify(clip));
          clone.id = `clip_${Date.now()}`;
          clone.start = Number((clip.start + clip.duration + 0.1).toFixed(3));
          track.clips.push(clone);
          this.timeline.selectedClipId = clone.id;
          this.timeline.render();
        } else if (act === 'del') {
          this.deleteSelectedClip();
        }
      });
    });
  }

  private updateTimecodeReadouts(): void {
    const tcMain = document.getElementById('vegas-big-timecode');
    if (tcMain) {
      tcMain.textContent = formatSMPTE(this.project.playhead, this.project.fps, true);
    }
    const tcSub = document.getElementById('vegas-sub-timecode');
    if (tcSub) {
      const frame = Math.floor(this.project.playhead * this.project.fps);
      tcSub.textContent = `F:${frame} • ${this.project.width}x${this.project.height} @${this.project.fps}p`;
    }
  }

  private updateMasterMeters(): void {
    const reading = this.audioEngine.syncAudio(this.project, this.isPlaying);
    const barL = document.getElementById('meter-bar-l');
    const barR = document.getElementById('meter-bar-r');
    const peakLbl = document.getElementById('meter-peak-db');

    const normL = Math.max(6, Math.min(100, ((reading.leftDb + 60) / 66) * 100));
    const normR = Math.max(6, Math.min(100, ((reading.rightDb + 60) / 66) * 100));

    if (barL) barL.style.height = `${normL.toFixed(1)}%`;
    if (barR) barR.style.height = `${normR.toFixed(1)}%`;
    if (peakLbl) {
      const maxPeak = Math.max(reading.leftDb, reading.rightDb);
      peakLbl.textContent = maxPeak <= -59 ? '-∞ dB' : `${maxPeak.toFixed(1)} dB`;
      peakLbl.style.color = reading.clipping ? '#ef4444' : '#38bdf8';
    }
  }

  public async openDiagnosticsModal(): Promise<void> {
    const modal = document.getElementById('modal-diagnostics');
    if (!modal) return;
    modal.classList.remove('hidden');

    const reportBox = document.getElementById('diag-fixes-list') || document.getElementById('diag-report-output');
    const errLogBox = document.getElementById('diag-error-log-box') || document.getElementById('diag-error-log-output');
    const btnRun = document.getElementById('btn-run-diag-repair');

    const runCheck = async () => {
      if (reportBox) {
        reportBox.innerHTML = `<div class="diag-fix-card">Analisando problemas recorrentes do VEGAS Pro (VFR, So4, Mídia Offline, Envelopes, Auto-Save)...</div>`;
      }
      try {
        const [fixRes, errRes] = await Promise.all([
          fetch('/api/diagnostics/fix-problems', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ project: this.project }),
          }).then((r) => r.json()),
          fetch('/api/diagnostics/errors').then((r) => r.json()),
        ]);

        if (fixRes.ok) {
          this.project = fixRes.project;
          this.timeline.setProject(this.project);
          if (reportBox) {
            const protections = (fixRes.protections_active || []) as string[];
            const fixes = (fixRes.fixes_applied || []) as string[];
            reportBox.innerHTML = [
              ...protections.map((p) => `<div class="diag-fix-card"><strong>[PROTEÇÃO ATIVA]</strong> ${p}</div>`),
              ...fixes.map((f) => `<div class="diag-fix-card"><strong>[CORREÇÃO APLICADA]</strong> ${f}</div>`),
            ].join('');
          }
          this.setStatusMessage('Diagnóstico e correção preventiva do VEGAS Pro concluídos com sucesso.');
        }
        if (errRes.ok && errLogBox) {
          errLogBox.textContent = (errRes.lines || []).join('\n');
        }
      } catch (err) {
        if (reportBox) reportBox.innerHTML = `<div class="diag-fix-card">Erro na verificação: ${String(err)}</div>`;
      }
    };

    if (btnRun) {
      btnRun.onclick = () => runCheck();
    }
    await runCheck();
  }

  public async openRenderModal(): Promise<void> {
    const modal = document.getElementById('modal-render-as');
    if (!modal) return;
    modal.classList.remove('hidden');

    const fgBox = document.getElementById('render-filtergraph-preview');
    const statusBox = document.getElementById('render-output-status');
    if (fgBox) fgBox.textContent = 'Compilando timeline via Rust Core / Python FFmpeg + Frei0r + LADSPA + CorridorKey...';
    if (statusBox) statusBox.innerHTML = '';

    try {
      const res = await fetch('/api/timeline/compile', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project: this.project, outputName: 'OpenVegas_Master_Output.mp4' }),
      }).then((r) => r.json());

      if (res.ok && fgBox) {
        fgBox.textContent = `# Auto-Crossfades: ${res.crossfades?.length || 0}\n${res.ffmpeg_plan?.command_str || ''}`;
      }
    } catch (err) {
      if (fgBox) fgBox.textContent = `Filtergraph Offline Preview: ${String(err)}`;
    }

    const startBtn = document.getElementById('btn-start-ffmpeg-render');
    if (startBtn) {
      startBtn.onclick = async () => {
        if (statusBox) {
          statusBox.innerHTML = `<div class="render-progress-pill">Renderizando MP4 multi-pista via FFmpeg 9.0.2...</div>`;
        }
        try {
          const outName = (document.getElementById('inp-render-filename') as HTMLInputElement)?.value || 'OpenVegas_Master.mp4';
          const r = await fetch('/api/render/run', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ project: this.project, outputName: outName }),
          }).then((x) => x.json());

          if (r.ok && statusBox) {
            const kb = Math.round((r.sizeBytes || 0) / 1024);
            statusBox.innerHTML = `
              <div class="render-success-card">
                <div><strong>Renderização Concluída em ${r.elapsedSec}s!</strong> (${kb} KB • ${r.crossfadesCount} crossfades)</div>
                <a class="qt-btn qt-btn-primary" href="${r.url}" download="${r.fileName}" target="_blank">Baixar / Abrir ${r.fileName}</a>
              </div>
            `;
            this.setStatusMessage(`Renderizado ${r.fileName} (${kb} KB) via FFmpeg`);
          } else if (statusBox) {
            statusBox.innerHTML = `<div class="render-error-card">Falha na renderização: ${r.error || 'Erro desconhecido'}</div>`;
          }
        } catch (err) {
          if (statusBox) statusBox.innerHTML = `<div class="render-error-card">${String(err)}</div>`;
        }
      };
    }
  }

  public openScriptingModal(): void {
    const modal = document.getElementById('modal-scripting');
    if (!modal) return;
    modal.classList.remove('hidden');

    const presetContainer = document.getElementById('script-preset-buttons');
    const codeArea = document.getElementById('script-code-editor') as HTMLTextAreaElement | null;
    const logBox = document.getElementById('script-output-console');

    if (presetContainer) {
      presetContainer.innerHTML = Object.entries(this.scriptPresets)
        .map(
          ([key, p]) =>
            `<button class="qt-btn" data-script-preset="${key}" title="${p.description}">${p.name}</button>`
        )
        .join('');

      presetContainer.querySelectorAll('[data-script-preset]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const k = (btn as HTMLElement).dataset.scriptPreset!;
          if (codeArea && this.scriptPresets[k]) {
            codeArea.value = this.scriptPresets[k].code;
          }
        });
      });
    }

    if (codeArea && !codeArea.value.trim()) {
      const firstPreset = Object.values(this.scriptPresets)[0];
      codeArea.value =
        firstPreset?.code ||
        `# Vegas Python Scripting API\nvegas.add_marker(vegas.cursor, "Script Marker", "#38bdf8")\nvegas.log("Marker added at cursor!")\n`;
    }

    const runBtn = document.getElementById('btn-execute-python-script');
    if (runBtn) {
      runBtn.onclick = async () => {
        if (!codeArea) return;
        if (logBox) logBox.textContent = 'Executando script Python na timeline...';
        try {
          const res = await fetch('/api/scripts/execute', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code: codeArea.value, project: this.project }),
          }).then((r) => r.json());

          if (res.ok) {
            this.project = res.project;
            this.timeline.setProject(this.project);
            this.renderTrackHeaders();
            this.renderInspectorPanel();
            this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
            if (logBox) {
              logBox.textContent = (res.logs || []).join('\n') || res.stdout || 'Script executado com sucesso.';
            }
          } else if (logBox) {
            logBox.textContent = `Erro: ${res.error}`;
          }
        } catch (err) {
          if (logBox) logBox.textContent = `Erro ao comunicar com backend Python: ${String(err)}`;
        }
      };
    }
  }

  private setStatusMessage(msg: string): void {
    const el = document.getElementById('vegas-footer-status');
    if (el) el.textContent = msg;
  }

  public exportProjectJson(): string {
    return JSON.stringify(this.project, null, 2);
  }

  public loadProjectJson(rawJson: string): void {
    try {
      const parsed = JSON.parse(rawJson) as VegasProject;
      if (parsed && Array.isArray(parsed.tracks)) {
        this.project = parsed;
        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.updateTimecodeReadouts();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.setStatusMessage(`Projeto carregado: "${this.project.name}"`);
      }
    } catch (err) {
      this.logErrorToBackend('LOAD_VEG_JSON', String(err));
    }
  }
}

declare global {
  interface Window {
    openVegasApp?: OpenVegasStudioApp;
  }
}

window.addEventListener('DOMContentLoaded', () => {
  const app = new OpenVegasStudioApp();
  window.openVegasApp = app;
  app.init().catch(console.error);
});
