<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Hand,
    WandSparkles,
    Eraser,
    Paintbrush,
    Brush,
    Crop,
    Scissors,
    Undo2,
    Redo2,
    ZoomIn,
    ZoomOut,
    Eye,
    RotateCcw,
    Upload,
    Save,
    Download,
    Cpu,
    Sparkles,
    Grid3x3,
    Sliders,
    Share2,
    Check,
    Loader2,
    FolderOpen,
    RefreshCw,
    Settings,
    MessageSquarePlus,
    Trash2,
    X,
    Plus
  } from '@lucide/svelte';
  import { api, assetUrl } from '$lib/api';
  import type { AssetItem, MattingModelInfo } from '$lib/api/types';
  import { appStore } from '$lib/stores/settings.svelte';
  import { jobsStore } from '$lib/stores/jobs.svelte';
  import { loadPersistedState, savePersistedState } from '$lib/stores/workspace';
  import { i18n } from '$lib/i18n/index.svelte';
  import ProviderModelPicker from '$lib/components/ui/ProviderModelPicker.svelte';
  import {
    cleanPixelArtCanvas,
    type PixelDetailMode,
    type PixelGridMode,
    type PixelPaletteMode
  } from '$lib/imaging/pixel-clean';
  import {
    applyWandFloodFix,
    removeSolidBackground,
    computeAlphaBounds
  } from '$lib/imaging/color-cutout';

  type ToolKind = 'pan' | 'restore' | 'erase' | 'brushR' | 'brushE' | 'crop' | 'annotate';
  type BackdropKind = 'checker' | 'dark' | 'light' | 'magenta';

  interface StudioRegionBox {
    id: string;
    index: number;
    x: number;
    y: number;
    w: number;
    h: number;
    note: string;
    color: string;
  }

  const REGION_COLORS = ['#00E5FF', '#FF3366', '#FFB800', '#A855F7', '#10B981', '#F97316'];

  interface HistorySnapshot {
    w: number;
    h: number;
    proc: Uint8ClampedArray;
    orig: Uint8ClampedArray;
  }

  interface CropRect {
    x0: number;
    y0: number;
    x1: number;
    y1: number;
  }

  // Viewport & Canvas state
  let viewportEl = $state<HTMLDivElement | null>(null);
  let displayCanvas = $state<HTMLCanvasElement | null>(null);
  let fileInputEl = $state<HTMLInputElement | null>(null);

  // Offscreen working layers
  let origCanvas: HTMLCanvasElement | null = null;
  let origData: ImageData | null = null;
  let procCanvas: HTMLCanvasElement | null = null;
  let procData: ImageData | null = null;
  let pixelBaseCanvas: HTMLCanvasElement | null = null;
  let origSourceDataUrl = $state<string>('');
  let currentAsset = $state<AssetItem | null>(null);

  // Dimensions & Transform
  let imgWidth = $state(0);
  let imgHeight = $state(0);
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let activeTool = $state<ToolKind>('pan');
  let spaceHeld = $state(false);
  let comparingOriginal = $state(false);
  let backdropMode = $state<BackdropKind>('checker');

  // Retouch parameters
  let tolerance = $state(34);
  let brushSize = $state(20);
  let cursorPos = $state<{ x: number; y: number; visible: boolean }>({
    x: 0,
    y: 0,
    visible: false
  });

  // Crop marquee
  let cropSel = $state<CropRect | null>(null);

  // Undo / Redo stacks
  let undoCount = $state(0);
  let redoCount = $state(0);
  const undoStack: HistorySnapshot[] = [];
  const redoStack: HistorySnapshot[] = [];
  const MAX_HISTORY = 12;

  // AI Matting Inspector state
  let mattingModels = $state<MattingModelInfo[]>([]);
  let selectedMattingModel = $state('birefnet-lite');
  let mattingDevice = $state<'auto' | 'gpu' | 'cpu'>('auto');
  let mattingEnhanced = $state(false);
  let mattingChoke = $state(0);
  let isMattingBusy = $state(false);
  let lastMattingProvider = $state('DirectML (GPU) / CPU Ready');

  // Pixel Art Cleaner state
  let blockSizeMode = $state<'auto' | 'manual'>('auto');
  let manualBlockSize = $state(8);
  let pixelDetailMode = $state<PixelDetailMode>('standard');
  let pixelGrid = $state<PixelGridMode>('auto');
  let pixelPalette = $state<PixelPaletteMode>('auto');
  let pixelCoalesce = $state(18);
  let pixelHardAlpha = $state(true);
  let showPixelGrid = $state(false);
  let lastPixelGridDims = $state<{ gw: number; gh: number } | null>(null);
  let lastPixelMeta = $state<string>('');

  // Custom Resolution Export state
  let exportW = $state(1024);
  let exportH = $state(1024);
  let exportKeepRatio = $state(true);
  let exportNearest = $state(false);

  // Annotated Region Edit (局部批注修改重绘) state
  let annotations = $state<StudioRegionBox[]>([]);
  let activeAnnotationId = $state<string | null>(null);
  let draftAnnotRect = $state<CropRect | null>(null);
  let regionEditMode = $state<'visual_guide' | 'mask_edit'>('visual_guide');
  let regionBlendMode = $state<'full' | 'patch'>('full');
  let regionGlobalNote = $state<string>('');
  let isRegionEditing = $state(false);
  let hasRegionEditResult = $state(false);
  let lastRegionEditFullCanvas: HTMLCanvasElement | null = null;
  let lastRegionEditPreCanvas: HTMLCanvasElement | null = null;
  let activeNoteInputEl = $state<HTMLInputElement | null>(null);

  // Filmstrip Gallery state
  let studioProjectFilter = $state<string>('_all');
  let galleryAssets = $state<AssetItem[]>([]);
  let isDragOver = $state(false);
  let dragDepth = 0;

  // Pointer drag tracking
  let dragState:
    | { mode: 'pan'; sx: number; sy: number; px: number; py: number }
    | { mode: 'brush'; lx: number; ly: number; restore: boolean }
    | { mode: 'crop' }
    | { mode: 'annot_draw' }
    | { mode: 'annot_move'; id: string; sx: number; sy: number; ox: number; oy: number }
    | { mode: 'annot_resize'; id: string; sx: number; sy: number; ow: number; oh: number }
    | null = null;

  let installedMattingModels = $derived(mattingModels.filter((m) => m.installed));
  let activeAnnotation = $derived(
    annotations.find((a) => a.id === activeAnnotationId) ?? null
  );

  let draftAnnotStyle = $derived.by(() => {
    if (!draftAnnotRect) return null;
    const x = Math.min(draftAnnotRect.x0, draftAnnotRect.x1);
    const y = Math.min(draftAnnotRect.y0, draftAnnotRect.y1);
    const w = Math.abs(draftAnnotRect.x1 - draftAnnotRect.x0);
    const h = Math.abs(draftAnnotRect.y1 - draftAnnotRect.y0);
    return {
      left: `${panX + x * zoom}px`,
      top: `${panY + y * zoom}px`,
      width: `${w * zoom}px`,
      height: `${h * zoom}px`,
      pixelW: Math.round(w),
      pixelH: Math.round(h)
    };
  });

  let cropBoxStyle = $derived.by(() => {
    if (!cropSel) return null;
    const x = Math.min(cropSel.x0, cropSel.x1);
    const y = Math.min(cropSel.y0, cropSel.y1);
    const w = Math.abs(cropSel.x1 - cropSel.x0);
    const h = Math.abs(cropSel.y1 - cropSel.y0);
    return {
      left: `${panX + x * zoom}px`,
      top: `${panY + y * zoom}px`,
      width: `${w * zoom}px`,
      height: `${h * zoom}px`,
      pixelW: Math.round(w),
      pixelH: Math.round(h),
      valid: w >= 4 && h >= 4
    };
  });

  let studioDraftHydrated = $state(false);

  onMount(async () => {
    const saved = loadPersistedState('studio', {
      lastAssetId: '',
      activeTool: 'pan' as ToolKind,
      backdropMode: 'checker' as BackdropKind,
      tolerance: 34,
      brushSize: 20,
      selectedMattingModel: 'birefnet-lite',
      mattingDevice: 'auto' as 'auto' | 'gpu' | 'cpu',
      mattingEnhanced: false,
      mattingChoke: 0,
      blockSizeMode: 'auto' as 'auto' | 'manual',
      manualBlockSize: 8,
      pixelDetailMode: 'standard' as PixelDetailMode,
      pixelGrid: 'auto' as PixelGridMode,
      pixelPalette: 'auto' as PixelPaletteMode,
      pixelCoalesce: 18,
      pixelHardAlpha: true,
      regionEditMode: 'visual_guide' as 'visual_guide' | 'mask_edit',
      regionBlendMode: 'full' as 'full' | 'patch',
      regionGlobalNote: '',
      annotations: [] as StudioRegionBox[]
    });

    activeTool = saved.activeTool || 'pan';
    backdropMode = saved.backdropMode || 'checker';
    tolerance = saved.tolerance ?? 34;
    brushSize = saved.brushSize ?? 20;
    selectedMattingModel = saved.selectedMattingModel || 'birefnet-lite';
    mattingDevice = saved.mattingDevice || 'auto';
    mattingEnhanced = saved.mattingEnhanced ?? false;
    mattingChoke = saved.mattingChoke ?? 0;
    blockSizeMode = saved.blockSizeMode || 'auto';
    manualBlockSize = saved.manualBlockSize ?? 8;
    pixelDetailMode = saved.pixelDetailMode || 'standard';
    pixelGrid = saved.pixelGrid || 'auto';
    pixelPalette = saved.pixelPalette || 'auto';
    pixelCoalesce = saved.pixelCoalesce ?? 18;
    pixelHardAlpha = saved.pixelHardAlpha ?? true;
    regionEditMode = saved.regionEditMode || 'visual_guide';
    regionBlendMode = saved.regionBlendMode || 'full';
    regionGlobalNote = saved.regionGlobalNote || '';

    if (appStore.settings?.activeProject) {
      studioProjectFilter = appStore.settings.activeProject;
    }
    await Promise.all([refreshMattingModels(), refreshFilmstrip()]);

    if (appStore.studioPendingAsset) {
      const target = appStore.studioPendingAsset;
      appStore.studioPendingAsset = null;
      await loadAssetIntoStudio(target);
    } else if (galleryAssets.length > 0) {
      const remembered = saved.lastAssetId
        ? galleryAssets.find((a) => a.id === saved.lastAssetId)
        : null;
      const target = remembered ?? galleryAssets[0]!;
      await loadAssetIntoStudio(target);
      if (remembered && Array.isArray(saved.annotations) && saved.annotations.length > 0) {
        annotations = saved.annotations;
        activeAnnotationId = saved.annotations[0]?.id ?? null;
      }
    }
    studioDraftHydrated = true;
  });

  $effect(() => {
    if (appStore.studioPendingAsset) {
      const target = appStore.studioPendingAsset;
      appStore.studioPendingAsset = null;
      loadAssetIntoStudio(target);
    }
  });

  $effect(() => {
    // Re-center viewport if Studio was initially mounted while hidden
    if (appStore.activeView === 'studio' && imgWidth > 0 && panX === 0 && panY === 0) {
      requestAnimationFrame(() => fitViewport());
    }
  });

  $effect(() => {
    // 监听视图切入或全局资产版本变动，确保工作台胶片栏实时更新
    const isStudio = appStore.activeView === 'studio';
    const _rev = appStore.assetsRevision;
    if (isStudio && studioDraftHydrated) {
      refreshFilmstrip();
    }
  });

  $effect(() => {
    if (!studioDraftHydrated) return;
    savePersistedState('studio', {
      lastAssetId: currentAsset?.id ?? '',
      activeTool,
      backdropMode,
      tolerance,
      brushSize,
      selectedMattingModel,
      mattingDevice,
      mattingEnhanced,
      mattingChoke,
      blockSizeMode,
      manualBlockSize,
      pixelDetailMode,
      pixelGrid,
      pixelPalette,
      pixelCoalesce,
      pixelHardAlpha,
      regionEditMode,
      regionBlendMode,
      regionGlobalNote,
      annotations
    });
  });

  async function refreshMattingModels() {
    try {
      const list = await api.listMattingModels();
      mattingModels = list;
      const installed = list.filter((m) => m.installed);
      if (installed.length > 0 && !installed.some((m) => m.id === selectedMattingModel)) {
        selectedMattingModel = installed[0]!.id;
      }
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to load matting models');
    }
  }

  async function refreshFilmstrip() {
    try {
      const proj = studioProjectFilter === '_all' ? undefined : studioProjectFilter;
      galleryAssets = await api.listAssets(proj, 80);
    } catch {
      // ignore
    }
  }

  function loadHtmlImage(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error('Failed to decode image into Studio canvas'));
      img.src = src;
    });
  }

  async function loadAssetIntoStudio(asset: AssetItem) {
    try {
      const dataUrl = await api.readImageDataUrl(asset.filePath);
      currentAsset = asset;
      await loadFromDataUrl(dataUrl);
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to load image file');
    }
  }

  async function loadFromDataUrl(dataUrl: string) {
    const im = await loadHtmlImage(dataUrl);
    const w = im.naturalWidth || im.width;
    const h = im.naturalHeight || im.height;

    const ocv = document.createElement('canvas');
    ocv.width = w;
    ocv.height = h;
    const octx = ocv.getContext('2d', { willReadFrequently: true })!;
    octx.drawImage(im, 0, 0);

    const pcv = document.createElement('canvas');
    pcv.width = w;
    pcv.height = h;
    const pctx = pcv.getContext('2d', { willReadFrequently: true })!;
    pctx.drawImage(im, 0, 0);

    origCanvas = ocv;
    origData = octx.getImageData(0, 0, w, h);
    procCanvas = pcv;
    procData = pctx.getImageData(0, 0, w, h);
    pixelBaseCanvas = null;
    origSourceDataUrl = dataUrl;

    imgWidth = w;
    imgHeight = h;
    exportW = w;
    exportH = h;
    cropSel = null;
    annotations = [];
    activeAnnotationId = null;
    draftAnnotRect = null;
    hasRegionEditResult = false;
    lastRegionEditFullCanvas = null;
    lastRegionEditPreCanvas = null;
    lastPixelGridDims = null;
    lastPixelMeta = '';
    undoStack.length = 0;
    redoStack.length = 0;
    undoCount = 0;
    redoCount = 0;

    renderDisplayCanvas();
    requestAnimationFrame(() => fitViewport());
  }

  function renderDisplayCanvas() {
    if (!displayCanvas || !procCanvas || !origCanvas) return;
    const source = comparingOriginal ? origCanvas : procCanvas;
    if (displayCanvas.width !== source.width || displayCanvas.height !== source.height) {
      displayCanvas.width = source.width;
      displayCanvas.height = source.height;
    }
    const ctx = displayCanvas.getContext('2d')!;
    ctx.clearRect(0, 0, displayCanvas.width, displayCanvas.height);
    ctx.drawImage(source, 0, 0);

    if (showPixelGrid && lastPixelGridDims && !comparingOriginal) {
      const { gw, gh } = lastPixelGridDims;
      const w = displayCanvas.width;
      const h = displayCanvas.height;
      ctx.save();
      ctx.strokeStyle = 'rgba(0, 229, 255, 0.35)';
      ctx.lineWidth = Math.max(1, 1 / Math.max(0.25, zoom));
      ctx.beginPath();
      for (let gx = 1; gx < gw; gx++) {
        const x = Math.round((gx * w) / gw) + 0.5;
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
      }
      for (let gy = 1; gy < gh; gy++) {
        const y = Math.round((gy * h) / gh) + 0.5;
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
      }
      ctx.stroke();
      ctx.restore();
    }
  }

  function syncWorkingData() {
    if (!procCanvas || !origCanvas) return;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    procData = pctx.getImageData(0, 0, procCanvas.width, procCanvas.height);
    const octx = origCanvas.getContext('2d', { willReadFrequently: true })!;
    origData = octx.getImageData(0, 0, origCanvas.width, origCanvas.height);
    imgWidth = procCanvas.width;
    imgHeight = procCanvas.height;
    renderDisplayCanvas();
  }

  function pushUndo() {
    if (!procCanvas || !origCanvas) return;
    const w = procCanvas.width;
    const h = procCanvas.height;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    const octx = origCanvas.getContext('2d', { willReadFrequently: true })!;
    if (undoStack.length >= MAX_HISTORY) undoStack.shift();
    undoStack.push({
      w,
      h,
      proc: new Uint8ClampedArray(pctx.getImageData(0, 0, w, h).data),
      orig: new Uint8ClampedArray(octx.getImageData(0, 0, w, h).data)
    });
    redoStack.length = 0;
    undoCount = undoStack.length;
    redoCount = 0;
  }

  function handleUndo() {
    if (!procCanvas || !origCanvas || undoStack.length === 0) return;
    const w = procCanvas.width;
    const h = procCanvas.height;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    const octx = origCanvas.getContext('2d', { willReadFrequently: true })!;
    redoStack.push({
      w,
      h,
      proc: new Uint8ClampedArray(pctx.getImageData(0, 0, w, h).data),
      orig: new Uint8ClampedArray(octx.getImageData(0, 0, w, h).data)
    });

    const snap = undoStack.pop()!;
    undoCount = undoStack.length;
    redoCount = redoStack.length;
    restoreSnapshot(snap);
  }

  function handleRedo() {
    if (!procCanvas || !origCanvas || redoStack.length === 0) return;
    const w = procCanvas.width;
    const h = procCanvas.height;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    const octx = origCanvas.getContext('2d', { willReadFrequently: true })!;
    undoStack.push({
      w,
      h,
      proc: new Uint8ClampedArray(pctx.getImageData(0, 0, w, h).data),
      orig: new Uint8ClampedArray(octx.getImageData(0, 0, w, h).data)
    });

    const snap = redoStack.pop()!;
    undoCount = undoStack.length;
    redoCount = redoStack.length;
    restoreSnapshot(snap);
  }

  function restoreSnapshot(snap: HistorySnapshot) {
    if (!procCanvas || !origCanvas) return;
    procCanvas.width = snap.w;
    procCanvas.height = snap.h;
    procCanvas
      .getContext('2d', { willReadFrequently: true })!
      .putImageData(new ImageData(new Uint8ClampedArray(snap.proc), snap.w, snap.h), 0, 0);

    origCanvas.width = snap.w;
    origCanvas.height = snap.h;
    origCanvas
      .getContext('2d', { willReadFrequently: true })!
      .putImageData(new ImageData(new Uint8ClampedArray(snap.orig), snap.w, snap.h), 0, 0);

    pixelBaseCanvas = null;
    exportW = snap.w;
    exportH = snap.h;
    syncWorkingData();
  }

  function handleResetToOriginal() {
    if (!procCanvas || !origCanvas) return;
    pushUndo();
    procCanvas.width = origCanvas.width;
    procCanvas.height = origCanvas.height;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    pctx.clearRect(0, 0, procCanvas.width, procCanvas.height);
    pctx.drawImage(origCanvas, 0, 0);
    pixelBaseCanvas = null;
    lastPixelGridDims = null;
    lastPixelMeta = '';
    syncWorkingData();
  }

  function fitViewport() {
    if (!viewportEl || !imgWidth || !imgHeight) return;
    const r = viewportEl.getBoundingClientRect();
    if (r.width < 20 || r.height < 20) return;
    const scale = Math.min(r.width / imgWidth, r.height / imgHeight) * 0.9;
    zoom = Math.max(0.05, Math.min(16, scale || 1));
    panX = (r.width - imgWidth * zoom) / 2;
    panY = (r.height - imgHeight * zoom) / 2;
  }

  function zoomAt(factor: number, cx: number, cy: number) {
    const nextZ = Math.min(24, Math.max(0.05, zoom * factor));
    panX = cx - ((cx - panX) * nextZ) / zoom;
    panY = cy - ((cy - panY) * nextZ) / zoom;
    zoom = nextZ;
  }

  function zoomCenter(factor: number) {
    if (!viewportEl) return;
    const r = viewportEl.getBoundingClientRect();
    zoomAt(factor, r.width / 2, r.height / 2);
  }

  function clientToImageXY(e: PointerEvent | MouseEvent): [number, number] {
    if (!viewportEl) return [0, 0];
    const r = viewportEl.getBoundingClientRect();
    return [(e.clientX - r.left - panX) / zoom, (e.clientY - r.top - panY) / zoom];
  }

  function stampBrush(x: number, y: number, restore: boolean) {
    if (!procCanvas || !origCanvas) return;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    pctx.save();
    pctx.beginPath();
    pctx.arc(x, y, brushSize / 2, 0, Math.PI * 2);
    if (restore) {
      pctx.clip();
      pctx.drawImage(origCanvas, 0, 0);
    } else {
      pctx.globalCompositeOperation = 'destination-out';
      pctx.fillStyle = '#000';
      pctx.fill();
    }
    pctx.restore();
  }

  function strokeBrushLine(x0: number, y0: number, x1: number, y1: number, restore: boolean) {
    const dist = Math.hypot(x1 - x0, y1 - y0);
    const step = Math.max(1, brushSize / 4);
    for (let t = 0; t <= dist; t += step) {
      const k = dist > 0 ? t / dist : 0;
      stampBrush(x0 + (x1 - x0) * k, y0 + (y1 - y0) * k, restore);
    }
  }

  function onViewportWheel(e: WheelEvent) {
    e.preventDefault();
    if (!viewportEl) return;
    const r = viewportEl.getBoundingClientRect();
    zoomAt(e.deltaY < 0 ? 1.2 : 1 / 1.2, e.clientX - r.left, e.clientY - r.top);
  }

  function onViewportPointerDown(e: PointerEvent) {
    if (!procCanvas || !origCanvas || !viewportEl) return;
    const [ix, iy] = clientToImageXY(e);

    if (activeTool === 'pan' || spaceHeld || e.button === 1) {
      dragState = { mode: 'pan', sx: e.clientX, sy: e.clientY, px: panX, py: panY };
      viewportEl.setPointerCapture(e.pointerId);
      return;
    }

    if (e.button !== 0) return;

    if (activeTool === 'restore' || activeTool === 'erase') {
      if (ix < 0 || iy < 0 || ix >= imgWidth || iy >= imgHeight) return;
      pushUndo();
      syncWorkingData();
      if (!origData || !procData) return;
      const changed = applyWandFloodFix(
        origData,
        procData,
        imgWidth,
        imgHeight,
        Math.floor(ix),
        Math.floor(iy),
        tolerance,
        activeTool
      );
      if (changed > 0) {
        procCanvas.getContext('2d', { willReadFrequently: true })!.putImageData(procData, 0, 0);
        pixelBaseCanvas = null;
        renderDisplayCanvas();
      } else {
        undoStack.pop();
        undoCount = undoStack.length;
      }
    } else if (activeTool === 'brushR' || activeTool === 'brushE') {
      pushUndo();
      pixelBaseCanvas = null;
      const restore = activeTool === 'brushR';
      dragState = { mode: 'brush', lx: ix, ly: iy, restore };
      stampBrush(ix, iy, restore);
      renderDisplayCanvas();
      viewportEl.setPointerCapture(e.pointerId);
    } else if (activeTool === 'crop') {
      dragState = { mode: 'crop' };
      cropSel = { x0: ix, y0: iy, x1: ix, y1: iy };
      viewportEl.setPointerCapture(e.pointerId);
    } else if (activeTool === 'annotate') {
      const clampedX = Math.max(0, Math.min(imgWidth, ix));
      const clampedY = Math.max(0, Math.min(imgHeight, iy));
      dragState = { mode: 'annot_draw' };
      draftAnnotRect = { x0: clampedX, y0: clampedY, x1: clampedX, y1: clampedY };
      viewportEl.setPointerCapture(e.pointerId);
    }
  }

  function onViewportPointerMove(e: PointerEvent) {
    if (viewportEl) {
      const r = viewportEl.getBoundingClientRect();
      cursorPos = {
        x: e.clientX - r.left,
        y: e.clientY - r.top,
        visible: activeTool === 'brushR' || activeTool === 'brushE'
      };
    }

    if (!dragState) return;
    const [ix, iy] = clientToImageXY(e);

    if (dragState.mode === 'pan') {
      panX = dragState.px + (e.clientX - dragState.sx);
      panY = dragState.py + (e.clientY - dragState.sy);
    } else if (dragState.mode === 'brush') {
      strokeBrushLine(dragState.lx, dragState.ly, ix, iy, dragState.restore);
      dragState.lx = ix;
      dragState.ly = iy;
      renderDisplayCanvas();
    } else if (dragState.mode === 'crop' && cropSel) {
      cropSel = { ...cropSel, x1: ix, y1: iy };
    } else if (dragState.mode === 'annot_draw' && draftAnnotRect) {
      const clampedX = Math.max(0, Math.min(imgWidth, ix));
      const clampedY = Math.max(0, Math.min(imgHeight, iy));
      draftAnnotRect = { ...draftAnnotRect, x1: clampedX, y1: clampedY };
    } else if (dragState.mode === 'annot_move') {
      const targetId = dragState.id;
      const dx = ix - dragState.sx;
      const dy = iy - dragState.sy;
      annotations = annotations.map((a) => {
        if (a.id !== targetId) return a;
        const nx = Math.round(Math.max(0, Math.min(imgWidth - a.w, dragState!.mode === 'annot_move' ? dragState!.ox + dx : a.x)));
        const ny = Math.round(Math.max(0, Math.min(imgHeight - a.h, dragState!.mode === 'annot_move' ? dragState!.oy + dy : a.y)));
        return { ...a, x: nx, y: ny };
      });
    } else if (dragState.mode === 'annot_resize') {
      const targetId = dragState.id;
      const dx = ix - dragState.sx;
      const dy = iy - dragState.sy;
      annotations = annotations.map((a) => {
        if (a.id !== targetId) return a;
        const nw = Math.round(
          Math.max(16, Math.min(imgWidth - a.x, dragState!.mode === 'annot_resize' ? dragState!.ow + dx : a.w))
        );
        const nh = Math.round(
          Math.max(16, Math.min(imgHeight - a.y, dragState!.mode === 'annot_resize' ? dragState!.oh + dy : a.h))
        );
        return { ...a, w: nw, h: nh };
      });
    }
  }

  function onViewportPointerUp() {
    if (dragState?.mode === 'brush') {
      syncWorkingData();
    } else if (dragState?.mode === 'annot_draw' && draftAnnotRect) {
      const x = Math.round(Math.min(draftAnnotRect.x0, draftAnnotRect.x1));
      const y = Math.round(Math.min(draftAnnotRect.y0, draftAnnotRect.y1));
      const w = Math.round(Math.abs(draftAnnotRect.x1 - draftAnnotRect.x0));
      const h = Math.round(Math.abs(draftAnnotRect.y1 - draftAnnotRect.y0));
      draftAnnotRect = null;
      if (w >= 14 && h >= 14) {
        const nextIdx = annotations.length + 1;
        const color = REGION_COLORS[(nextIdx - 1) % REGION_COLORS.length]!;
        const newBox: StudioRegionBox = {
          id: 'reg_' + Math.random().toString(36).slice(2, 9),
          index: nextIdx,
          x,
          y,
          w,
          h,
          note: '',
          color
        };
        annotations = [...annotations, newBox];
        activeAnnotationId = newBox.id;
        requestAnimationFrame(() => activeNoteInputEl?.focus());
      }
    }
    dragState = null;
  }

  function startMoveAnnotation(box: StudioRegionBox, e: PointerEvent) {
    if (spaceHeld || e.button !== 0) return;
    e.stopPropagation();
    activeAnnotationId = box.id;
    const [ix, iy] = clientToImageXY(e);
    dragState = { mode: 'annot_move', id: box.id, sx: ix, sy: iy, ox: box.x, oy: box.y };
    viewportEl?.setPointerCapture(e.pointerId);
  }

  function startResizeAnnotation(box: StudioRegionBox, e: PointerEvent) {
    if (spaceHeld || e.button !== 0) return;
    e.stopPropagation();
    activeAnnotationId = box.id;
    const [ix, iy] = clientToImageXY(e);
    dragState = { mode: 'annot_resize', id: box.id, sx: ix, sy: iy, ow: box.w, oh: box.h };
    viewportEl?.setPointerCapture(e.pointerId);
  }

  function deleteAnnotation(id: string) {
    const rest = annotations
      .filter((a) => a.id !== id)
      .map((a, idx) => ({
        ...a,
        index: idx + 1,
        color: REGION_COLORS[idx % REGION_COLORS.length]!
      }));
    annotations = rest;
    if (activeAnnotationId === id) {
      activeAnnotationId = rest[rest.length - 1]?.id ?? null;
    }
  }

  function clearAnnotations() {
    annotations = [];
    activeAnnotationId = null;
    draftAnnotRect = null;
  }

  function buildCleanWhiteBackedDataUrl(sourceCv: HTMLCanvasElement): string {
    const cv = document.createElement('canvas');
    cv.width = sourceCv.width;
    cv.height = sourceCv.height;
    const ctx = cv.getContext('2d')!;
    ctx.fillStyle = '#FFFFFF';
    ctx.fillRect(0, 0, cv.width, cv.height);
    ctx.drawImage(sourceCv, 0, 0);
    return cv.toDataURL('image/png');
  }

  function buildVisualGuideDataUrl(
    sourceCv: HTMLCanvasElement,
    boxes: StudioRegionBox[]
  ): string {
    const w = sourceCv.width;
    const h = sourceCv.height;
    const cv = document.createElement('canvas');
    cv.width = w;
    cv.height = h;
    const ctx = cv.getContext('2d')!;
    ctx.fillStyle = '#FFFFFF';
    ctx.fillRect(0, 0, w, h);
    ctx.drawImage(sourceCv, 0, 0);

    const strokeW = Math.max(3, Math.round(Math.min(w, h) / 170));
    const fontSize = Math.max(16, Math.round(Math.min(w, h) / 36));

    for (const b of boxes) {
      ctx.save();
      // High-contrast outer dark halo + inner neon border
      ctx.strokeStyle = 'rgba(0, 0, 0, 0.85)';
      ctx.lineWidth = strokeW + 2;
      ctx.strokeRect(b.x, b.y, b.w, b.h);

      ctx.strokeStyle = b.color;
      ctx.lineWidth = strokeW;
      ctx.strokeRect(b.x, b.y, b.w, b.h);

      // Number badge (#1, #2...)
      const label = `#${b.index}`;
      ctx.font = `900 ${fontSize}px sans-serif`;
      const tw = ctx.measureText(label).width;
      const padX = Math.round(fontSize * 0.45);
      const badgeW = tw + padX * 2;
      const badgeH = Math.round(fontSize * 1.35);
      const bx = Math.max(0, Math.min(w - badgeW, b.x));
      const by = b.y >= badgeH + 2 ? b.y - badgeH : b.y;

      ctx.fillStyle = b.color;
      ctx.fillRect(bx, by, badgeW, badgeH);
      ctx.strokeStyle = '#000000';
      ctx.lineWidth = 2;
      ctx.strokeRect(bx, by, badgeW, badgeH);

      ctx.fillStyle = '#000000';
      ctx.textBaseline = 'middle';
      ctx.fillText(label, bx + padX, by + badgeH / 2 + 1);
      ctx.restore();
    }

    return cv.toDataURL('image/png');
  }

  function buildInpaintMaskDataUrl(
    w: number,
    h: number,
    boxes: StudioRegionBox[]
  ): string {
    const cv = document.createElement('canvas');
    cv.width = w;
    cv.height = h;
    const ctx = cv.getContext('2d')!;
    // OpenAI /v1/images/edits mask rule: opaque pixels are preserved, transparent (alpha=0) pixels are inpainted
    ctx.fillStyle = 'rgba(0, 0, 0, 1)';
    ctx.fillRect(0, 0, w, h);
    for (const b of boxes) {
      const pad = 4;
      const x = Math.max(0, b.x - pad);
      const y = Math.max(0, b.y - pad);
      const rw = Math.min(w - x, b.w + pad * 2);
      const rh = Math.min(h - y, b.h + pad * 2);
      ctx.clearRect(x, y, rw, rh);
    }
    return cv.toDataURL('image/png');
  }

  function applyRegionBlendResult(mode: 'full' | 'patch') {
    regionBlendMode = mode;
    if (!procCanvas || !lastRegionEditFullCanvas || !lastRegionEditPreCanvas) return;
    const w = procCanvas.width;
    const h = procCanvas.height;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;

    if (mode === 'full' || annotations.length === 0) {
      pctx.clearRect(0, 0, w, h);
      pctx.drawImage(lastRegionEditFullCanvas, 0, 0, w, h);
      pixelBaseCanvas = null;
      syncWorkingData();
      return;
    }

    // Patch mode: keep everything outside the annotated boxes 100% locked to pre-edit canvas (including transparent background),
    // and feather-blend only the annotated regions from the AI result.
    const preCtx = lastRegionEditPreCanvas.getContext('2d', { willReadFrequently: true })!;
    const fullCtx = lastRegionEditFullCanvas.getContext('2d', { willReadFrequently: true })!;
    const preImg = preCtx.getImageData(0, 0, w, h);
    const fullImg = fullCtx.getImageData(0, 0, w, h);
    const outImg = pctx.createImageData(w, h);
    outImg.data.set(preImg.data);

    const preD = preImg.data;
    const fullD = fullImg.data;
    const outD = outImg.data;

    // Detect if pre-edit canvas had transparent background
    let transparentCornerCount = 0;
    const corners: [number, number][] = [
      [2, 2],
      [w - 3, 2],
      [2, h - 3],
      [w - 3, h - 3]
    ];
    let bgR = 255;
    let bgG = 255;
    let bgB = 255;
    let bgSampleCount = 0;
    for (const [cx, cy] of corners) {
      if (cx >= 0 && cy >= 0 && cx < w && cy < h) {
        const ci = (cy * w + cx) * 4;
        if ((preD[ci + 3] ?? 255) < 32) transparentCornerCount++;
        bgR += fullD[ci] ?? 255;
        bgG += fullD[ci + 1] ?? 255;
        bgB += fullD[ci + 2] ?? 255;
        bgSampleCount++;
      }
    }
    if (bgSampleCount > 0) {
      bgR = Math.round((bgR - 255) / bgSampleCount);
      bgG = Math.round((bgG - 255) / bgSampleCount);
      bgB = Math.round((bgB - 255) / bgSampleCount);
    }
    const hadTransparentBg = transparentCornerCount >= 2;

    for (const b of annotations) {
      const pad = Math.max(4, Math.round(Math.min(b.w, b.h) * 0.08));
      const x0 = Math.max(0, b.x - pad);
      const y0 = Math.max(0, b.y - pad);
      const x1 = Math.min(w, b.x + b.w + pad);
      const y1 = Math.min(h, b.y + b.h + pad);
      const feather = Math.max(4, Math.min(18, Math.round(Math.min(x1 - x0, y1 - y0) * 0.14)));

      for (let py = y0; py < y1; py++) {
        const dyTop = py - y0;
        const dyBot = y1 - 1 - py;
        const wy = Math.min(1, Math.min(dyTop, dyBot) / feather);
        for (let px = x0; px < x1; px++) {
          const dxLeft = px - x0;
          const dxRight = x1 - 1 - px;
          const wx = Math.min(1, Math.min(dxLeft, dxRight) / feather);
          const weight = wx * wy;
          if (weight <= 0) continue;

          const idx = (py * w + px) * 4;
          const nr = fullD[idx] ?? 0;
          const ng = fullD[idx + 1] ?? 0;
          const nb = fullD[idx + 2] ?? 0;
          let na = fullD[idx + 3] ?? 255;

          // If pre-edit image had transparent backdrop and the new pixel inside the box is pure backdrop color while original was transparent, keep it transparent
          if (hadTransparentBg && (preD[idx + 3] ?? 0) < 32) {
            const distBg = Math.abs(nr - bgR) + Math.abs(ng - bgG) + Math.abs(nb - bgB);
            if (distBg < 28) {
              na = 0;
            }
          }

          const pr = preD[idx] ?? 0;
          const pg = preD[idx + 1] ?? 0;
          const pb = preD[idx + 2] ?? 0;
          const pa = preD[idx + 3] ?? 0;

          outD[idx] = Math.round(pr * (1 - weight) + nr * weight);
          outD[idx + 1] = Math.round(pg * (1 - weight) + ng * weight);
          outD[idx + 2] = Math.round(pb * (1 - weight) + nb * weight);
          outD[idx + 3] = Math.round(pa * (1 - weight) + na * weight);
        }
      }
    }

    pctx.putImageData(outImg, 0, 0);
    pixelBaseCanvas = null;
    syncWorkingData();
  }

  async function handleRunRegionEdit() {
    if (!procCanvas || !origCanvas || isRegionEditing) return;
    const validBoxes = annotations.filter((a) => a.note.trim().length > 0);
    if (validBoxes.length === 0) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please enter an edit instruction in the box first'
          : '请先在选框内输入修改指令（例如：把剑柄改成金色龙爪）'
      );
      if (annotations.length > 0 && annotations[0]) {
        activeAnnotationId = annotations[0].id;
        requestAnimationFrame(() => activeNoteInputEl?.focus());
      }
      return;
    }

    const prof = appStore.activeProfile;
    if (!prof || !prof.hasKey) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please configure a valid API Key in API & Settings first'
          : '请先在「API 与设置」中配置生图 API Key'
      );
      appStore.openSettings('providers');
      return;
    }

    isRegionEditing = true;
    pushUndo();

    // Save pre-edit canvas snapshot for real-time Patch vs Full switching
    const preCv = document.createElement('canvas');
    preCv.width = procCanvas.width;
    preCv.height = procCanvas.height;
    preCv.getContext('2d')!.drawImage(procCanvas, 0, 0);
    lastRegionEditPreCanvas = preCv;

    try {
      const cleanDataUrl = buildCleanWhiteBackedDataUrl(procCanvas);
      const guideDataUrl = buildVisualGuideDataUrl(procCanvas, validBoxes);
      const maskDataUrl =
        regionEditMode === 'mask_edit'
          ? buildInpaintMaskDataUrl(procCanvas.width, procCanvas.height, validBoxes)
          : null;

      const aspect = procCanvas.width / procCanvas.height;
      const sizeStr =
        aspect > 1.2 ? '1536x1024' : aspect < 0.8 ? '1024x1536' : '1024x1024';

      const res = await api.runStudioRegionEdit({
        profileId: prof.id,
        model: prof.apiModel,
        mode: regionEditMode,
        originalDataUrl: cleanDataUrl,
        guideDataUrl,
        maskDataUrl,
        annotations: validBoxes.map((b) => ({
          index: b.index,
          x: b.x,
          y: b.y,
          w: b.w,
          h: b.h,
          note: b.note.trim()
        })),
        globalNote: regionGlobalNote.trim(),
        size: sizeStr
      });

      const genImg = await loadHtmlImage(res.imageDataUrl);
      const fullCv = document.createElement('canvas');
      fullCv.width = procCanvas.width;
      fullCv.height = procCanvas.height;
      const fctx = fullCv.getContext('2d')!;
      fctx.imageSmoothingEnabled = !exportNearest;
      fctx.drawImage(genImg, 0, 0, procCanvas.width, procCanvas.height);

      lastRegionEditFullCanvas = fullCv;
      hasRegionEditResult = true;
      applyRegionBlendResult(regionBlendMode);

      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? `Region redraw complete (${(res.elapsedMs / 1000).toFixed(1)}s · ${res.via})`
          : `局部批注修改完成 (${(res.elapsedMs / 1000).toFixed(1)}s · ${res.via})`
      );
    } catch (e: any) {
      undoStack.pop();
      undoCount = undoStack.length;
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Region redraw failed' : '局部批注修改失败')
      );
    } finally {
      isRegionEditing = false;
    }
  }

  function cropBothLayers(x: number, y: number, w2: number, h2: number) {
    if (!procCanvas || !origCanvas) return;
    pushUndo();
    const cut = (cv: HTMLCanvasElement) => {
      const tmp = document.createElement('canvas');
      tmp.width = w2;
      tmp.height = h2;
      tmp.getContext('2d')!.drawImage(cv, x, y, w2, h2, 0, 0, w2, h2);
      cv.width = w2;
      cv.height = h2;
      cv.getContext('2d')!.drawImage(tmp, 0, 0);
    };
    cut(procCanvas);
    cut(origCanvas);
    pixelBaseCanvas = null;
    lastPixelGridDims = null;
    cropSel = null;
    exportW = w2;
    exportH = h2;
    syncWorkingData();
    fitViewport();
  }

  function handleApplyCrop() {
    if (!cropSel || !procCanvas) return;
    const x = Math.max(0, Math.round(Math.min(cropSel.x0, cropSel.x1)));
    const y = Math.max(0, Math.round(Math.min(cropSel.y0, cropSel.y1)));
    const w2 = Math.min(Math.round(Math.abs(cropSel.x1 - cropSel.x0)), procCanvas.width - x);
    const h2 = Math.min(Math.round(Math.abs(cropSel.y1 - cropSel.y0)), procCanvas.height - y);
    if (w2 < 4 || h2 < 4) return;
    cropBothLayers(x, y, w2, h2);
    appStore.toast('ok', `Cropped to ${w2} × ${h2}`);
  }

  function handleAutoTrim() {
    if (!procCanvas) return;
    syncWorkingData();
    if (!procData) return;
    const bbox = computeAlphaBounds(procData.data, procCanvas.width, procCanvas.height, 8);
    if (!bbox) {
      appStore.toast('info', 'No opaque pixels found to trim');
      return;
    }
    const pad = 4;
    const x = Math.max(0, bbox[0] - pad);
    const y = Math.max(0, bbox[1] - pad);
    const x2 = Math.min(procCanvas.width, bbox[0] + bbox[2] + pad);
    const y2 = Math.min(procCanvas.height, bbox[1] + bbox[3] + pad);
    cropBothLayers(x, y, x2 - x, y2 - y);
    appStore.toast('ok', `Auto-trimmed to ${x2 - x} × ${y2 - y}`);
  }

  async function handleRunAiMatting() {
    if (!origCanvas || !procCanvas || isMattingBusy) return;
    if (installedMattingModels.length === 0) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'No installed matting model found. Please download one in Settings.'
          : '未检测到已安装的抠图模型，请在设置中下载'
      );
      appStore.activeView = 'settings';
      return;
    }
    isMattingBusy = true;
    pushUndo();
    try {
      const sourceUrl = origCanvas.toDataURL('image/png');
      const res = await api.runAiMatting({
        imageDataUrl: sourceUrl,
        modelId: selectedMattingModel,
        device: mattingDevice,
        enhanced: mattingEnhanced,
        choke: mattingChoke
      });
      const mattedImg = await loadHtmlImage(res.imageDataUrl);
      procCanvas.width = mattedImg.naturalWidth;
      procCanvas.height = mattedImg.naturalHeight;
      const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
      pctx.clearRect(0, 0, procCanvas.width, procCanvas.height);
      pctx.drawImage(mattedImg, 0, 0);
      pixelBaseCanvas = null;
      lastMattingProvider = `${res.providerInfo} · ${(res.elapsedMs / 1000).toFixed(2)}s`;
      syncWorkingData();
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? `AI matting complete (${(res.elapsedMs / 1000).toFixed(1)}s)`
          : `AI 抠图完成 (${(res.elapsedMs / 1000).toFixed(1)}s)`
      );
    } catch (e: any) {
      undoStack.pop();
      undoCount = undoStack.length;
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'AI matting failed' : 'AI 抠图失败')
      );
    } finally {
      isMattingBusy = false;
    }
  }

  function handleSolidBgCutout() {
    if (!origCanvas || !procCanvas) return;
    pushUndo();
    const resCanvas = removeSolidBackground(origCanvas, tolerance, true);
    procCanvas.width = resCanvas.width;
    procCanvas.height = resCanvas.height;
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    pctx.clearRect(0, 0, procCanvas.width, procCanvas.height);
    pctx.drawImage(resCanvas, 0, 0);
    pixelBaseCanvas = null;
    syncWorkingData();
    appStore.toast(
      'ok',
      i18n.locale === 'en'
        ? 'Solid background removed with 3px edge color unmixing'
        : '纯色背景已剔除，并完成 3px 边缘反解去白边'
    );
  }

  function ensurePixelBaseCanvas(): HTMLCanvasElement {
    if (
      pixelBaseCanvas &&
      procCanvas &&
      pixelBaseCanvas.width === procCanvas.width &&
      pixelBaseCanvas.height === procCanvas.height
    ) {
      return pixelBaseCanvas;
    }
    const base = document.createElement('canvas');
    base.width = procCanvas!.width;
    base.height = procCanvas!.height;
    base.getContext('2d')!.drawImage(procCanvas!, 0, 0);
    pixelBaseCanvas = base;
    return base;
  }

  function handleRunPixelClean() {
    if (!procCanvas || !origCanvas) return;
    const baseSource = ensurePixelBaseCanvas();
    pushUndo();
    const res = cleanPixelArtCanvas(baseSource, {
      grid: pixelGrid,
      manualBlockSize: blockSizeMode === 'manual' ? manualBlockSize : undefined,
      palette: pixelPalette,
      coalesceTolerance: pixelCoalesce,
      hardAlpha: pixelHardAlpha,
      denoise: true,
      detailMode: pixelDetailMode
    });
    const pctx = procCanvas.getContext('2d', { willReadFrequently: true })!;
    pctx.clearRect(0, 0, procCanvas.width, procCanvas.height);
    pctx.drawImage(res.canvas, 0, 0);

    exportNearest = true;
    lastPixelGridDims = { gw: res.gw, gh: res.gh };
    const stepStr =
      Math.abs(res.stepX - res.stepY) < 0.3
        ? `${res.stepX}px`
        : `${res.stepX}×${res.stepY}px`;
    lastPixelMeta = `${stepStr} → ${res.gw}×${res.gh} (${res.colorCount}${i18n.locale === 'en' ? ' colors' : '色'})`;
    syncWorkingData();
    appStore.toast(
      'ok',
      i18n.locale === 'en'
        ? `Pixel grid cleaned: step ${stepStr} · grid ${res.gw}×${res.gh} (${res.colorCount} colors)`
        : `像素纯化完成: 步长 ${stepStr} · 规整网格 ${res.gw}×${res.gh} (${res.colorCount} 色)`
    );
  }

  async function handleNativePixelExport() {
    if (!procCanvas) return;
    const baseSource = ensurePixelBaseCanvas();
    const res = cleanPixelArtCanvas(baseSource, {
      grid: pixelGrid,
      manualBlockSize: blockSizeMode === 'manual' ? manualBlockSize : undefined,
      palette: pixelPalette,
      coalesceTolerance: pixelCoalesce,
      hardAlpha: pixelHardAlpha,
      denoise: true,
      detailMode: pixelDetailMode
    });
    lastPixelGridDims = { gw: res.gw, gh: res.gh };
    const stepStr =
      Math.abs(res.stepX - res.stepY) < 0.3
        ? `${res.stepX}px`
        : `${res.stepX}×${res.stepY}px`;
    lastPixelMeta = `${stepStr} → ${res.gw}×${res.gh} (${res.colorCount}${i18n.locale === 'en' ? ' colors' : '色'})`;
    const dataUrl = res.nativeCanvas.toDataURL('image/png');
    const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
    const outPath = await api.exportStudioPng(dataUrl, `pixel_${res.gw}x${res.gh}_${ts}.png`, true);
    appStore.toast(
      'ok',
      i18n.locale === 'en'
        ? `Exported 1:1 native ${res.gw}×${res.gh} pixel PNG (${outPath})`
        : `已导出 1:1 原生 ${res.gw}×${res.gh} 真像素 PNG (${outPath})`
    );
  }

  function handleSendToForgeRefine() {
    if (!procCanvas) return;
    jobsStore.sketchPreview = procCanvas.toDataURL('image/png');
    if (currentAsset?.prompt && !jobsStore.promptDraft.trim()) {
      jobsStore.promptDraft = currentAsset.prompt;
    }
    appStore.activeView = 'generate';
    appStore.toast(
      'ok',
      i18n.locale === 'en'
        ? 'Sent current canvas to Asset Forge as reference image'
        : '已将当前规整画布作为垫图送回生图工坊，可直接重绘细化'
    );
  }

  async function handleSaveToLibrary() {
    if (!procCanvas) return;
    try {
      const dataUrl = procCanvas.toDataURL('image/png');
      const targetProj =
        studioProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : studioProjectFilter;
      const arch = currentAsset?.archetype || appStore.settings?.defaultArchetype || 'item_icon';
      const label = currentAsset?.prompt
        ? `${currentAsset.prompt} [Studio]`
        : 'Studio Processed Asset';
      const saved = await api.saveStudioAsset(dataUrl, targetProj, arch, label);
      currentAsset = saved;
      appStore.notifyAssetsChanged();
      appStore.notifyAssetsChanged();
    await Promise.all([refreshFilmstrip(), appStore.refreshProjects()]);
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? `Saved to project library (${saved.width}×${saved.height})`
          : `已保存至项目库 (${saved.width}×${saved.height})`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Save failed' : '保存失败'));
    }
  }

  async function handleQuickExportPng() {
    if (!procCanvas) return;
    try {
      const dataUrl = procCanvas.toDataURL('image/png');
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const outPath = await api.exportStudioPng(dataUrl, `asset_${imgWidth}x${imgHeight}_${ts}.png`, true);
      appStore.toast(
        'ok',
        i18n.locale === 'en' ? `Exported PNG: ${outPath}` : `已导出 PNG: ${outPath}`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Export failed' : '导出失败'));
    }
  }

  async function handleCustomSizeExport() {
    if (!procCanvas) return;
    let W = Math.round(Number(exportW) || 0);
    let H = Math.round(Number(exportH) || 0);
    if (W < 1 && H < 1) {
      appStore.toast(
        'err',
        i18n.locale === 'en' ? 'Please enter target width or height' : '请输入目标宽或高'
      );
      return;
    }
    const sw = procCanvas.width;
    const sh = procCanvas.height;
    if (W < 1) W = Math.max(1, Math.round((sw * H) / sh));
    if (H < 1) H = Math.max(1, Math.round((sh * W) / sw));

    const cv = document.createElement('canvas');
    cv.width = W;
    cv.height = H;
    const ctx = cv.getContext('2d')!;
    ctx.imageSmoothingEnabled = !exportNearest;
    if (!exportNearest) ctx.imageSmoothingQuality = 'high';

    if (exportKeepRatio) {
      const k = Math.min(W / sw, H / sh);
      const dw = Math.max(1, Math.round(sw * k));
      const dh = Math.max(1, Math.round(sh * k));
      ctx.drawImage(procCanvas, Math.round((W - dw) / 2), Math.round((H - dh) / 2), dw, dh);
    } else {
      ctx.drawImage(procCanvas, 0, 0, W, H);
    }

    try {
      const dataUrl = cv.toDataURL('image/png');
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const outPath = await api.exportStudioPng(dataUrl, `asset_${W}x${H}_${ts}.png`, true);
      appStore.toast(
        'ok',
        i18n.locale === 'en' ? `Exported ${W}×${H} PNG` : `已按 ${W}×${H} 导出 PNG`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Export failed' : '导出失败'));
    }
  }

  async function importExternalFiles(files: File[]) {
    const valid = files.filter(
      (f) => f.type.startsWith('image/') || /\.(png|jpe?g|webp|bmp|gif)$/i.test(f.name)
    );
    if (valid.length === 0) return;

    const targetProj =
      studioProjectFilter === '_all'
        ? appStore.settings?.activeProject || ''
        : studioProjectFilter;
    const arch = appStore.settings?.defaultArchetype || 'item_icon';

    let firstAsset: AssetItem | null = null;
    for (let i = 0; i < valid.length; i++) {
      const f = valid[i];
      if (!f) continue;
      const durl = await new Promise<string>((resolve, reject) => {
        const rd = new FileReader();
        rd.onload = () => resolve(String(rd.result || ''));
        rd.onerror = reject;
        rd.readAsDataURL(f);
      });
      try {
        const saved = await api.saveStudioAsset(
          durl,
          targetProj,
          arch,
          f.name.replace(/\.[^.]+$/, '')
        );
        if (i === 0) firstAsset = saved;
      } catch {
        // ignore individual failure
      }
    }

    appStore.notifyAssetsChanged();
    await Promise.all([refreshFilmstrip(), appStore.refreshProjects()]);
    if (firstAsset) {
      await loadAssetIntoStudio(firstAsset);
    }
    appStore.toast(
      'ok',
      i18n.locale === 'en' ? `Imported ${valid.length} image(s)` : `已导入 ${valid.length} 张图片`
    );
  }

  function onWindowKeyDown(e: KeyboardEvent) {
    if (appStore.activeView !== 'studio') return;
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

    if (e.code === 'Space') {
      spaceHeld = true;
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z' && !e.shiftKey) {
      e.preventDefault();
      handleUndo();
    } else if (
      ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') ||
      ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'z')
    ) {
      e.preventDefault();
      handleRedo();
    } else if (e.key === 'Enter' && activeTool === 'crop' && cropBoxStyle?.valid) {
      e.preventDefault();
      handleApplyCrop();
    }
  }

  function onWindowKeyUp(e: KeyboardEvent) {
    if (e.code === 'Space') {
      spaceHeld = false;
    }
  }
</script>

<svelte:window onkeydown={onWindowKeyDown} onkeyup={onWindowKeyUp} />

<div class="flex h-full w-full overflow-hidden bg-ink-0 select-none">
  <!-- LEFT & CENTER: Workbench Toolbar + Interactive Viewport + Bottom Filmstrip -->
  <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
    <!-- Top Workbench Tool Bar (No wrapping, whitespace-nowrap & shrink-0) -->
    <div
      class="flex h-12 shrink-0 items-center justify-between gap-3 overflow-x-auto border-b border-line bg-ink-1/90 px-3 no-scrollbar"
    >
      <!-- Left: Retouch Tool Selector -->
      <div class="flex shrink-0 items-center gap-1.5">
        <div class="flex shrink-0 items-center rounded-lg border border-line bg-ink-0 p-0.5">
          <button
            type="button"
            onclick={() => (activeTool = 'pan')}
            title={i18n.t('studio_tool_pan')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'pan'
              ? 'bg-accent text-accent-ink'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <Hand size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_pan_short')}</span>
          </button>

          <button
            type="button"
            onclick={() => (activeTool = 'restore')}
            title={i18n.t('studio_tool_wand_restore')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'restore'
              ? 'bg-ok text-ink-0'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <WandSparkles size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_restore_short')}</span>
          </button>

          <button
            type="button"
            onclick={() => (activeTool = 'erase')}
            title={i18n.t('studio_tool_wand_erase')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'erase'
              ? 'bg-err text-white'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <Eraser size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_erase_short')}</span>
          </button>

          <button
            type="button"
            onclick={() => (activeTool = 'brushR')}
            title={i18n.t('studio_tool_brush_restore')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'brushR'
              ? 'bg-ok text-ink-0'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <Paintbrush size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_brush_r_short')}</span>
          </button>

          <button
            type="button"
            onclick={() => (activeTool = 'brushE')}
            title={i18n.t('studio_tool_brush_erase')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'brushE'
              ? 'bg-err text-white'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <Brush size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_brush_e_short')}</span>
          </button>

          <button
            type="button"
            onclick={() => (activeTool = 'crop')}
            title={i18n.t('studio_tool_crop')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'crop'
              ? 'bg-accent text-accent-ink'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <Crop size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_crop_short')}</span>
          </button>

          <button
            type="button"
            onclick={() => (activeTool = 'annotate')}
            title={i18n.t('studio_tool_annotate_tip')}
            class="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-semibold whitespace-nowrap transition-colors {activeTool ===
            'annotate'
              ? 'bg-accent text-accent-ink shadow-sm'
              : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <MessageSquarePlus size={14} strokeWidth={1.9} />
            <span>{i18n.t('studio_tool_annotate_short')}</span>
            {#if annotations.length > 0}
              <span
                class="rounded-full px-1.5 py-0.2 font-mono text-[10px] font-extrabold {activeTool ===
                'annotate'
                  ? 'bg-ink-0 text-accent'
                  : 'bg-accent text-accent-ink'}"
              >
                {annotations.length}
              </span>
            {/if}
          </button>
        </div>

        <div class="h-4 w-px shrink-0 bg-line"></div>

        <!-- Auto-Trim & Undo/Redo -->
        <button
          type="button"
          onclick={handleAutoTrim}
          disabled={!imgWidth}
          title={i18n.t('studio_btn_trim')}
          class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-2/70 px-2.5 py-1 text-xs font-medium whitespace-nowrap text-fg transition-colors hover:border-accent/50 hover:text-accent disabled:opacity-40"
        >
          <Scissors size={13} strokeWidth={1.8} />
          <span>{i18n.t('studio_btn_trim')}</span>
        </button>

        <button
          type="button"
          onclick={handleUndo}
          disabled={undoCount === 0}
          title={i18n.t('studio_btn_undo')}
          class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border border-line bg-ink-2/70 text-fg-muted transition-colors hover:text-fg disabled:opacity-35"
        >
          <Undo2 size={14} strokeWidth={1.8} />
        </button>

        <button
          type="button"
          onclick={handleRedo}
          disabled={redoCount === 0}
          title={i18n.t('studio_btn_redo')}
          class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border border-line bg-ink-2/70 text-fg-muted transition-colors hover:text-fg disabled:opacity-35"
        >
          <Redo2 size={14} strokeWidth={1.8} />
        </button>

        <button
          type="button"
          onpointerdown={() => {
            comparingOriginal = true;
            renderDisplayCanvas();
          }}
          onpointerup={() => {
            comparingOriginal = false;
            renderDisplayCanvas();
          }}
          onpointerleave={() => {
            if (comparingOriginal) {
              comparingOriginal = false;
              renderDisplayCanvas();
            }
          }}
          disabled={!imgWidth}
          title={i18n.t('studio_btn_compare')}
          class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-2/70 px-2 py-1 text-xs font-medium whitespace-nowrap text-fg-muted transition-colors hover:text-fg active:border-accent active:text-accent disabled:opacity-35"
        >
          <Eye size={13} strokeWidth={1.8} />
          <span>{i18n.t('studio_btn_compare_short')}</span>
        </button>

        <button
          type="button"
          onclick={handleResetToOriginal}
          disabled={!imgWidth}
          title={i18n.t('studio_btn_reset_orig')}
          class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border border-line bg-ink-2/70 text-fg-muted transition-colors hover:text-fg disabled:opacity-35"
        >
          <RotateCcw size={13} strokeWidth={1.8} />
        </button>
      </div>

      <!-- Right: Backdrop Switcher, Zoom Controls & File Actions -->
      <div class="flex shrink-0 items-center gap-1.5">
        <!-- Backdrop Color Switcher -->
        <div
          class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-0 p-1"
          title={i18n.t('studio_backdrop_tip')}
        >
          <button
            type="button"
            onclick={() => (backdropMode = 'checker')}
            aria-label="Checkerboard backdrop"
            class="h-4 w-4 shrink-0 rounded-sm border {backdropMode === 'checker'
              ? 'border-accent ring-1 ring-accent'
              : 'border-line'}"
            style="background: repeating-conic-gradient(#2a2e3d 0% 25%, #181b24 0% 50%) 50% / 8px 8px;"
          ></button>
          <button
            type="button"
            onclick={() => (backdropMode = 'dark')}
            aria-label="Pure black backdrop"
            class="h-4 w-4 shrink-0 rounded-sm border bg-[#07080a] {backdropMode === 'dark'
              ? 'border-accent ring-1 ring-accent'
              : 'border-line'}"
          ></button>
          <button
            type="button"
            onclick={() => (backdropMode = 'light')}
            aria-label="Pure white backdrop"
            class="h-4 w-4 shrink-0 rounded-sm border bg-white {backdropMode === 'light'
              ? 'border-accent ring-1 ring-accent'
              : 'border-line'}"
          ></button>
          <button
            type="button"
            onclick={() => (backdropMode = 'magenta')}
            aria-label="Magenta inspection backdrop"
            class="h-4 w-4 shrink-0 rounded-sm border bg-[#ff00aa] {backdropMode === 'magenta'
              ? 'border-accent ring-1 ring-accent'
              : 'border-line'}"
          ></button>
        </div>

        <!-- Zoom Controls -->
        <div
          class="flex shrink-0 items-center gap-0.5 rounded-lg border border-line bg-ink-0 px-1 py-0.5"
        >
          <button
            type="button"
            onclick={() => zoomCenter(1 / 1.25)}
            aria-label="Zoom out"
            class="flex h-6 w-6 shrink-0 items-center justify-center rounded text-fg-muted hover:bg-ink-2 hover:text-fg"
          >
            <ZoomOut size={13} />
          </button>
          <button
            type="button"
            onclick={fitViewport}
            title={i18n.t('studio_btn_fit')}
            class="min-w-[46px] shrink-0 px-1 text-center font-mono text-[11px] font-bold whitespace-nowrap text-fg hover:text-accent"
          >
            {Math.round(zoom * 100)}%
          </button>
          <button
            type="button"
            onclick={() => zoomCenter(1.25)}
            aria-label="Zoom in"
            class="flex h-6 w-6 shrink-0 items-center justify-center rounded text-fg-muted hover:bg-ink-2 hover:text-fg"
          >
            <ZoomIn size={13} />
          </button>
        </div>

        <input
          bind:this={fileInputEl}
          type="file"
          accept="image/*"
          multiple
          class="hidden"
          onchange={(e) => {
            const list = Array.from((e.currentTarget as HTMLInputElement).files || []);
            if (list.length) importExternalFiles(list);
            (e.currentTarget as HTMLInputElement).value = '';
          }}
        />

        <button
          type="button"
          onclick={() => fileInputEl?.click()}
          class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-xs font-semibold whitespace-nowrap text-fg transition-colors hover:border-accent/50"
        >
          <Upload size={13} strokeWidth={1.9} />
          <span>{i18n.t('studio_btn_import')}</span>
        </button>

        <button
          type="button"
          onclick={handleSaveToLibrary}
          disabled={!imgWidth}
          class="flex shrink-0 items-center gap-1 rounded-lg border border-accent/40 bg-accent/15 px-2.5 py-1 text-xs font-bold whitespace-nowrap text-accent transition-colors hover:bg-accent hover:text-accent-ink disabled:opacity-40"
        >
          <Save size={13} strokeWidth={2} />
          <span>{i18n.t('studio_btn_save_lib')}</span>
        </button>

        <button
          type="button"
          onclick={handleQuickExportPng}
          disabled={!imgWidth}
          class="flex shrink-0 items-center gap-1 rounded-lg bg-accent px-3 py-1 text-xs font-extrabold whitespace-nowrap text-accent-ink shadow-sm transition-transform active:scale-95 disabled:opacity-40"
        >
          <Download size={13} strokeWidth={2.2} />
          <span>{i18n.t('studio_btn_quick_dl')}</span>
        </button>
      </div>
    </div>

    <!-- Center Interactive Canvas Viewport -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      bind:this={viewportEl}
      onwheel={onViewportWheel}
      onpointerdown={onViewportPointerDown}
      onpointermove={onViewportPointerMove}
      onpointerup={onViewportPointerUp}
      onpointerleave={() => {
        cursorPos.visible = false;
      }}
      ondragenter={(e) => {
        e.preventDefault();
        dragDepth++;
        isDragOver = true;
      }}
      ondragover={(e) => {
        e.preventDefault();
        isDragOver = true;
      }}
      ondragleave={(e) => {
        e.preventDefault();
        dragDepth = Math.max(0, dragDepth - 1);
        if (dragDepth === 0) isDragOver = false;
      }}
      ondrop={(e) => {
        e.preventDefault();
        dragDepth = 0;
        isDragOver = false;
        const files = Array.from(e.dataTransfer?.files || []);
        if (files.length) importExternalFiles(files);
      }}
      class="relative flex-1 overflow-hidden {activeTool === 'pan' || spaceHeld
        ? 'cursor-grab active:cursor-grabbing'
        : 'cursor-crosshair'}"
      style={backdropMode === 'checker'
        ? 'background: repeating-conic-gradient(rgba(255,255,255,0.06) 0% 25%, rgba(0,0,0,0.22) 0% 50%) 50% / 20px 20px;'
        : backdropMode === 'dark'
          ? 'background: #060709;'
          : backdropMode === 'light'
            ? 'background: #f4f5f7;'
            : 'background: #d90096;'}
    >
      {#if !imgWidth}
        <div class="flex h-full w-full flex-col items-center justify-center p-8 text-center">
          <div class="mb-3 rounded-2xl border border-line bg-ink-1/80 p-4 text-accent">
            <Sliders size={28} strokeWidth={1.6} />
          </div>
          <h3 class="font-display text-base font-bold text-fg">
            {i18n.t('studio_empty_title')}
          </h3>
          <p class="mt-1.5 max-w-md text-xs text-fg-muted">
            {i18n.t('studio_empty_sub')}
          </p>
        </div>
      {/if}

      <!-- Transformed Working Canvas -->
      <canvas
        bind:this={displayCanvas}
        class="pointer-events-none absolute top-0 left-0 origin-top-left shadow-2xl {imgWidth
          ? 'block'
          : 'hidden'}"
        style="transform: translate({panX}px, {panY}px) scale({zoom}); image-rendering: {zoom >= 2 ||
        exportNearest
          ? 'pixelated'
          : 'auto'};"
      ></canvas>

      <!-- Live Brush Circle Overlay -->
      {#if cursorPos.visible && imgWidth > 0}
        <div
          class="pointer-events-none absolute rounded-full border-2 {activeTool === 'brushR'
            ? 'border-ok bg-ok/15'
            : 'border-err bg-err/15'}"
          style="width: {Math.max(4, brushSize * zoom)}px; height: {Math.max(
            4,
            brushSize * zoom
          )}px; left: {cursorPos.x - (brushSize * zoom) / 2}px; top: {cursorPos.y -
            (brushSize * zoom) / 2}px;"
        ></div>
      {/if}

      <!-- Crop Marquee Box Overlay -->
      {#if cropBoxStyle}
        <div
          class="pointer-events-none absolute border-2 border-dashed border-accent bg-accent/10 shadow-[0_0_0_9999px_rgba(0,0,0,0.45)]"
          style="left: {cropBoxStyle.left}; top: {cropBoxStyle.top}; width: {cropBoxStyle.width}; height: {cropBoxStyle.height};"
        >
          <div
            class="absolute -top-6 left-0 rounded bg-accent px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent-ink"
          >
            {cropBoxStyle.pixelW} × {cropBoxStyle.pixelH}
          </div>
          {#if cropBoxStyle.valid}
            <button
              type="button"
              onclick={(e) => {
                e.stopPropagation();
                handleApplyCrop();
              }}
              class="pointer-events-auto absolute right-1 bottom-1 flex items-center gap-1 rounded-md bg-accent px-2.5 py-1 text-xs font-extrabold whitespace-nowrap text-accent-ink shadow-lg hover:brightness-110"
            >
              <Check size={13} strokeWidth={2.5} />
              <span>{i18n.t('studio_btn_apply_crop')}</span>
            </button>
          {/if}
        </div>
      {/if}

      <!-- Live Draft Annotation Marquee Box -->
      {#if draftAnnotStyle && activeTool === 'annotate'}
        <div
          class="pointer-events-none absolute border-2 border-dashed border-accent bg-accent/15"
          style="left: {draftAnnotStyle.left}; top: {draftAnnotStyle.top}; width: {draftAnnotStyle.width}; height: {draftAnnotStyle.height};"
        >
          <div
            class="absolute -top-6 left-0 rounded bg-accent px-1.5 py-0.5 font-mono text-[10px] font-bold whitespace-nowrap text-accent-ink"
          >
            #{annotations.length + 1} · {draftAnnotStyle.pixelW} × {draftAnnotStyle.pixelH}
          </div>
        </div>
      {/if}

      <!-- Annotated Region Edit Boxes & Floating Popover Overlay -->
      {#if imgWidth > 0 && !comparingOriginal && (activeTool === 'annotate' || annotations.length > 0)}
        {#each annotations as box (box.id)}
          {@const isSelected = activeAnnotationId === box.id}
          {@const screenLeft = panX + box.x * zoom}
          {@const screenTop = panY + box.y * zoom}
          {@const screenW = Math.max(20, box.w * zoom)}
          {@const screenH = Math.max(20, box.h * zoom)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            onpointerdown={(e) => startMoveAnnotation(box, e)}
            class="absolute transition-shadow {activeTool === 'annotate'
              ? 'pointer-events-auto cursor-move'
              : 'pointer-events-none'}"
            style="left: {screenLeft}px; top: {screenTop}px; width: {screenW}px; height: {screenH}px; border: 2px {isSelected
              ? 'solid'
              : 'dashed'} {box.color}; background: {isSelected
              ? `${box.color}22`
              : `${box.color}12`}; box-shadow: {isSelected
              ? `0 0 0 1px rgba(0,0,0,0.7), 0 0 16px ${box.color}40`
              : '0 0 0 1px rgba(0,0,0,0.55)'};"
          >
            <!-- Top-Left Number Badge (#1, #2...) -->
            <div
              class="absolute -top-6 left-0 flex items-center gap-1 rounded-t-md px-2 py-0.5 font-mono text-[11px] font-black whitespace-nowrap text-ink-0 shadow-md"
              style="background: {box.color};"
            >
              <span>#{box.index}</span>
              {#if box.note.trim() && !isSelected}
                <span class="max-w-[160px] truncate font-sans font-bold">· {box.note}</span>
              {/if}
            </div>

            <!-- Top-Right Delete Button -->
            {#if activeTool === 'annotate'}
              <button
                type="button"
                onpointerdown={(e) => e.stopPropagation()}
                onclick={(e) => {
                  e.stopPropagation();
                  deleteAnnotation(box.id);
                }}
                title={i18n.t('studio_annot_clear')}
                class="pointer-events-auto absolute -top-6 right-0 flex h-5 w-5 items-center justify-center rounded-t-md bg-ink-0/90 text-fg-muted hover:bg-err hover:text-white"
              >
                <X size={12} />
              </button>

              <!-- Bottom-Right Resize Handle -->
              <div
                onpointerdown={(e) => startResizeAnnotation(box, e)}
                class="pointer-events-auto absolute -right-1.5 -bottom-1.5 h-3.5 w-3.5 cursor-nwse-resize rounded-xs border border-ink-0 shadow"
                style="background: {box.color};"
              ></div>
            {/if}

            <!-- Floating Popover Note Editor when Selected in Annotate Mode -->
            {#if isSelected && activeTool === 'annotate'}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                onpointerdown={(e) => e.stopPropagation()}
                class="pointer-events-auto absolute left-0 z-30 flex w-[310px] flex-col gap-2 rounded-xl border border-line bg-ink-1/95 p-2.5 shadow-2xl backdrop-blur-md"
                style={screenTop + screenH + 130 > (viewportEl?.clientHeight ?? 700)
                  ? 'bottom: calc(100% + 28px);'
                  : 'top: calc(100% + 8px);'}
              >
                <div class="flex items-center justify-between gap-1.5">
                  <div class="flex items-center gap-1.5 text-xs font-extrabold text-fg">
                    <span
                      class="rounded px-1.5 py-0.5 font-mono text-[10px] font-black text-ink-0"
                      style="background: {box.color};"
                    >
                      #{box.index}
                    </span>
                    <span>{i18n.t('studio_annot_pop_title')}</span>
                  </div>
                  <span class="font-mono text-[10px] text-fg-muted">{box.w}×{box.h}px</span>
                </div>

                <input
                  bind:this={activeNoteInputEl}
                  type="text"
                  bind:value={box.note}
                  placeholder={i18n.t('studio_annot_pop_ph')}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                      e.preventDefault();
                      handleRunRegionEdit();
                    } else if (e.key === 'Enter') {
                      e.preventDefault();
                      activeAnnotationId = null;
                    }
                  }}
                  class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs text-fg placeholder:text-fg-muted/50 focus:border-accent focus:outline-none"
                />

                <!-- Quick Prompt Chips -->
                <div class="flex flex-wrap gap-1">
                  {#each [i18n.t('studio_annot_chip_remove'), i18n.t('studio_annot_chip_fix'), i18n.t('studio_annot_chip_gold'), i18n.t('studio_annot_chip_ice')] as preset (preset)}
                    <button
                      type="button"
                      onclick={() => {
                        box.note = box.note.trim() ? `${box.note.trim()}, ${preset}` : preset;
                      }}
                      class="rounded border border-line bg-ink-2 px-1.5 py-0.5 text-[10px] font-medium whitespace-nowrap text-fg-muted transition-colors hover:border-accent/50 hover:text-fg"
                    >
                      +{preset}
                    </button>
                  {/each}
                </div>

                <div class="flex items-center justify-between gap-1.5 pt-0.5">
                  <button
                    type="button"
                    onclick={() => (activeAnnotationId = null)}
                    class="rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-[11px] font-semibold whitespace-nowrap text-fg-muted hover:text-fg"
                  >
                    {i18n.t('studio_annot_pop_ok')}
                  </button>

                  <button
                    type="button"
                    onclick={handleRunRegionEdit}
                    disabled={isRegionEditing || !box.note.trim()}
                    class="flex items-center gap-1 rounded-lg bg-accent px-3 py-1 text-[11px] font-extrabold whitespace-nowrap text-accent-ink shadow-sm hover:brightness-105 disabled:opacity-40"
                  >
                    {#if isRegionEditing}
                      <Loader2 size={12} class="animate-spin" />
                      <span>{i18n.t('studio_annot_running')}</span>
                    {:else}
                      <Sparkles size={12} />
                      <span>{i18n.t('studio_annot_pop_run')} ({annotations.filter((a) => a.note.trim()).length})</span>
                    {/if}
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/each}
      {/if}

      <!-- Top-Left Canvas Dimension & Status Pill -->
      {#if imgWidth > 0}
        <div
          class="pointer-events-none absolute top-3 left-3 flex items-center gap-2 rounded-lg border border-line bg-ink-1/90 px-2.5 py-1 font-mono text-[11px] whitespace-nowrap text-fg-muted backdrop-blur-md"
        >
          <span class="font-bold text-fg">{imgWidth} × {imgHeight} px</span>
          {#if comparingOriginal}
            <span class="rounded bg-warn/20 px-1.5 py-0.5 text-[10px] font-bold text-warn">
              ORIGINAL PREVIEW
            </span>
          {/if}
          {#if lastPixelMeta}
            <span class="rounded bg-accent/15 px-1.5 py-0.5 text-[10px] font-bold text-accent">
              {lastPixelMeta}
            </span>
          {/if}
        </div>
      {/if}

      <!-- Drag & Drop Overlay -->
      {#if isDragOver}
        <div
          class="pointer-events-none absolute inset-3 z-40 flex flex-col items-center justify-center rounded-2xl border-2 border-dashed border-accent bg-ink-0/85 backdrop-blur-md"
        >
          <Upload size={36} class="mb-2 text-accent" />
          <p class="font-display text-sm font-bold text-fg">{i18n.t('studio_drop_overlay')}</p>
        </div>
      {/if}
    </div>

    <!-- Bottom Project Asset Filmstrip -->
    <div class="flex h-28 shrink-0 flex-col border-t border-line bg-ink-1/95">
      <div class="flex h-7 items-center justify-between border-b border-line/60 px-3">
        <div class="flex items-center gap-2">
          <FolderOpen size={12} class="text-accent" />
          <span
            class="text-[11px] font-bold tracking-wider whitespace-nowrap text-fg-muted uppercase"
          >
            {i18n.t('studio_filmstrip_title')}
          </span>
          <select
            bind:value={studioProjectFilter}
            onchange={refreshFilmstrip}
            class="rounded border border-line bg-ink-0 px-2 py-0.5 text-[11px] font-medium text-fg focus:border-accent focus:outline-none"
          >
            <option value="_all">{i18n.t('project_all')}</option>
            <option value="">{i18n.t('project_uncategorized')}</option>
            {#each appStore.projects as p (p.name)}
              <option value={p.name}>{p.name} ({p.assetCount})</option>
            {/each}
          </select>
        </div>

        <button
          type="button"
          onclick={refreshFilmstrip}
          class="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] whitespace-nowrap text-fg-muted hover:bg-ink-2 hover:text-fg"
        >
          <RefreshCw size={11} />
          <span>{i18n.t('studio_refresh')}</span>
        </button>
      </div>

      <div class="flex flex-1 items-center gap-2 overflow-x-auto px-3 py-1.5">
        {#each galleryAssets as item (item.id)}
          <button
            type="button"
            onclick={() => loadAssetIntoStudio(item)}
            title="{item.prompt || item.filePath} ({item.width}×{item.height})"
            class="group relative h-16 w-16 shrink-0 overflow-hidden rounded-lg border transition-all {currentAsset?.id ===
            item.id
              ? 'border-accent ring-2 ring-accent/40'
              : 'border-line bg-ink-0 hover:border-fg-muted'}"
          >
            <img
              src={assetUrl(item.filePath)}
              alt={item.prompt}
              draggable="false"
              class="h-full w-full object-contain"
            />
          </button>
        {/each}
      </div>
    </div>
  </div>

  <!-- RIGHT INSPECTOR DOCK (Annotated Region Edit / AI Matting / Pixel Cleaner / Retouch & Export) -->
  <aside
    class="flex h-full w-[355px] shrink-0 flex-col gap-3 overflow-y-auto border-l border-line bg-ink-1 p-3"
  >
    <!-- CARD 0: Annotated Region Edit -->
    <section
      class="flex flex-col gap-2.5 rounded-xl border p-3 transition-colors {activeTool ===
        'annotate' || annotations.length > 0
        ? 'border-accent/60 bg-ink-0/90 shadow-sm'
        : 'border-line bg-ink-0/60'}"
    >
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-1.5 text-xs font-extrabold tracking-wide text-fg">
          <MessageSquarePlus size={14} class="text-accent" />
          <span>{i18n.t('studio_annot_title')}</span>
          {#if annotations.length > 0}
            <span
              class="rounded-full bg-accent px-1.5 py-0.2 font-mono text-[10px] font-black text-accent-ink"
            >
              {annotations.length}
            </span>
          {/if}
        </div>

        <div class="flex items-center gap-1">
          {#if annotations.length > 0}
            <button
              type="button"
              onclick={clearAnnotations}
              title={i18n.t('studio_annot_clear')}
              class="flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-semibold whitespace-nowrap text-fg-muted hover:bg-err/15 hover:text-err"
            >
              <Trash2 size={11} />
              <span>{i18n.t('studio_annot_clear')}</span>
            </button>
          {/if}
          <button
            type="button"
            onclick={() => (activeTool = activeTool === 'annotate' ? 'pan' : 'annotate')}
            class="flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11px] font-bold whitespace-nowrap transition-colors {activeTool ===
            'annotate'
              ? 'border-accent bg-accent text-accent-ink'
              : 'border-line bg-ink-2 text-fg hover:border-accent/50'}"
          >
            <Plus size={11} />
            <span>{activeTool === 'annotate' ? i18n.t('studio_annot_drawing') : i18n.t('studio_annot_start')}</span>
          </button>
        </div>
      </div>

      <!-- Active Provider & Model Selector right inside Studio -->
      <div class="flex flex-col gap-1">
        <span class="text-[11px] font-medium text-fg-muted">{i18n.t('studio_annot_model_label')}</span>
        <ProviderModelPicker />
      </div>

      <!-- Guidance Protocol Switcher: Visual Guide vs Alpha Mask -->
      <div class="flex flex-col gap-1">
        <span class="text-[11px] font-medium text-fg-muted">{i18n.t('studio_annot_protocol_label')}</span>
        <div class="flex rounded-lg border border-line bg-ink-1 p-0.5 text-xs">
          <button
            type="button"
            onclick={() => (regionEditMode = 'visual_guide')}
            class="flex-1 rounded-md py-1 text-center font-semibold whitespace-nowrap transition-colors {regionEditMode ===
            'visual_guide'
              ? 'bg-ink-2 text-accent shadow-sm'
              : 'text-fg-muted hover:text-fg'}"
          >
            {i18n.t('studio_annot_proto_visual')}
          </button>
          <button
            type="button"
            onclick={() => (regionEditMode = 'mask_edit')}
            class="flex-1 rounded-md py-1 text-center font-semibold whitespace-nowrap transition-colors {regionEditMode ===
            'mask_edit'
              ? 'bg-ink-2 text-accent shadow-sm'
              : 'text-fg-muted hover:text-fg'}"
          >
            {i18n.t('studio_annot_proto_mask')}
          </button>
        </div>
      </div>

      <!-- Post-Merge Strategy Switcher (Works Live After Generation too!) -->
      <div class="flex flex-col gap-1">
        <span class="text-[11px] font-medium text-fg-muted">
          {i18n.t('studio_annot_blend_label')} {hasRegionEditResult ? i18n.t('studio_annot_blend_live') : ''}
        </span>
        <div class="flex rounded-lg border border-line bg-ink-1 p-0.5 text-xs">
          <button
            type="button"
            onclick={() => applyRegionBlendResult('full')}
            class="flex-1 rounded-md py-1 text-center font-semibold whitespace-nowrap transition-colors {regionBlendMode ===
            'full'
              ? 'bg-ink-2 text-accent shadow-sm'
              : 'text-fg-muted hover:text-fg'}"
          >
            {i18n.t('studio_annot_blend_full')}
          </button>
          <button
            type="button"
            onclick={() => applyRegionBlendResult('patch')}
            class="flex-1 rounded-md py-1 text-center font-semibold whitespace-nowrap transition-colors {regionBlendMode ===
            'patch'
              ? 'bg-ink-2 text-accent shadow-sm'
              : 'text-fg-muted hover:text-fg'}"
          >
            {i18n.t('studio_annot_blend_patch')}
          </button>
        </div>
      </div>

      <!-- Annotated Regions List -->
      {#if annotations.length === 0}
        <button
          type="button"
          onclick={() => (activeTool = 'annotate')}
          class="flex flex-col items-center justify-center gap-1 rounded-lg border border-dashed border-line bg-ink-1/50 px-3 py-3 text-center transition-colors hover:border-accent/50"
        >
          <span class="text-xs font-semibold text-fg">
            {i18n.t('studio_annot_empty_title')}
          </span>
          <span class="text-[11px] text-fg-muted">
            {i18n.t('studio_annot_empty_sub')}
          </span>
        </button>
      {:else}
        <div class="flex max-h-44 flex-col gap-1.5 overflow-y-auto pr-0.5">
          {#each annotations as item (item.id)}
            <div
              onclick={() => {
                activeAnnotationId = item.id;
                activeTool = 'annotate';
              }}
              role="button"
              tabindex="0"
              onkeydown={(e) => {
                if (e.key === 'Enter') activeAnnotationId = item.id;
              }}
              class="flex items-center gap-1.5 rounded-lg border p-1.5 transition-colors {activeAnnotationId ===
              item.id
                ? 'border-accent bg-ink-1'
                : 'border-line bg-ink-1/50 hover:border-line/90'}"
            >
              <span
                class="flex h-5 w-6 shrink-0 items-center justify-center rounded font-mono text-[10px] font-black text-ink-0"
                style="background: {item.color};"
              >
                #{item.index}
              </span>
              <input
                type="text"
                bind:value={item.note}
                placeholder={i18n.t('studio_annot_box_ph')}
                onclick={(e) => e.stopPropagation()}
                class="min-w-0 flex-1 rounded border border-transparent bg-transparent px-1.5 py-0.5 text-xs text-fg placeholder:text-fg-muted/50 focus:border-line focus:bg-ink-0 focus:outline-none"
              />
              <button
                type="button"
                onclick={(e) => {
                  e.stopPropagation();
                  deleteAnnotation(item.id);
                }}
                title={i18n.t('studio_annot_clear')}
                class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-fg-muted hover:bg-err/20 hover:text-err"
              >
                <X size={12} />
              </button>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Optional Global Consistency Constraint -->
      <input
        type="text"
        bind:value={regionGlobalNote}
        placeholder={i18n.t('studio_annot_global_ph')}
        class="w-full rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 text-xs text-fg placeholder:text-fg-muted/50 focus:border-accent focus:outline-none"
      />

      <button
        type="button"
        onclick={handleRunRegionEdit}
        disabled={!imgWidth || isRegionEditing || annotations.length === 0}
        class="flex w-full items-center justify-center gap-1.5 rounded-lg bg-accent py-2 text-xs font-extrabold whitespace-nowrap text-accent-ink shadow-sm transition-transform active:scale-98 disabled:opacity-40"
      >
        {#if isRegionEditing}
          <Loader2 size={14} class="animate-spin" />
          <span>{i18n.t('studio_annot_running')}</span>
        {:else}
          <Sparkles size={14} strokeWidth={2.1} />
          <span>
            {i18n.t('studio_annot_run_btn')} ({annotations.filter((a) => a.note.trim()).length || annotations.length})
          </span>
        {/if}
      </button>
    </section>

    <!-- CARD 1: AI Deep Matting & De-Fringe -->
    <section class="flex flex-col gap-2.5 rounded-xl border border-line bg-ink-0/60 p-3">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5 text-xs font-extrabold tracking-wide text-fg">
          <Sparkles size={14} class="text-accent" />
          <span>{i18n.t('studio_matting_title')}</span>
        </div>
        <span
          class="max-w-[140px] truncate rounded bg-accent/15 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-accent"
          title={lastMattingProvider}
        >
          {lastMattingProvider}
        </span>
      </div>

      <!-- Model Selector: Installed Only + Settings Shortcut -->
      <div class="flex flex-col gap-1">
        <div class="flex items-center justify-between text-[11px] font-medium text-fg-muted">
          <span>{i18n.t('studio_matting_model')}</span>
          <button
            type="button"
            onclick={() => appStore.openSettings('matting')}
            class="flex items-center gap-1 text-[10px] text-accent hover:underline"
          >
            <Settings size={11} />
            <span>{i18n.t('studio_matting_manage')}</span>
          </button>
        </div>

        {#if installedMattingModels.length > 0}
          <select
            id="matting-model-sel"
            bind:value={selectedMattingModel}
            class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 text-xs font-semibold text-fg focus:border-accent focus:outline-none"
          >
            {#each installedMattingModels as m (m.id)}
              <option value={m.id}>{i18n.mattingModelName(m.id, m.name)} ({m.sizeMb}MB)</option>
            {/each}
          </select>
        {:else}
          <div class="rounded-lg border border-warn/30 bg-warn/10 p-2.5 text-xs text-warn">
            <p class="font-semibold">{i18n.t('studio_matting_none_title')}</p>
            <p class="mt-1 text-[11px] text-fg-muted">
              {i18n.t('studio_matting_none_sub')}
            </p>
            <button
              type="button"
              onclick={() => appStore.openSettings('matting')}
              class="mt-2 flex items-center gap-1 rounded bg-accent px-2 py-1 text-xs font-bold text-accent-ink"
            >
              <Settings size={12} />
              <span>{i18n.t('studio_matting_go_install')}</span>
            </button>
          </div>
        {/if}
      </div>

      <!-- Execution Device -->
      <div class="flex flex-col gap-1">
        <label for="matting-device-sel" class="text-[11px] font-medium text-fg-muted">
          {i18n.t('studio_matting_device')}
        </label>
        <select
          id="matting-device-sel"
          bind:value={mattingDevice}
          class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 text-xs font-medium text-fg focus:border-accent focus:outline-none"
        >
          <option value="auto">{i18n.t('studio_device_auto')}</option>
          <option value="gpu">{i18n.t('studio_device_gpu')}</option>
          <option value="cpu">{i18n.t('studio_device_cpu')}</option>
        </select>
      </div>

      <!-- Choke & Enhanced Recovery -->
      <div class="flex items-center justify-between pt-0.5">
        <label class="flex cursor-pointer items-center gap-1.5 text-xs whitespace-nowrap text-fg">
          <input
            type="checkbox"
            bind:checked={mattingEnhanced}
            class="h-3.5 w-3.5 rounded accent-accent"
          />
          <span>{i18n.t('studio_matting_enhanced')}</span>
        </label>

        <div class="flex items-center gap-1.5">
          <span class="text-[11px] whitespace-nowrap text-fg-muted"
            >{i18n.t('studio_matting_choke')}</span
          >
          <input
            type="range"
            min="0"
            max="5"
            step="1"
            bind:value={mattingChoke}
            class="w-16 accent-accent"
          />
          <span class="w-5 text-right font-mono text-xs font-bold text-fg">{mattingChoke}px</span>
        </div>
      </div>

      <div
        class="rounded-lg border border-ok/25 bg-ok/10 px-2.5 py-1 text-[10px] font-medium text-ok"
      >
        {i18n.t('studio_matting_unmix_badge')}
      </div>

      <div class="flex gap-1.5 pt-0.5">
        <button
          type="button"
          onclick={handleRunAiMatting}
          disabled={!imgWidth || isMattingBusy || installedMattingModels.length === 0}
          class="flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-accent py-2 text-xs font-extrabold whitespace-nowrap text-accent-ink shadow-sm transition-transform active:scale-98 disabled:opacity-40"
        >
          {#if isMattingBusy}
            <Loader2 size={14} class="animate-spin" />
            <span>{i18n.t('studio_matting_running')}</span>
          {:else}
            <Cpu size={14} strokeWidth={2.1} />
            <span>{i18n.t('studio_matting_run')}</span>
          {/if}
        </button>

        <button
          type="button"
          onclick={handleSolidBgCutout}
          disabled={!imgWidth || isMattingBusy}
          title={i18n.t('studio_solid_cutout_tip')}
          class="shrink-0 rounded-lg border border-line bg-ink-2 px-2.5 py-2 text-xs font-semibold whitespace-nowrap text-fg transition-colors hover:border-accent/50 disabled:opacity-40"
        >
          {i18n.t('studio_matting_solid_btn')}
        </button>
      </div>
    </section>

    <!-- CARD 2: AI Pixel Art Cleaner (Upgraded with Periodicity Detection & Block Slider) -->
    <section class="flex flex-col gap-2.5 rounded-xl border border-line bg-ink-0/60 p-3">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5 text-xs font-extrabold tracking-wide text-fg">
          <Grid3x3 size={14} class="text-accent" />
          <span>{i18n.t('studio_pixel_title')}</span>
        </div>
        {#if lastPixelMeta}
          <span class="font-mono text-[10px] font-bold text-accent">{lastPixelMeta}</span>
        {/if}
      </div>

      <!-- Mode Pill Switcher: Auto Detect vs Manual Block Size -->
      <div class="flex rounded-lg border border-line bg-ink-1 p-0.5 text-xs">
        <button
          type="button"
          onclick={() => (blockSizeMode = 'auto')}
          class="flex-1 rounded-md py-1 text-center font-semibold transition-colors {blockSizeMode ===
          'auto'
            ? 'bg-accent text-accent-ink shadow-sm'
            : 'text-fg-muted hover:text-fg'}"
        >
          {i18n.t('studio_pixel_mode_auto')}
        </button>
        <button
          type="button"
          onclick={() => (blockSizeMode = 'manual')}
          class="flex-1 rounded-md py-1 text-center font-semibold transition-colors {blockSizeMode ===
          'manual'
            ? 'bg-accent text-accent-ink shadow-sm'
            : 'text-fg-muted hover:text-fg'}"
        >
          {i18n.t('studio_pixel_mode_manual')}
        </button>
      </div>

      {#if blockSizeMode === 'manual'}
        <!-- Manual Block Size Slider & Quick Presets -->
        <div class="flex flex-col gap-1.5 rounded-lg border border-line/70 bg-ink-1/60 p-2">
          <div class="flex items-center justify-between text-xs">
            <span class="text-fg-muted">{i18n.t('studio_pixel_block_size')}</span>
            <span class="font-mono font-bold text-accent">{manualBlockSize} px</span>
          </div>
          <input
            type="range"
            min="2"
            max="32"
            step="1"
            bind:value={manualBlockSize}
            class="w-full accent-accent"
          />
          <div class="flex flex-wrap gap-1">
            {#each [3, 4, 6, 8, 10, 12, 16, 24] as b (b)}
              <button
                type="button"
                onclick={() => (manualBlockSize = b)}
                class="rounded border border-line bg-ink-2 px-1.5 py-0.5 font-mono text-[10px] font-bold transition-colors {manualBlockSize ===
                b
                  ? 'border-accent bg-accent text-accent-ink'
                  : 'text-fg-muted hover:text-fg'}"
              >
                {b}px
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <div class="grid grid-cols-2 gap-2">
        <div class="flex flex-col gap-1">
          <label for="pixel-grid-sel" class="text-[11px] font-medium text-fg-muted">
            {i18n.t('studio_pixel_target_grid')}
          </label>
          <select
            id="pixel-grid-sel"
            bind:value={pixelGrid}
            class="rounded-lg border border-line bg-ink-1 px-2 py-1.5 text-xs font-medium text-fg focus:border-accent focus:outline-none"
          >
            <option value="auto">{i18n.t('studio_pixel_follow_auto')}</option>
            <option value="32">32 × 32</option>
            <option value="48">48 × 48</option>
            <option value="64">64 × 64</option>
            <option value="96">96 × 96</option>
            <option value="128">128 × 128</option>
            <option value="160">160 × 160</option>
            <option value="256">256 × 256</option>
          </select>
        </div>

        <div class="flex flex-col gap-1">
          <label for="pixel-pal-sel" class="text-[11px] font-medium text-fg-muted">
            {i18n.t('studio_pixel_palette')}
          </label>
          <select
            id="pixel-pal-sel"
            bind:value={pixelPalette}
            class="rounded-lg border border-line bg-ink-1 px-2 py-1.5 text-xs font-medium text-fg focus:border-accent focus:outline-none"
          >
            <option value="auto">{i18n.t('studio_pal_auto')}</option>
            <option value="16">{i18n.t('studio_pal_16')}</option>
            <option value="32">{i18n.t('studio_pal_32')}</option>
            <option value="64">{i18n.t('studio_pal_64')}</option>
            <option value="pico8">{i18n.t('studio_pal_pico8')}</option>
            <option value="gameboy">{i18n.t('studio_pal_gameboy')}</option>
          </select>
        </div>
      </div>

      <!-- Detail Fidelity Switcher: 1x Standard vs 2x Sub-pixel Detail Preservation -->
      <div class="flex flex-col gap-1">
        <span class="text-[11px] font-medium text-fg-muted">
          {i18n.t('studio_pixel_detail_label')}
        </span>
        <div class="flex rounded-lg border border-line bg-ink-1 p-0.5 text-xs">
          <button
            type="button"
            onclick={() => (pixelDetailMode = 'standard')}
            class="flex-1 rounded-md py-1 text-center font-semibold whitespace-nowrap transition-colors {pixelDetailMode ===
            'standard'
              ? 'bg-ink-2 text-accent shadow-sm'
              : 'text-fg-muted hover:text-fg'}"
          >
            {i18n.t('studio_pixel_detail_1x')}
          </button>
          <button
            type="button"
            onclick={() => (pixelDetailMode = 'fine2x')}
            class="flex-1 rounded-md py-1 text-center font-semibold whitespace-nowrap transition-colors {pixelDetailMode ===
            'fine2x'
              ? 'bg-ink-2 text-accent shadow-sm'
              : 'text-fg-muted hover:text-fg'}"
          >
            {i18n.t('studio_pixel_detail_2x')}
          </button>
        </div>
      </div>

      <!-- Perceptual Color Coalescing Slider (Removes AI intra-color noise) -->
      <div class="flex flex-col gap-1">
        <div class="flex items-center justify-between text-xs">
          <span class="text-fg-muted">
            {i18n.t('studio_pixel_coalesce')}
          </span>
          <span class="font-mono font-bold text-accent">{pixelCoalesce}</span>
        </div>
        <input
          type="range"
          min="0"
          max="45"
          step="1"
          bind:value={pixelCoalesce}
          class="w-full accent-accent"
        />
      </div>

      <div class="flex items-center justify-between gap-2">
        <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg">
          <input
            type="checkbox"
            bind:checked={pixelHardAlpha}
            class="h-3.5 w-3.5 rounded accent-accent"
          />
          <span>{i18n.t('studio_pixel_hard_alpha')}</span>
        </label>

        <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg">
          <input
            type="checkbox"
            bind:checked={showPixelGrid}
            onchange={() => renderDisplayCanvas()}
            class="h-3.5 w-3.5 rounded accent-accent"
          />
          <span>{i18n.t('studio_pixel_show_grid')}</span>
        </label>
      </div>

      <div class="flex gap-1.5">
        <button
          type="button"
          onclick={handleRunPixelClean}
          disabled={!imgWidth}
          class="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-accent/40 bg-accent/15 py-2 text-xs font-bold whitespace-nowrap text-accent transition-colors hover:bg-accent hover:text-accent-ink disabled:opacity-40"
        >
          <Grid3x3 size={13} strokeWidth={2} />
          <span>{i18n.t('studio_pixel_clean_btn')}</span>
        </button>

        <button
          type="button"
          onclick={handleNativePixelExport}
          disabled={!imgWidth}
          class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-2 text-xs font-semibold whitespace-nowrap text-fg transition-colors hover:border-accent/50 disabled:opacity-40"
        >
          <Download size={13} />
          <span>{i18n.t('studio_pixel_native_btn')}</span>
        </button>
      </div>

      <button
        type="button"
        onclick={handleSendToForgeRefine}
        disabled={!imgWidth}
        class="flex w-full items-center justify-center gap-1.5 rounded-lg border border-line bg-ink-1 py-1.5 text-xs font-semibold whitespace-nowrap text-fg-muted transition-colors hover:border-accent/50 hover:text-fg disabled:opacity-40"
      >
        <Sparkles size={12} class="text-accent" />
        <span>{i18n.t('studio_pixel_send_forge')}</span>
      </button>
    </section>

    <!-- CARD 3: Wand Tolerance & Brush Radius -->
    <section class="flex flex-col gap-2.5 rounded-xl border border-line bg-ink-0/60 p-3">
      <div class="flex items-center gap-1.5 text-xs font-extrabold tracking-wide text-fg">
        <Sliders size={14} class="text-accent" />
        <span>{i18n.t('studio_retouch_title')}</span>
      </div>

      <div class="flex flex-col gap-1">
        <div class="flex items-center justify-between text-xs">
          <span class="text-fg-muted">{i18n.t('studio_tol_label')}</span>
          <span class="font-mono font-bold text-fg">{tolerance}</span>
        </div>
        <input
          type="range"
          min="5"
          max="140"
          step="1"
          bind:value={tolerance}
          class="w-full accent-accent"
        />
      </div>

      <div class="flex flex-col gap-1">
        <div class="flex items-center justify-between text-xs">
          <span class="text-fg-muted">{i18n.t('studio_brush_label')}</span>
          <span class="font-mono font-bold text-fg">{brushSize} px</span>
        </div>
        <input
          type="range"
          min="2"
          max="120"
          step="1"
          bind:value={brushSize}
          class="w-full accent-accent"
        />
      </div>
    </section>

    <!-- CARD 4: Custom Resolution Lossless Export -->
    <section class="flex flex-col gap-2.5 rounded-xl border border-line bg-ink-0/60 p-3">
      <div class="flex items-center gap-1.5 text-xs font-extrabold tracking-wide text-fg">
        <Share2 size={14} class="text-accent" />
        <span>{i18n.t('studio_export_title')}</span>
      </div>

      <div class="grid grid-cols-2 gap-2">
        <div class="flex flex-col gap-1">
          <label for="export-w-in" class="text-[11px] font-medium text-fg-muted">
            {i18n.t('studio_export_w')}
          </label>
          <input
            id="export-w-in"
            type="number"
            min="1"
            max="8192"
            bind:value={exportW}
            class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-xs text-fg focus:border-accent focus:outline-none"
          />
        </div>
        <div class="flex flex-col gap-1">
          <label for="export-h-in" class="text-[11px] font-medium text-fg-muted">
            {i18n.t('studio_export_h')}
          </label>
          <input
            id="export-h-in"
            type="number"
            min="1"
            max="8192"
            bind:value={exportH}
            class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-xs text-fg focus:border-accent focus:outline-none"
          />
        </div>
      </div>

      <!-- Quick Resolution Preset Chips -->
      <div class="flex flex-wrap gap-1">
        {#each [64, 128, 256, 512, 1024] as dim (dim)}
          <button
            type="button"
            onclick={() => {
              exportW = dim;
              exportH = dim;
            }}
            class="rounded border border-line bg-ink-1 px-2 py-0.5 font-mono text-[10px] font-semibold whitespace-nowrap text-fg-muted transition-colors hover:border-accent hover:text-accent"
          >
            {dim}×{dim}
          </button>
        {/each}
        {#if imgWidth > 0}
          <button
            type="button"
            onclick={() => {
              exportW = imgWidth;
              exportH = imgHeight;
            }}
            class="rounded border border-line bg-ink-1 px-2 py-0.5 font-mono text-[10px] font-semibold whitespace-nowrap text-fg-muted transition-colors hover:border-accent hover:text-accent"
          >
            {i18n.locale === 'en' ? 'Original' : '原尺寸'}
          </button>
        {/if}
      </div>

      <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg">
        <input
          type="checkbox"
          bind:checked={exportKeepRatio}
          class="h-3.5 w-3.5 rounded accent-accent"
        />
        <span>{i18n.t('studio_export_keep_ratio')}</span>
      </label>

      <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg">
        <input
          type="checkbox"
          bind:checked={exportNearest}
          class="h-3.5 w-3.5 rounded accent-accent"
        />
        <span>{i18n.t('studio_export_pixel_nn')}</span>
      </label>

      <button
        type="button"
        onclick={handleCustomSizeExport}
        disabled={!imgWidth}
        class="flex items-center justify-center gap-1.5 rounded-lg border border-line bg-ink-2 py-2 text-xs font-bold whitespace-nowrap text-fg transition-colors hover:border-accent hover:bg-accent hover:text-accent-ink disabled:opacity-40"
      >
        <Download size={13} strokeWidth={2} />
        <span>{i18n.t('studio_export_btn')}</span>
      </button>
    </section>
  </aside>
</div>
