/**
 * OpenNeoveg — Interactive Window Docking, Floating ("Janelas Soltas") & Multi-Monitor OS Window Engine
 * Allows every studio panel (Project Media, Inspector/Pan-Crop/Settings, Video Preview,
 * Master Mixer/Scopes, and Timeline) to be:
 * - Resized via interactive horizontal & vertical splitter bars while docked
 * - Detached ("Solta" / Undocked) inside the workspace by dragging its `⋮⋮` header grip or clicking Float
 * - Popped out into a TRUE Native Windows OS Window (`NeovegPopoutWindow`) that can be dragged freely
 *   to any secondary/tertiary monitor (`Screen.AllScreens`) via `[data-win-action="popout"]` or by dragging
 *   a floating panel past the outer edge of the main window!
 * - Synchronized in real time across all monitors via `BroadcastChannel('neoveg_multimonitor_sync')`
 */

import { Neoveg_ICONS } from './icons';

export interface FloatingRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PanelState {
  id: string;
  title: string;
  floating: boolean;
  poppedOutToOS: boolean;
  maximized: boolean;
  minimized: boolean;
  rect: FloatingRect;
  dockOrder: number;
  dockFlex: number;
}

export class NeovegHostDockManager {
  private panels: Map<string, PanelState> = new Map();
  private topZ = 200;
  private onLayoutResize: () => void;
  private popoutWindows: Map<string, Window | null> = new Map();
  private syncChannel: BroadcastChannel | null = null;
  public readonly activePopoutPanelId: string | null;

  private activeDragPanelId: string | null = null;
  private dragType: 'move' | 'resize-se' | 'resize-e' | 'resize-s' | 'resize-w' | 'undock-threshold' | null = null;
  private dragStartX = 0;
  private dragStartY = 0;
  private initialRect: FloatingRect = { x: 0, y: 0, w: 420, h: 320 };

  constructor(onLayoutResize: () => void) {
    this.onLayoutResize = onLayoutResize;
    const params = new URLSearchParams(window.location.search);
    this.activePopoutPanelId = params.get('popout');
    this.initDefaultPanelStates();

    try {
      this.syncChannel = new BroadcastChannel('neoveg_multimonitor_sync');
      this.syncChannel.addEventListener('message', (ev) => {
        const msg = ev.data;
        if (!msg || typeof msg !== 'object') return;
        if (msg.type === 'REDOCK_PANEL' && !this.activePopoutPanelId) {
          this.restoreFromOSPopout(msg.panelId);
        }
      });
    } catch {
      // Fallback if BroadcastChannel is unavailable
    }
  }

  private initDefaultPanelStates(): void {
    const defaults: PanelState[] = [
      {
        id: 'pane-media',
        title: 'Project Media / FX / Generators',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 40, y: 70, w: 430, h: 360 },
        dockOrder: 1,
        dockFlex: 28,
      },
      {
        id: 'pane-inspector',
        title: 'Event Pan/Crop, FX, Resolve & Configurações',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 490, y: 70, w: 420, h: 360 },
        dockOrder: 2,
        dockFlex: 28,
      },
      {
        id: 'pane-preview',
        title: 'Video Preview Monitor',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 930, y: 70, w: 480, h: 370 },
        dockOrder: 3,
        dockFlex: 32,
      },
      {
        id: 'pane-mixer',
        title: 'Master Audio Bus & RGB Scopes',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 1180, y: 450, w: 210, h: 360 },
        dockOrder: 4,
        dockFlex: 12,
      },
      {
        id: 'pane-timeline',
        title: 'Multi-Track Timeline & Envelopes',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 60, y: 430, w: 1280, h: 380 },
        dockOrder: 5,
        dockFlex: 100,
      },
    ];

    for (const p of defaults) {
      this.panels.set(p.id, p);
    }
  }

  public initDOM(): void {
    // If this window was spawned as a dedicated OS Multi-Monitor Popout Window (`?popout=pane-...`)
    if (this.activePopoutPanelId) {
      this.configureAsDedicatedOSPopoutWindow(this.activePopoutPanelId);
      return;
    }

    for (const [id, state] of this.panels.entries()) {
      const el = document.getElementById(id);
      if (!el) continue;

      const hdr = el.querySelector('.neoveg-win-titlebar') as HTMLElement | null;
      const winControls = el.querySelector('.win-controls') as HTMLElement | null;
      const btnFloat = el.querySelector('[data-win-action="float"]') as HTMLButtonElement | null;
      const btnMax = el.querySelector('[data-win-action="maximize"]') as HTMLButtonElement | null;
      const btnMin = el.querySelector('[data-win-action="minimize"]') as HTMLButtonElement | null;

      // Inject Multi-Monitor Native OS Popout Button if not already in DOM
      let btnPopout = el.querySelector('[data-win-action="popout"]') as HTMLButtonElement | null;
      if (!btnPopout && winControls) {
        btnPopout = document.createElement('button');
        btnPopout.className = 'win-ctrl-btn popout-monitor-btn';
        btnPopout.setAttribute('data-win-action', 'popout');
        btnPopout.title = 'Destacar como Janela Nativa do Windows para arrastar para OUTRO MONITOR';
        btnPopout.innerHTML = `${Neoveg_ICONS.popoutMonitor}`;
        winControls.insertBefore(btnPopout, btnFloat);
      }

      btnPopout?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.popoutPanelToMonitor(id, e.screenX, e.screenY);
      });

      el.addEventListener('mousedown', () => {
        if (state.floating) {
          this.bringToFront(id);
        }
      });

      btnFloat?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleFloat(id);
      });

      btnMax?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleMaximize(id);
      });

      btnMin?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleMinimize(id);
      });

      hdr?.addEventListener('dblclick', (e) => {
        if ((e.target as HTMLElement).tagName === 'BUTTON') return;
        this.toggleFloat(id);
      });

      hdr?.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        const targetTag = (e.target as HTMLElement).tagName;
        if (targetTag === 'BUTTON' || targetTag === 'SELECT' || targetTag === 'INPUT' || targetTag === 'SVG' || targetTag === 'PATH') return;

        e.preventDefault();
        this.activeDragPanelId = id;
        this.dragStartX = e.clientX;
        this.dragStartY = e.clientY;

        if (state.floating) {
          this.bringToFront(id);
          this.dragType = 'move';
          this.initialRect = { ...state.rect };
        } else {
          this.dragType = 'undock-threshold';
          const domRect = el.getBoundingClientRect();
          this.initialRect = {
            x: Math.max(16, domRect.left),
            y: Math.max(52, domRect.top),
            w: Math.max(260, Math.min(960, domRect.width)),
            h: Math.max(220, Math.min(620, domRect.height)),
          };
        }
      });

      const handles: Array<{ cls: string; mode: typeof this.dragType }> = [
        { cls: 'win-resize-se', mode: 'resize-se' },
        { cls: 'win-resize-e', mode: 'resize-e' },
        { cls: 'win-resize-s', mode: 'resize-s' },
        { cls: 'win-resize-w', mode: 'resize-w' },
      ];
      for (const h of handles) {
        const handleEl = document.createElement('div');
        handleEl.className = `win-resize-handle ${h.cls}`;
        handleEl.addEventListener('mousedown', (e) => {
          if (!state.floating || e.button !== 0) return;
          e.preventDefault();
          e.stopPropagation();
          this.bringToFront(id);
          this.activeDragPanelId = id;
          this.dragType = h.mode;
          this.dragStartX = e.clientX;
          this.dragStartY = e.clientY;
          this.initialRect = { ...state.rect };
        });
        el.appendChild(handleEl);
      }
    }

    window.addEventListener('mousemove', (e) => this.onGlobalMouseMove(e));
    window.addEventListener('mouseup', (e) => this.onGlobalMouseUp(e));

    this.initWorkspaceSplitters();
    this.applyAllPanelStyles();
  }

  /**
   * Configures this browser/WebView2 instance when running inside a native OS Multi-Monitor Popout Window.
   */
  private configureAsDedicatedOSPopoutWindow(panelId: string): void {
    document.body.classList.add('is-multimonitor-popout');
    const state = this.panels.get(panelId);
    if (state) {
      document.title = `OpenNeoveg 21 — ${state.title} [Janela Multi-Monitor]`;
    }

    // Hide topbar, statusbar, welcome modal, and all other panels
    document.querySelector('.neoveg-topbar')?.classList.add('hidden');
    document.querySelector('.neoveg-statusbar')?.classList.add('hidden');
    document.getElementById('modal-welcome')?.classList.add('hidden');
    document.getElementById('workspace-h-splitter')?.classList.add('hidden');
    document.querySelectorAll('.dock-v-splitter').forEach((s) => s.classList.add('hidden'));

    for (const id of this.panels.keys()) {
      const el = document.getElementById(id);
      if (!el) continue;
      if (id === panelId) {
        el.style.position = 'fixed';
        el.style.inset = '0';
        el.style.width = '100vw';
        el.style.height = '100vh';
        el.style.zIndex = '999';
        const btnFloat = el.querySelector('[data-win-action="float"]') as HTMLElement | null;
        if (btnFloat) {
          btnFloat.innerHTML = `${Neoveg_ICONS.dockWindow}<span>Acoplar na Janela Principal</span>`;
          btnFloat.onclick = () => {
            this.syncChannel?.postMessage({ type: 'REDOCK_PANEL', panelId });
            window.close();
          };
        }
      } else {
        el.classList.add('hidden');
      }
    }

    window.addEventListener('beforeunload', () => {
      this.syncChannel?.postMessage({ type: 'REDOCK_PANEL', panelId });
    });
  }

  /**
   * Detaches a studio panel into a real native OS Window (`Form` / `window.open`) that can be dragged
   * freely across multiple monitors (`Screen.AllScreens`).
   */
  public popoutPanelToMonitor(panelId: string, screenX?: number, screenY?: number): void {
    const state = this.panels.get(panelId);
    if (!state) return;

    // If already popped out, focus existing OS window
    const existingWin = this.popoutWindows.get(panelId);
    if (existingWin && !existingWin.closed) {
      existingWin.focus();
      return;
    }

    const theme = document.documentElement.getAttribute('data-theme') || 'qt-neoveg21';
    const w = Math.max(420, state.rect.w || 580);
    const h = Math.max(320, state.rect.h || 420);
    const left = screenX !== undefined ? Math.max(0, screenX - 120) : window.screenX + 80;
    const top = screenY !== undefined ? Math.max(0, screenY - 20) : window.screenY + 80;

    // Notify C# native host (if running in WebView2) about desired multi-monitor window coordinates
    const wv = (window as unknown as { chrome?: { webview?: { postMessage: (msg: unknown) => void } } }).chrome?.webview;
    wv?.postMessage({
      action: 'prepare_popout_bounds',
      panelId,
      title: state.title,
      x: Math.round(left),
      y: Math.round(top),
      width: Math.round(w),
      height: Math.round(h),
    });

    const popUrl = `${window.location.origin}/?popout=${encodeURIComponent(panelId)}&theme=${encodeURIComponent(theme)}`;
    const features = `popup=yes,width=${Math.round(w)},height=${Math.round(h)},left=${Math.round(left)},top=${Math.round(top)},resizable=yes`;
    const newWin = window.open(popUrl, `neoveg_${panelId}`, features);

    if (newWin) {
      this.popoutWindows.set(panelId, newWin);
      state.poppedOutToOS = true;
      state.floating = false;
      this.applyAllPanelStyles();
      this.onLayoutResize();

      const watchTimer = window.setInterval(() => {
        if (newWin.closed) {
          window.clearInterval(watchTimer);
          this.restoreFromOSPopout(panelId);
        }
      }, 600);
    }
  }

  public restoreFromOSPopout(panelId: string): void {
    const state = this.panels.get(panelId);
    if (!state) return;
    const win = this.popoutWindows.get(panelId);
    if (win && !win.closed) {
      try {
        win.close();
      } catch {
        // Ignore
      }
    }
    this.popoutWindows.delete(panelId);
    state.poppedOutToOS = false;
    state.floating = false;
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

  private onGlobalMouseMove(e: MouseEvent): void {
    if (!this.activeDragPanelId || !this.dragType) return;
    const state = this.panels.get(this.activeDragPanelId);
    if (!state) return;

    const dx = e.clientX - this.dragStartX;
    const dy = e.clientY - this.dragStartY;

    if (this.dragType === 'undock-threshold') {
      if (Math.hypot(dx, dy) > 16) {
        state.floating = true;
        state.maximized = false;
        state.rect = {
          x: Math.max(10, e.clientX - Math.min(180, this.initialRect.w / 2)),
          y: Math.max(48, e.clientY - 14),
          w: this.initialRect.w,
          h: this.initialRect.h,
        };
        this.initialRect = { ...state.rect };
        this.dragStartX = e.clientX;
        this.dragStartY = e.clientY;
        this.dragType = 'move';
        this.bringToFront(state.id);
        this.applyAllPanelStyles();
        this.onLayoutResize();
      }
      return;
    }

    if (this.dragType === 'move') {
      state.rect.x = this.initialRect.x + dx;
      state.rect.y = Math.max(36, this.initialRect.y + dy);
      this.applyPanelStyle(state);
    } else if (this.dragType === 'resize-se') {
      state.rect.w = Math.max(200, this.initialRect.w + dx);
      state.rect.h = Math.max(150, this.initialRect.h + dy);
      this.applyPanelStyle(state);
      this.onLayoutResize();
    } else if (this.dragType === 'resize-e') {
      state.rect.w = Math.max(200, this.initialRect.w + dx);
      this.applyPanelStyle(state);
      this.onLayoutResize();
    } else if (this.dragType === 'resize-s') {
      state.rect.h = Math.max(150, this.initialRect.h + dy);
      this.applyPanelStyle(state);
      this.onLayoutResize();
    } else if (this.dragType === 'resize-w') {
      const newW = Math.max(200, this.initialRect.w - dx);
      state.rect.x = this.initialRect.x + (this.initialRect.w - newW);
      state.rect.w = newW;
      this.applyPanelStyle(state);
      this.onLayoutResize();
    }
  }

  private onGlobalMouseUp(e: MouseEvent): void {
    if (this.activeDragPanelId) {
      const panelId = this.activeDragPanelId;
      const mode = this.dragType;
      const state = this.panels.get(panelId);
      this.activeDragPanelId = null;
      this.dragType = null;

      // If user dragged a floating window past the left/right/top/bottom edge of the main window,
      // automatically pop it out into a Native OS Multi-Monitor Window so it can go to Monitor 2!
      if (
        mode === 'move' &&
        state &&
        state.floating &&
        (e.clientX <= 2 || e.clientX >= window.innerWidth - 2 || e.clientY <= 2 || e.clientY >= window.innerHeight - 2)
      ) {
        this.popoutPanelToMonitor(panelId, e.screenX, e.screenY);
        return;
      }

      if (state && state.floating) {
        state.rect.x = Math.max(4, Math.min(window.innerWidth - 120, state.rect.x));
        state.rect.y = Math.max(42, Math.min(window.innerHeight - 60, state.rect.y));
        this.applyPanelStyle(state);
      }
      this.onLayoutResize();
    }
  }

  public toggleFloat(panelId: string, forceState?: boolean): void {
    const state = this.panels.get(panelId);
    if (!state) return;
    const el = document.getElementById(panelId);

    const nextFloating = forceState !== undefined ? forceState : !state.floating;
    if (nextFloating && !state.floating && el) {
      const domRect = el.getBoundingClientRect();
      state.rect = {
        x: Math.max(24, Math.min(window.innerWidth - 360, domRect.left + 18)),
        y: Math.max(54, Math.min(window.innerHeight - 260, domRect.top + 18)),
        w: Math.max(240, Math.min(920, domRect.width)),
        h: Math.max(220, Math.min(580, domRect.height)),
      };
    }

    state.floating = nextFloating;
    state.maximized = false;
    state.minimized = false;
    if (state.floating) {
      this.bringToFront(panelId);
    }
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

  public toggleMaximize(panelId: string): void {
    const state = this.panels.get(panelId);
    if (!state) return;
    state.maximized = !state.maximized;
    state.minimized = false;
    if (state.maximized) {
      this.bringToFront(panelId);
    }
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

  public toggleMinimize(panelId: string): void {
    const state = this.panels.get(panelId);
    if (!state) return;
    state.minimized = !state.minimized;
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

  public floatAllPanels(): void {
    const presets: Record<string, FloatingRect> = {
      'pane-media': { x: 24, y: 58, w: 390, h: 340 },
      'pane-inspector': { x: 430, y: 58, w: 400, h: 340 },
      'pane-preview': { x: 845, y: 58, w: 460, h: 340 },
      'pane-mixer': { x: 1315, y: 58, w: 150, h: 340 },
    };
    for (const [id, rect] of Object.entries(presets)) {
      const st = this.panels.get(id);
      if (st) {
        st.floating = true;
        st.maximized = false;
        st.minimized = false;
        st.rect = { ...rect };
      }
    }
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

  public resetDefaultLayout(): void {
    for (const id of Array.from(this.popoutWindows.keys())) {
      this.restoreFromOSPopout(id);
    }
    this.initDefaultPanelStates();
    const upper = document.getElementById('neoveg-upper-workspace');
    if (upper) upper.style.height = '48%';
    const trackCol = document.getElementById('timeline-left-column');
    if (trackCol) trackCol.style.width = '255px';
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

  private bringToFront(panelId: string): void {
    this.topZ++;
    const el = document.getElementById(panelId);
    if (el) {
      el.style.zIndex = String(this.topZ);
    }
    document.querySelectorAll('.neoveg-dock-window').forEach((w) => w.classList.remove('win-focused'));
    el?.classList.add('win-focused');
  }

  private applyAllPanelStyles(): void {
    for (const state of this.panels.values()) {
      this.applyPanelStyle(state);
    }
    const upper = document.getElementById('neoveg-upper-workspace');
    const hSplitter = document.getElementById('workspace-h-splitter');
    const anyTopDocked = ['pane-media', 'pane-inspector', 'pane-preview', 'pane-mixer'].some((id) => {
      const st = this.panels.get(id);
      return st && !st.floating && !st.poppedOutToOS;
    });
    if (upper) {
      upper.classList.toggle('all-undocked', !anyTopDocked);
    }
    if (hSplitter) {
      hSplitter.classList.toggle('hidden', !anyTopDocked);
    }
  }

  private applyPanelStyle(state: PanelState): void {
    const el = document.getElementById(state.id);
    if (!el) return;

    if (state.poppedOutToOS) {
      el.classList.add('hidden');
      return;
    } else {
      el.classList.remove('hidden');
    }

    const btnFloat = el.querySelector('[data-win-action="float"]') as HTMLElement | null;
    const btnMax = el.querySelector('[data-win-action="maximize"]') as HTMLElement | null;
    const btnMin = el.querySelector('[data-win-action="minimize"]') as HTMLElement | null;

    el.classList.toggle('is-floating', state.floating);
    el.classList.toggle('is-maximized', state.maximized);
    el.classList.toggle('is-minimized', state.minimized);

    if (btnMin) {
      btnMin.innerHTML = Neoveg_ICONS.minimize;
    }
    if (btnFloat) {
      btnFloat.innerHTML = state.floating ? `${Neoveg_ICONS.dockWindow}` : `${Neoveg_ICONS.floatWindow}`;
      btnFloat.title = state.floating
        ? 'Acoplar janela de volta na grade (Dock)'
        : 'Soltar janela como flutuante (ou arraste até a borda da tela para jogar em outro monitor)';
    }
    if (btnMax) {
      btnMax.innerHTML = state.maximized ? Neoveg_ICONS.restore : Neoveg_ICONS.maximize;
    }

    if (state.maximized) {
      el.style.position = 'fixed';
      el.style.left = '8px';
      el.style.top = '48px';
      el.style.width = `${window.innerWidth - 16}px`;
      el.style.height = `${window.innerHeight - 76}px`;
      el.style.flex = 'none';
      return;
    }

    if (state.floating) {
      el.style.position = 'fixed';
      el.style.left = `${state.rect.x}px`;
      el.style.top = `${state.rect.y}px`;
      el.style.width = `${state.rect.w}px`;
      el.style.height = state.minimized ? '30px' : `${state.rect.h}px`;
      el.style.flex = 'none';
    } else {
      el.style.position = 'relative';
      el.style.left = '';
      el.style.top = '';
      el.style.width = '';
      el.style.height = '';
      el.style.zIndex = '';
      if (state.id !== 'pane-timeline') {
        el.style.flex = `${state.dockFlex} 1 0%`;
      } else {
        el.style.flex = '1 1 auto';
      }
    }
  }

  private initWorkspaceSplitters(): void {
    const hSplitter = document.getElementById('workspace-h-splitter');
    const upper = document.getElementById('neoveg-upper-workspace');
    if (hSplitter && upper) {
      let draggingH = false;
      hSplitter.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        draggingH = true;
        document.body.style.cursor = 'ns-resize';
      });
      window.addEventListener('mousemove', (e) => {
        if (!draggingH) return;
        const minH = 170;
        const maxH = window.innerHeight - 180;
        const newH = Math.max(minH, Math.min(maxH, e.clientY - 42));
        upper.style.height = `${newH}px`;
        this.onLayoutResize();
      });
      window.addEventListener('mouseup', () => {
        if (draggingH) {
          draggingH = false;
          document.body.style.cursor = '';
          this.onLayoutResize();
        }
      });
    }

    document.querySelectorAll('.dock-v-splitter').forEach((splitter) => {
      let draggingV = false;
      let leftId = '';
      let rightId = '';
      let startX = 0;
      let startLeftFlex = 25;
      let startRightFlex = 25;

      splitter.addEventListener('mousedown', (e) => {
        const me = e as MouseEvent;
        if (me.button !== 0) return;
        me.preventDefault();
        leftId = (splitter as HTMLElement).dataset.leftPane || '';
        rightId = (splitter as HTMLElement).dataset.rightPane || '';
        const pLeft = this.panels.get(leftId);
        const pRight = this.panels.get(rightId);
        if (!pLeft || !pRight || pLeft.floating || pRight.floating) return;

        draggingV = true;
        startX = me.clientX;
        startLeftFlex = pLeft.dockFlex;
        startRightFlex = pRight.dockFlex;
        document.body.style.cursor = 'ew-resize';
      });

      window.addEventListener('mousemove', (e) => {
        if (!draggingV) return;
        const pLeft = this.panels.get(leftId);
        const pRight = this.panels.get(rightId);
        if (!pLeft || !pRight) return;

        const totalWidth = upper?.clientWidth || window.innerWidth;
        const deltaPct = ((e.clientX - startX) / Math.max(400, totalWidth)) * 100;
        const sumFlex = startLeftFlex + startRightFlex;
        const newLeft = Math.max(10, Math.min(sumFlex - 10, startLeftFlex + deltaPct));
        const newRight = sumFlex - newLeft;

        pLeft.dockFlex = newLeft;
        pRight.dockFlex = newRight;
        this.applyPanelStyle(pLeft);
        this.applyPanelStyle(pRight);
        this.onLayoutResize();
      });

      window.addEventListener('mouseup', () => {
        if (draggingV) {
          draggingV = false;
          document.body.style.cursor = '';
          this.onLayoutResize();
        }
      });
    });

    const trackSplitter = document.getElementById('timeline-track-splitter');
    const trackCol = document.getElementById('timeline-left-column');
    if (trackSplitter && trackCol) {
      let draggingTr = false;
      trackSplitter.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        draggingTr = true;
        document.body.style.cursor = 'ew-resize';
      });
      window.addEventListener('mousemove', (e) => {
        if (!draggingTr) return;
        const rect = trackCol.parentElement?.getBoundingClientRect();
        const offsetLeft = rect ? rect.left : 0;
        const newW = Math.max(180, Math.min(460, e.clientX - offsetLeft));
        trackCol.style.width = `${newW}px`;
        this.onLayoutResize();
      });
      window.addEventListener('mouseup', () => {
        if (draggingTr) {
          draggingTr = false;
          document.body.style.cursor = '';
          this.onLayoutResize();
        }
      });
    }
  }
}
