<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    Layers,
    FolderOpen,
    Download,
    CheckCircle2,
    AlertTriangle,
    Cpu,
    HardDrive,
    Eye,
    EyeOff,
    ArrowUp,
    ArrowDown,
    Sparkles,
    Scissors,
    FileOutput,
    PackageOpen,
    RefreshCw,
    Trash2,
    Settings2,
    ZoomIn,
    ZoomOut,
    RotateCcw,
    Loader2,
    Upload,
    Wand2,
    History,
    X,
    Square,
    FlipHorizontal2,
    Layers2,
    Move,
    Hand,
    ArrowLeft,
    ArrowRight,
    GripVertical
  } from '@lucide/svelte';
  import { api, assetUrl } from '$lib/api';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n } from '$lib/i18n/index.svelte';
  import type {
    AssetItem,
    DecomposeEnvStatus,
    DecomposeHistoryItem,
    DecomposeJobResult,
    DecomposeLayerInfo
  } from '$lib/api/types';

  const LAYER_ZH_MAP: Record<string, string> = {
    'front hair': '前发 / 刘海',
    'back hair': '后发 / 披发',
    headwear: '头饰',
    face: '脸部皮肤',
    eyebrow: '眉毛',
    'eyebrow-l': '左眉',
    'eyebrow-r': '右眉',
    eyelash: '眼睫 / 眼眶',
    'eyelash-l': '左睫毛',
    'eyelash-r': '右睫毛',
    irides: '虹膜 / 瞳孔',
    'irides-l': '左瞳孔',
    'irides-r': '右瞳孔',
    eyewhite: '眼白',
    'eyewhite-l': '左眼白',
    'eyewhite-r': '右眼白',
    eyewear: '眼镜 / 眼饰',
    ears: '耳朵',
    'ears-l': '左耳',
    'ears-r': '右耳',
    earwear: '耳饰',
    nose: '鼻子',
    mouth: '嘴巴',
    neck: '颈部',
    neckwear: '颈饰 / 领结',
    topwear: '上装',
    handwear: '手臂 / 手套',
    'handwear-l': '左臂 / 手饰',
    'handwear-r': '右臂 / 手饰',
    bottomwear: '下装',
    legwear: '腿部 / 袜饰',
    'legwear-l': '左腿 / 袜饰',
    'legwear-r': '右腿 / 袜饰',
    footwear: '鞋履',
    'footwear-l': '左鞋履',
    'footwear-r': '右鞋履',
    tail: '尾巴',
    wings: '翅膀',
    objects: '手持道具'
  };

  function layerDisplayName(name: string): string {
    const key = name.trim().toLowerCase();
    const direct = LAYER_ZH_MAP[key];
    if (isEn) return name;
    if (direct) return `${direct} (${name})`;
    for (const [suf, label] of [
      ['-front', '前景'],
      ['-back', '后景'],
      ['-l', '左侧'],
      ['-r', '右侧']
    ] as const) {
      if (key.endsWith(suf)) {
        const stem = key.slice(0, -suf.length);
        const stemZh = LAYER_ZH_MAP[stem] ?? stem;
        return `${stemZh}·${label} (${name})`;
      }
    }
    return name;
  }

  let isEn = $derived(i18n.locale === 'en');

  // Environment & Extension setup state
  let envStatus = $state<DecomposeEnvStatus | null>(null);
  let loadingEnv = $state(false);
  let showSetupPanel = $state(false);
  let targetDirInput = $state('');
  let selectedQuant = $state<'nf4' | 'bf16'>('nf4');
  let selectedMirror = $state<'hf-mirror' | 'huggingface'>('hf-mirror');
  let selectedPypiMirror = $state<string>('https://pypi.tuna.tsinghua.edu.cn/simple');
  let uninstallConfirm = $state(false);

  // Install progress
  let isInstalling = $state(false);
  let installStage = $state('');
  let installPercent = $state(0);
  let installDownloadedMb = $state(0);
  let installTotalMb = $state(0);
  let installSpeedMbS = $state(0);
  let installMessage = $state('');

  // Run state
  let inputPreviewSrc = $state('');
  let inputFilePath = $state('');
  let runPreset = $state<'896_16' | '1024_20' | '1280_30'>('1024_20');
  let runResolution = $derived(runPreset === '896_16' ? 896 : runPreset === '1280_30' ? 1280 : 1024);
  let runSteps = $derived(runPreset === '896_16' ? 16 : runPreset === '1280_30' ? 30 : 20);
  let runTblrSplit = $state(true);
  let runAutoRemoveBg = $state(true);
  let runAutoGroupOffload = $state(true);
  let runCoolMode = $state(true);
  let runCpuOffload = $state(false);
  let isRunning = $state(false);
  let runStage = $state('');
  let runPercent = $state(0);
  let runMessage = $state('');
  let runElapsedSec = $state(0);
  let runTimerId: ReturnType<typeof setInterval> | null = null;

  // Active decompose workspace state
  let currentJob = $state<DecomposeJobResult | null>(null);
  let layers = $state<DecomposeLayerInfo[]>([]);
  let selectedLayerId = $state<string | null>(null);
  let layerCacheBust = $state<Record<string, number>>({});
  let soloMode = $state(false);
  let viewDepthMap = $state(false);
  let backdropMode = $state<'checker' | 'dark' | 'light' | 'magenta'>('checker');
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let isPanning = $state(false);
  let panStart = { x: 0, y: 0, ox: 0, oy: 0 };
  let interactionMode = $state<'pan' | 'move'>('move');
  type GizmoAxis = 'x' | 'y' | 'xy';
  let moveAxis = $state<GizmoAxis | null>(null);
  let hoveredAxis = $state<GizmoAxis | null>(null);
  let layerMoveStep = $state<1 | 5 | 20>(5);
  let isMovingLayer = $state(false);
  let moveLayerStart = { x: 0, y: 0, ox: 0, oy: 0 };
  let draggingLayerId = $state<string | null>(null);
  let dragOverLayerId = $state<string | null>(null);
  let persistTimerId: ReturnType<typeof setTimeout> | null = null;
  let isExporting = $state(false);
  let defringeBusy = $state(false);
  let splitBusy = $state<'lr' | 'depth' | null>(null);

  let hasAnyLayerOffset = $derived(
    layers.some((l) => (l.offsetX ?? 0) !== 0 || (l.offsetY ?? 0) !== 0)
  );

  let stageDisplay = $derived.by(() => {
    if (!currentJob || currentJob.width <= 0 || currentJob.height <= 0) {
      return { w: 680, h: 680 };
    }
    const maxDim = 680;
    const scale = Math.min(1, maxDim / Math.max(currentJob.width, currentJob.height));
    return {
      w: Math.round(currentJob.width * scale),
      h: Math.round(currentJob.height * scale)
    };
  });

  // History & Filmstrip assets
  let historyList = $state<DecomposeHistoryItem[]>([]);
  let libraryAssets = $state<AssetItem[]>([]);
  let filmstripProjectFilter = $state<string>('_all');
  let filmstripProjectInitialized = false;

  let fileInputEl = $state<HTMLInputElement | null>(null);
  let unlistenInstall: UnlistenFn | null = null;
  let unlistenRun: UnlistenFn | null = null;
  let unlistenMcp: UnlistenFn | null = null;
  let activeMcpTaskId = $state<string | null>(null);

  let selectedLayer = $derived(
    layers.find((l) => l.id === selectedLayerId) ?? null
  );

  let selectedGizmo = $derived.by(() => {
    if (!currentJob || !selectedLayer) return null;
    const w = Math.max(1, currentJob.width);
    const h = Math.max(1, currentJob.height);
    const ox = selectedLayer.offsetX ?? 0;
    const oy = selectedLayer.offsetY ?? 0;
    const hasBbox =
      selectedLayer.right > selectedLayer.left && selectedLayer.bottom > selectedLayer.top;
    const left = (hasBbox ? selectedLayer.left : 0) + ox;
    const right = (hasBbox ? selectedLayer.right : w) + ox;
    const top = (hasBbox ? selectedLayer.top : 0) + oy;
    const bottom = (hasBbox ? selectedLayer.bottom : h) + oy;
    return {
      leftPct: (left / w) * 100,
      topPct: (top / h) * 100,
      widthPct: ((right - left) / w) * 100,
      heightPct: ((bottom - top) / h) * 100,
      cxPct: ((left + right) / 2 / w) * 100,
      cyPct: ((top + bottom) / 2 / h) * 100,
      ox,
      oy
    };
  });

  let gizmoXColor = $derived(
    moveAxis === 'x' || (!moveAxis && hoveredAxis === 'x') ? '#fde047' : '#ef4444'
  );
  let gizmoYColor = $derived(
    moveAxis === 'y' || (!moveAxis && hoveredAxis === 'y') ? '#fde047' : '#65e643'
  );
  let gizmoXyActive = $derived(
    moveAxis === 'xy' || (!moveAxis && hoveredAxis === 'xy')
  );

  // Display top-most layer first in tree (standard Photoshop / Live2D order), while compositing renders bottom-up
  let layersTopDown = $derived(
    [...layers].sort((a, b) => b.zIndex - a.zIndex)
  );
  let layersBottomUp = $derived(
    [...layers].sort((a, b) => a.zIndex - b.zIndex)
  );

  async function refreshEnvStatus() {
    loadingEnv = true;
    try {
      const s = await api.getDecomposeStatus();
      envStatus = s;
      targetDirInput = s.configuredDir || s.recommendedDir;
      selectedQuant = s.quantMode === 'bf16' ? 'bf16' : 'nf4';
      selectedMirror = s.mirror === 'huggingface' ? 'huggingface' : 'hf-mirror';
      if (s.vramTotalMb > 0 && s.vramTotalMb < 7800) {
        runCpuOffload = true;
      }
      if (!s.ready) {
        showSetupPanel = true;
      }
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to inspect decompose environment');
    } finally {
      loadingEnv = false;
    }
  }

  async function refreshHistoryAndAssets() {
    try {
      const proj = filmstripProjectFilter === '_all' ? undefined : filmstripProjectFilter;
      const [hist, assets] = await Promise.all([
        api.listDecomposeHistory(),
        api.listAssets(proj, 60)
      ]);
      historyList = hist;
      libraryAssets = assets;
    } catch {
      // ignore non-fatal filmstrip errors
    }
  }

  onMount(async () => {
    unlistenInstall = await listen<{
      stage: string;
      percent: number;
      downloadedMb: number;
      totalMb: number;
      speedMbS: number;
      message: string;
    }>('decompose-install-progress', (ev) => {
      installStage = ev.payload.stage;
      installPercent = ev.payload.percent;
      installDownloadedMb = ev.payload.downloadedMb;
      installTotalMb = ev.payload.totalMb;
      installSpeedMbS = ev.payload.speedMbS;
      installMessage = ev.payload.message;
    });

    unlistenRun = await listen<{
      stage: string;
      percent: number;
      message: string;
    }>('decompose-run-progress', (ev) => {
      runStage = ev.payload.stage;
      runPercent = ev.payload.percent;
      runMessage = ev.payload.message;
    });

    unlistenMcp = await listen<any>('mcp-ui-event', (ev) => {
      const p = ev.payload ?? {};
      if (p.kind === 'decompose_started') {
        showSetupPanel = false;
        if (p.inputPath) {
          inputFilePath = p.inputPath;
          inputPreviewSrc = assetUrl(p.inputPath);
        }
        activeMcpTaskId = p.taskId || null;
        isRunning = true;
        runPercent = 2;
        runElapsedSec = 0;
        runMessage = isEn
          ? 'MCP Agent started 23-layer decompose...'
          : 'AI Agent 已启动 23 层智能拆图...';
        if (runTimerId) clearInterval(runTimerId);
        const tStart = Date.now();
        runTimerId = setInterval(() => {
          runElapsedSec = Math.floor((Date.now() - tStart) / 1000);
        }, 500);
      } else if (p.kind === 'decompose_completed') {
        if (runTimerId) {
          clearInterval(runTimerId);
          runTimerId = null;
        }
        isRunning = false;
        activeMcpTaskId = null;
        if (p.job) {
          currentJob = p.job;
          layers = p.job.layers ?? [];
          selectedLayerId = layers[layers.length - 1]?.id ?? null;
          soloMode = false;
          viewDepthMap = false;
          interactionMode = 'move';
          zoom = 1;
          panX = 0;
          panY = 0;
        }
        void Promise.all([refreshHistoryAndAssets(), refreshEnvStatus()]);
        appStore.toast(
          'ok',
          isEn
            ? 'MCP character decompose complete — layers loaded'
            : 'AI Agent 拆图完成，已自动载入图层画布'
        );
      } else if (p.kind === 'decompose_cancelled') {
        if (runTimerId) {
          clearInterval(runTimerId);
          runTimerId = null;
        }
        isRunning = false;
        activeMcpTaskId = null;
        runMessage = '';
        void refreshEnvStatus();
        appStore.toast(
          'info',
          isEn
            ? 'MCP decompose stopped & VRAM released'
            : '已手动中止 AI Agent 拆图任务并释放显存'
        );
      } else if (p.kind === 'decompose_failed') {
        if (runTimerId) {
          clearInterval(runTimerId);
          runTimerId = null;
        }
        isRunning = false;
        activeMcpTaskId = null;
        appStore.toast('err', p.error || 'MCP decompose failed');
      }
    });

    if (appStore.settings?.activeProject) {
      filmstripProjectFilter = appStore.settings.activeProject;
      filmstripProjectInitialized = true;
    }
    if (appStore.projects.length === 0) {
      void appStore.refreshProjects();
    }
    await Promise.all([refreshEnvStatus(), refreshHistoryAndAssets()]);
  });

  $effect(() => {
    const ap = appStore.settings?.activeProject;
    if (!filmstripProjectInitialized && ap) {
      filmstripProjectFilter = ap;
      filmstripProjectInitialized = true;
      void refreshHistoryAndAssets();
    }
  });

  function handleWindowKeyDown(e: KeyboardEvent) {
    if (isRunning || showSetupPanel) return;
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    if (!e.ctrlKey && !e.metaKey && !e.altKey) {
      if (e.key === 'q' || e.key === 'Q') {
        interactionMode = 'pan';
        return;
      }
      if (e.key === 'w' || e.key === 'W') {
        interactionMode = 'move';
        return;
      }
    }
    if (!currentJob || !selectedLayer) return;
    const step = e.shiftKey ? 10 : 1;
    let dx = 0;
    let dy = 0;
    if (e.key === 'ArrowLeft') dx = -step;
    else if (e.key === 'ArrowRight') dx = step;
    else if (e.key === 'ArrowUp') dy = -step;
    else if (e.key === 'ArrowDown') dy = step;
    else return;
    e.preventDefault();
    nudgeLayerOffset(selectedLayer.id, dx, dy);
  }

  onDestroy(() => {
    if (unlistenInstall) unlistenInstall();
    if (unlistenRun) unlistenRun();
    if (unlistenMcp) unlistenMcp();
    if (runTimerId) clearInterval(runTimerId);
    if (persistTimerId) clearTimeout(persistTimerId);
  });

  // React to assets sent via "Send to Layer Split"
  $effect(() => {
    const pending = appStore.decomposePendingAsset;
    if (pending) {
      inputFilePath = pending.filePath;
      inputPreviewSrc = assetUrl(pending.filePath);
      appStore.decomposePendingAsset = null;
    }
  });

  $effect(() => {
    void appStore.assetsRevision;
    void appStore.settings?.activeProject;
    void refreshHistoryAndAssets();
  });

  async function handlePickInstallFolder() {
    const picked = await api.pickDecomposeDir(targetDirInput || undefined);
    if (!picked) return;
    targetDirInput = picked;
    try {
      const updated = await api.setDecomposeDir(picked);
      envStatus = updated;
      if (updated.ready) {
        appStore.toast(
          'ok',
          isEn
            ? 'Detected existing See-Through runtime & models in selected directory!'
            : '已自动识别选定目录中的现有拆图环境与模型权重，可直接使用！'
        );
        showSetupPanel = false;
      }
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to set directory');
    }
  }

  async function handleSaveTypedDir() {
    if (!targetDirInput.trim()) return;
    try {
      const updated = await api.setDecomposeDir(targetDirInput.trim());
      envStatus = updated;
      if (updated.ready) {
        appStore.toast(
          'ok',
          isEn ? 'Existing installation linked!' : '已成功关联该目录下的拆图扩展与权重！'
        );
        showSetupPanel = false;
      }
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Invalid directory');
    }
  }

  async function handleStartInstall() {
    if (!targetDirInput.trim() || isInstalling) return;
    isInstalling = true;
    installPercent = 1;
    installMessage = isEn ? 'Preparing installation...' : '正在准备安装环境与磁盘校验...';
    try {
      const updated = await api.installDecomposeExtension(
        targetDirInput.trim(),
        selectedQuant,
        selectedMirror,
        selectedMirror === 'huggingface' ? 'https://pypi.org/simple' : selectedPypiMirror
      );
      envStatus = updated;
      appStore.toast(
        'ok',
        isEn
          ? 'Layer Decompose Workshop installed and ready!'
          : '拆图工坊独立扩展包安装完成，现在可以开始拆解立绘！'
      );
      showSetupPanel = false;
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Installation failed or cancelled');
      await refreshEnvStatus();
    } finally {
      isInstalling = false;
    }
  }

  async function handleCancelInstall() {
    await api.cancelDecomposeInstall();
    appStore.toast('info', isEn ? 'Cancelling download...' : '正在取消下载...');
  }

  async function handleUninstall() {
    try {
      const updated = await api.uninstallDecomposeExtension();
      envStatus = updated;
      uninstallConfirm = false;
      appStore.toast('ok', isEn ? 'Extension uninstalled & disk space freed' : '已卸载扩展并释放磁盘空间');
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Uninstall failed');
    }
  }

  function handleSelectLocalFile(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        inputFilePath = reader.result;
        inputPreviewSrc = reader.result;
      }
    };
    reader.readAsDataURL(file);
    input.value = '';
  }

  function selectAssetForDecompose(asset: AssetItem) {
    inputFilePath = asset.filePath;
    inputPreviewSrc = assetUrl(asset.filePath);
  }

  async function handleRunDecompose() {
    if (!inputFilePath || isRunning) return;
    isRunning = true;
    runPercent = 2;
    runElapsedSec = 0;
    runMessage = isEn ? 'Releasing idle VRAM & launching See-Through...' : '正在回收预抠图显存并启动拆图引擎...';
    if (runTimerId) clearInterval(runTimerId);
    const tStart = Date.now();
    runTimerId = setInterval(() => {
      runElapsedSec = Math.floor((Date.now() - tStart) / 1000);
    }, 500);
    try {
      const res = await api.runDecomposeCharacter({
        inputPath: inputFilePath,
        project: appStore.settings?.activeProject || undefined,
        resolution: runResolution,
        steps: runSteps,
        quantMode: selectedQuant,
        tblrSplit: runTblrSplit,
        autoRemoveBg: runAutoRemoveBg,
        cpuOffload: runCpuOffload,
        groupOffload: runAutoGroupOffload ? 'auto' : 'off',
        coolMs: runCoolMode ? 60 : 0
      });
      currentJob = res;
      layers = res.layers;
      selectedLayerId = res.layers[res.layers.length - 1]?.id ?? null;
      soloMode = false;
      viewDepthMap = false;
      zoom = 1;
      panX = 0;
      panY = 0;
      appStore.notifyAssetsChanged();
      await Promise.all([refreshHistoryAndAssets(), refreshEnvStatus()]);
      appStore.toast(
        'ok',
        isEn
          ? `Decomposed ${res.layers.length} layers in ${res.elapsedSec}s (VRAM ${res.peakVramGb} GB)`
          : `拆图完成！共分离 ${res.layers.length} 个语义图层（耗时 ${res.elapsedSec}s · 峰值显存 ${res.peakVramGb} GB）`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Decompose failed');
      await refreshEnvStatus();
    } finally {
      if (runTimerId) {
        clearInterval(runTimerId);
        runTimerId = null;
      }
      isRunning = false;
    }
  }

  async function handleCancelDecomposeRun() {
    if (!isRunning) return;
    runMessage = isEn ? 'Stopping GPU worker and reclaiming VRAM...' : '正在紧急中止推理进程并释放显存...';
    try {
      if (activeMcpTaskId) {
        await api.cancelMcpTask(activeMcpTaskId);
      }
      await api.cancelDecomposeRun();
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Cancel failed');
    }
  }

  function schedulePersistLayers() {
    if (!currentJob) return;
    const mp = currentJob.manifestPath;
    const snapshot = layers;
    if (persistTimerId) clearTimeout(persistTimerId);
    persistTimerId = setTimeout(() => {
      void api.saveDecomposeLayers(mp, snapshot).catch(() => {});
    }, 250);
  }

  async function handleSplitSelectedLayer(mode: 'lr' | 'depth') {
    if (!currentJob || !selectedLayer || splitBusy) return;
    splitBusy = mode;
    if (persistTimerId) {
      clearTimeout(persistTimerId);
      persistTimerId = null;
    }
    const oldIds = new Set(layers.map((l) => l.id));
    try {
      const updated = await api.splitDecomposeLayer(
        currentJob.manifestPath,
        layers,
        selectedLayer.id,
        mode
      );
      currentJob = updated;
      layers = updated.layers;
      const nextBust: Record<string, number> = { ...layerCacheBust };
      const now = Date.now();
      for (const l of updated.layers) {
        nextBust[l.id] = now;
      }
      layerCacheBust = nextBust;
      const firstNew = updated.layers.find((l) => !oldIds.has(l.id));
      selectedLayerId = firstNew?.id ?? updated.layers[updated.layers.length - 1]?.id ?? null;
      appStore.toast(
        'ok',
        mode === 'lr'
          ? isEn
            ? 'Split into left/right layers'
            : '已完成当前图层的左右拆分'
          : isEn
            ? 'Split into front/back layers by depth'
            : '已按伪深度前后二拆完成'
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Split layer failed');
    } finally {
      splitBusy = null;
    }
  }

  async function handleLoadHistoryJob(item: DecomposeHistoryItem) {
    try {
      const job = await api.loadDecomposeJob(item.manifestPath);
      currentJob = job;
      layers = job.layers;
      selectedLayerId = job.layers[job.layers.length - 1]?.id ?? null;
      soloMode = false;
      viewDepthMap = false;
      zoom = 1;
      panX = 0;
      panY = 0;
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to load job');
    }
  }

  function toggleLayerVisibility(id: string) {
    layers = layers.map((l) => (l.id === id ? { ...l, visible: !l.visible } : l));
    schedulePersistLayers();
  }

  function setLayerOpacity(id: string, val: number) {
    layers = layers.map((l) => (l.id === id ? { ...l, opacity: val } : l));
    schedulePersistLayers();
  }

  function moveLayerOrder(id: string, delta: number) {
    // Sorted bottom-up by zIndex; positive delta moves visually UP (higher zIndex)
    const sorted = [...layers].sort((a, b) => a.zIndex - b.zIndex);
    const idx = sorted.findIndex((l) => l.id === id);
    if (idx < 0) return;
    const swapIdx = idx + delta;
    if (swapIdx < 0 || swapIdx >= sorted.length) return;
    const cur = sorted[idx];
    const sw = sorted[swapIdx];
    if (!cur || !sw) return;
    sorted[idx] = sw;
    sorted[swapIdx] = cur;
    layers = sorted.map((item, i) => ({ ...item, zIndex: i }));
    schedulePersistLayers();
  }

  function reorderLayerDrop(sourceId: string, targetId: string) {
    if (sourceId === targetId) return;
    // Right panel renders top-down (descending zIndex)
    const topDown = [...layers].sort((a, b) => b.zIndex - a.zIndex);
    const fromIdx = topDown.findIndex((l) => l.id === sourceId);
    const toIdx = topDown.findIndex((l) => l.id === targetId);
    if (fromIdx < 0 || toIdx < 0) return;
    const [moved] = topDown.splice(fromIdx, 1);
    if (!moved) return;
    topDown.splice(toIdx, 0, moved);
    const total = topDown.length;
    layers = topDown.map((item, i) => ({ ...item, zIndex: total - 1 - i }));
    schedulePersistLayers();
  }

  function setLayerOffset(id: string, ox: number, oy: number) {
    const cx = Math.round(ox);
    const cy = Math.round(oy);
    layers = layers.map((l) => (l.id === id ? { ...l, offsetX: cx, offsetY: cy } : l));
    schedulePersistLayers();
  }

  function nudgeLayerOffset(id: string, dx: number, dy: number) {
    const target = layers.find((l) => l.id === id);
    if (!target) return;
    setLayerOffset(id, (target.offsetX ?? 0) + dx, (target.offsetY ?? 0) + dy);
  }

  function resetAllLayerOffsets() {
    layers = layers.map((l) => ({ ...l, offsetX: 0, offsetY: 0 }));
    schedulePersistLayers();
    appStore.toast('info', isEn ? 'All layer offsets reset to (0, 0)' : '已将全部图层归位至原始坐标');
  }

  async function handleDefringeSelected() {
    if (!selectedLayer || defringeBusy) return;
    defringeBusy = true;
    try {
      await api.defringeDecomposeLayer(selectedLayer.imagePath, 1);
      layerCacheBust = { ...layerCacheBust, [selectedLayer.id]: Date.now() };
      appStore.toast('ok', isEn ? 'Layer edge de-fringed' : '已完成当前图层边缘去白边净化');
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Defringe failed');
    } finally {
      defringeBusy = false;
    }
  }

  function handleSendLayerToStudio() {
    if (!selectedLayer || !currentJob) return;
    appStore.openInStudio({
      id: `${currentJob.jobId}_${selectedLayer.id}`,
      project: currentJob.project || '',
      archetype: 'character_card',
      prompt: selectedLayer.name,
      filePath: selectedLayer.imagePath,
      model: 'see-through',
      via: 'see-through',
      width: currentJob.width,
      height: currentJob.height,
      createdAt: Date.now()
    });
  }

  async function handleExportPsd() {
    if (!currentJob || isExporting) return;
    isExporting = true;
    try {
      const psdOut = await api.exportDecomposePsd(currentJob.manifestPath, layers, undefined, true);
      appStore.toast(
        'ok',
        isEn ? `PSD saved: ${psdOut}` : `分层 PSD 已按当前顺序保存并定位：${psdOut}`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Export PSD failed');
    } finally {
      isExporting = false;
    }
  }

  async function handleExportPartsBundle() {
    if (!currentJob || isExporting) return;
    const dir = await api.pickExportDir(currentJob.jobDir);
    if (!dir) return;
    isExporting = true;
    try {
      const outDir = await api.exportDecomposeBundle(currentJob.manifestPath, dir, layers, true);
      appStore.toast(
        'ok',
        isEn ? `Parts & layers.json exported to ${outDir}` : `散件 PNG 与 layers.json 已导出至 ${outDir}`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Export parts failed');
    } finally {
      isExporting = false;
    }
  }

  function layerImgSrc(layer: DecomposeLayerInfo, depth = false): string {
    const p = depth && layer.depthPath ? layer.depthPath : layer.imagePath;
    const base = assetUrl(p);
    const bust = layerCacheBust[layer.id];
    return bust ? `${base}?t=${bust}` : base;
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.12 : 0.89;
    zoom = Math.max(0.25, Math.min(6, +(zoom * factor).toFixed(2)));
  }

  function startGizmoDrag(e: MouseEvent, axis: GizmoAxis) {
    if (e.button !== 0 || !selectedLayer || !currentJob) return;
    e.stopPropagation();
    e.preventDefault();
    isMovingLayer = true;
    moveAxis = axis;
    moveLayerStart = {
      x: e.clientX,
      y: e.clientY,
      ox: selectedLayer.offsetX ?? 0,
      oy: selectedLayer.offsetY ?? 0
    };
  }

  function handleMouseDown(e: MouseEvent) {
    if (e.button !== 0 && e.button !== 1) return;
    if (e.button === 1) e.preventDefault();
    isPanning = true;
    panStart = { x: e.clientX, y: e.clientY, ox: panX, oy: panY };
  }

  function handleMouseMove(e: MouseEvent) {
    if (isMovingLayer && moveAxis && selectedLayer && currentJob) {
      const scaleX = stageDisplay.w * zoom > 0 ? currentJob.width / (stageDisplay.w * zoom) : 1;
      const scaleY = stageDisplay.h * zoom > 0 ? currentJob.height / (stageDisplay.h * zoom) : 1;
      const dx = moveAxis === 'y' ? 0 : Math.round((e.clientX - moveLayerStart.x) * scaleX);
      const dy = moveAxis === 'x' ? 0 : Math.round((e.clientY - moveLayerStart.y) * scaleY);
      const nx = moveLayerStart.ox + dx;
      const ny = moveLayerStart.oy + dy;
      layers = layers.map((l) =>
        l.id === selectedLayer!.id ? { ...l, offsetX: nx, offsetY: ny } : l
      );
      return;
    }
    if (!isPanning) return;
    panX = panStart.ox + (e.clientX - panStart.x);
    panY = panStart.oy + (e.clientY - panStart.y);
  }

  function handleMouseUp() {
    if (isMovingLayer) {
      isMovingLayer = false;
      moveAxis = null;
      schedulePersistLayers();
    }
    isPanning = false;
  }

  function resetView() {
    zoom = 1;
    panX = 0;
    panY = 0;
  }
</script>

<svelte:window
  onkeydown={handleWindowKeyDown}
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
/>

<div class="flex h-full w-full flex-col overflow-hidden bg-ink-0">
  <!-- Top Subheader Status Bar -->
  <div class="flex h-11 shrink-0 items-center justify-between border-b border-line bg-ink-1 px-4">
    <div class="flex items-center gap-2.5">
      <Layers size={15} class="text-accent" />
      <span class="text-xs font-bold tracking-wide text-fg">
        {isEn ? 'Layer Decompose Workshop · See-Through 2.5D' : '拆图工坊 · 2.5D 立绘语义分层与遮挡补全'}
      </span>
      {#if envStatus}
        {#if envStatus.ready}
          <span class="flex items-center gap-1 rounded-full bg-ok/15 px-2 py-0.5 text-[10px] font-bold text-ok">
            <CheckCircle2 size={11} />
            {isEn ? 'Standalone Ready' : '独立引擎已就绪'} ({envStatus.quantMode.toUpperCase()})
          </span>
        {:else}
          <span class="flex items-center gap-1 rounded-full bg-warn/15 px-2 py-0.5 text-[10px] font-bold text-warn">
            <AlertTriangle size={11} />
            {isEn ? 'Extension Not Installed' : '待安装独立拆图扩展'}
          </span>
        {/if}
        {#if envStatus.gpuName}
          <button
            type="button"
            onclick={refreshEnvStatus}
            title={isEn ? 'Refresh Free VRAM' : '点击刷新实时空闲显存'}
            class="hidden items-center gap-1 rounded-md border px-2 py-0.5 font-mono text-[10px] transition-colors md:inline-flex {envStatus.vramFreeMb > 0 &&
            envStatus.vramFreeMb < 8200
              ? 'border-warn/50 bg-warn/10 text-warn'
              : 'border-line bg-ink-2 text-fg-muted hover:border-accent'}"
          >
            <span>
              {envStatus.gpuName} · {isEn ? 'Free' : '空闲'} {(envStatus.vramFreeMb / 1024).toFixed(1)}/{(envStatus.vramTotalMb / 1024).toFixed(1)} GB
            </span>
            <RefreshCw size={10} class={loadingEnv ? 'animate-spin' : ''} />
          </button>
        {/if}
      {/if}
    </div>

    <div class="flex items-center gap-2">
      <button
        type="button"
        onclick={() => (showSetupPanel = !showSetupPanel)}
        class="flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-[11px] font-semibold transition-colors {showSetupPanel
          ? 'border-accent bg-accent/15 text-accent'
          : 'border-line bg-ink-2 text-fg-muted hover:border-accent hover:text-fg'}"
      >
        <Settings2 size={13} />
        <span>{isEn ? 'Extension & Folder' : '扩展管理与目录'}</span>
      </button>
    </div>
  </div>

  {#if showSetupPanel || !envStatus?.ready}
    <!-- Extension Setup / Folder Picker / Size Disclosure Panel -->
    <div class="flex flex-1 items-start justify-center overflow-y-auto p-6">
      <div class="flex w-full max-w-3xl flex-col gap-5 rounded-2xl border border-line bg-ink-1 p-6 shadow-2xl">
        <div class="flex items-start justify-between gap-4">
          <div>
            <h2 class="text-base font-bold text-fg">
              {isEn
                ? 'See-Through Independent Extension Setup'
                : '配置「拆图工坊」独立扩展（零 ComfyUI 依赖 · 按需安装）'}
            </h2>
            <p class="mt-1 text-xs leading-relaxed text-fg-muted">
              {isEn
                ? 'Keeps Sketch2Asset installer slim (~25 MB). Python CUDA runtime & quantized models stay isolated in any folder you choose.'
                : '主程序保持极简轻量；仅在使用拆图功能时，将独立的 64 位 PyTorch CUDA 运行时与模型权重安装到您指定的文件夹中，绝不偷偷占用 C 盘。'}
            </p>
          </div>
          {#if envStatus?.ready}
            <button
              type="button"
              onclick={() => (showSetupPanel = false)}
              class="rounded-lg border border-line bg-ink-2 p-1.5 text-fg-muted hover:text-fg"
              title={isEn ? 'Back to Workspace' : '返回拆图工作台'}
            >
              <X size={16} />
            </button>
          {/if}
        </div>

        <!-- Hardware & Disk Preflight Cards -->
        <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
          <div class="rounded-xl border border-line bg-ink-2/60 p-3.5">
            <div class="flex items-center gap-2 text-xs font-bold text-fg">
              <Cpu size={14} class="text-accent" />
              <span>{isEn ? 'GPU & VRAM Detection' : '显卡与显存体检'}</span>
            </div>
            {#if loadingEnv}
              <p class="mt-2 text-xs text-fg-muted">{isEn ? 'Probing GPU...' : '正在探测显卡...'}</p>
            {:else if envStatus?.gpuName}
              <p class="mt-2 text-xs font-semibold {envStatus.hasSupportedGpu ? 'text-ok' : 'text-warn'}">
                {envStatus.gpuName} ({(envStatus.vramTotalMb / 1024).toFixed(1)} GB VRAM)
              </p>
              <p class="mt-1 text-[11px] text-fg-muted">
                {envStatus.hasSupportedGpu
                  ? isEn
                    ? 'Meets 8 GB+ NVIDIA requirement (NF4 peak ~6.7 GB VRAM).'
                    : '满足本地拆图要求（NF4 量化实测峰值仅 ~6.7 GB 显存）。'
                  : isEn
                    ? 'VRAM < 8 GB detected; local inference may fail or run very slowly.'
                    : '显存低于 8 GB，本地推理可能显存不足（建议仅 8 GB 以上 N 卡下载）。'}
              </p>
            {:else}
              <p class="mt-2 text-xs font-semibold text-err">
                {isEn ? 'No NVIDIA CUDA GPU detected' : '未检测到支持 CUDA 的 NVIDIA 显卡'}
              </p>
              <p class="mt-1 text-[11px] text-fg-muted">
                {isEn
                  ? 'See-Through requires an NVIDIA GPU (8 GB+ VRAM). Non-NVIDIA users may configure a remote ComfyUI endpoint instead.'
                  : '本地运行至少需要 8 GB+ 显存的 NVIDIA 显卡；非 N 卡设备建议前往设置页连接远程 ComfyUI 算力。'}
              </p>
            {/if}
          </div>

          <div class="rounded-xl border border-line bg-ink-2/60 p-3.5">
            <div class="flex items-center gap-2 text-xs font-bold text-fg">
              <HardDrive size={14} class="text-accent" />
              <span>{isEn ? 'Estimated Disk Footprint' : '下载与占用体积透明提示'}</span>
            </div>
            <div class="mt-2 flex flex-col gap-1 text-[11px] text-fg-muted">
              <div class="flex justify-between">
                <span>{isEn ? '64-bit Python + PyTorch CUDA Runtime:' : '独立 Python 3.12 + CUDA 运行时：'}</span>
                <span class="font-mono font-bold text-fg">~{envStatus?.runtimeEstimatedGb ?? 7.7} GB</span>
              </div>
              <div class="flex justify-between">
                <span>
                  {selectedQuant === 'bf16'
                    ? isEn
                      ? 'BF16 Full Precision Models:'
                      : 'BF16 全精度模型权重 (需 12GB+ 显存)：'
                    : isEn
                      ? 'NF4 Quantized Models (Recommended):'
                      : 'NF4 预量化模型权重（推荐）：'}
                </span>
                <span class="font-mono font-bold text-fg">
                  ~{selectedQuant === 'bf16' ? '12.8' : '5.3'} GB
                </span>
              </div>
              <div class="mt-0.5 flex justify-between border-t border-line pt-1">
                <span class="font-semibold text-fg">{isEn ? 'Recommended Free Space:' : '建议目标盘预留可用空间：'}</span>
                <span class="font-mono font-bold {envStatus?.hasEnoughDisk ? 'text-ok' : 'text-err'}">
                  >= {selectedQuant === 'bf16' ? '23.0' : '15.0'} GB
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- Target Installation Directory Picker -->
        <div class="flex flex-col gap-2 rounded-xl border border-line bg-ink-2/40 p-4">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold text-fg">
              {isEn ? 'Extension Installation Directory (Can link existing folder)' : '扩展安装目录（下载前可自由选盘符，也可直接关联已有目录）'}
            </span>
            {#if envStatus}
              <span class="font-mono text-[11px] {envStatus.hasEnoughDisk ? 'text-ok' : 'text-err'}">
                {isEn ? 'Available' : '该盘剩余可用'}: {envStatus.diskFreeGb.toFixed(1)} GB / {envStatus.diskTotalGb.toFixed(0)} GB
              </span>
            {/if}
          </div>

          <div class="flex gap-2">
            <input
              type="text"
              bind:value={targetDirInput}
              disabled={isInstalling}
              placeholder="D:/Sketch2Asset_Extensions/see-through"
              class="flex-1 rounded-lg border border-line bg-ink-0 px-3 py-2 font-mono text-xs text-fg focus:border-accent focus:outline-none"
            />
            <button
              type="button"
              disabled={isInstalling}
              onclick={handlePickInstallFolder}
              class="flex shrink-0 items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3 py-2 text-xs font-semibold text-fg hover:border-accent"
            >
              <FolderOpen size={14} />
              <span>{isEn ? 'Browse...' : '选择文件夹...'}</span>
            </button>
            <button
              type="button"
              disabled={isInstalling}
              onclick={handleSaveTypedDir}
              class="shrink-0 rounded-lg border border-line bg-ink-2 px-3 py-2 text-xs font-semibold text-fg-muted hover:border-accent hover:text-fg"
            >
              {isEn ? 'Check / Link' : '检测 / 关联'}
            </button>
          </div>

          {#if envStatus}
            <div class="mt-1 flex flex-wrap gap-3 text-[11px]">
              <span class={envStatus.repoReady ? 'text-ok' : 'text-fg-subtle'}>
                {envStatus.repoReady ? '✓' : '○'} {isEn ? 'Core Scripts' : '推理代码'}
              </span>
              <span class={envStatus.runtimeReady ? 'text-ok' : 'text-fg-subtle'}>
                {envStatus.runtimeReady ? '✓' : '○'} {isEn ? 'CUDA Python Runtime' : 'CUDA 运行时'}
              </span>
              <span class={envStatus.modelsReady ? 'text-ok' : 'text-fg-subtle'}>
                {envStatus.modelsReady ? '✓' : '○'} {isEn ? 'Model Weights' : '模型权重'}
              </span>
            </div>
          {/if}
        </div>

        <!-- Quantization & Mirror Options -->
        <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
          <label class="flex flex-col gap-1.5 text-xs">
            <span class="font-semibold text-fg-muted">{isEn ? 'Model Precision Profile' : '模型显存档案'}</span>
            <select
              bind:value={selectedQuant}
              disabled={isInstalling}
              class="rounded-lg border border-line bg-ink-0 px-3 py-2 text-xs text-fg focus:border-accent focus:outline-none"
            >
              <option value="nf4">NF4 4-bit ({isEn ? 'Recommended · ~5.3 GB · 8GB+ GPU' : '推荐 · 权重约 5.3 GB · 8GB+ 显卡'})</option>
              <option value="bf16">BF16 ({isEn ? 'High VRAM · ~12.8 GB · 12GB+ GPU' : '原精度 · 权重约 12.8 GB · 12GB+ 显卡'})</option>
            </select>
          </label>

          <label class="flex flex-col gap-1.5 text-xs">
            <span class="font-semibold text-fg-muted">{isEn ? 'Download Source (8x Parallel Range)' : '下载加速源（8 线程分片并发）'}</span>
            <select
              bind:value={selectedMirror}
              disabled={isInstalling}
              class="rounded-lg border border-line bg-ink-0 px-3 py-2 text-xs text-fg focus:border-accent focus:outline-none"
            >
              <option value="hf-mirror">{isEn ? 'HF-Mirror + Tsinghua/Aliyun (CN Fast)' : 'HF-Mirror 国内多线加速 + 清华/阿里源（推荐）'}</option>
              <option value="huggingface">{isEn ? 'Official HuggingFace + PyPI' : 'HuggingFace + PyTorch 官方直连'}</option>
            </select>
          </label>
        </div>

        {#if isInstalling}
          <div class="flex flex-col gap-2 rounded-xl border border-accent/40 bg-accent/5 p-4">
            <div class="flex items-center justify-between text-xs">
              <span class="flex items-center gap-2 font-bold text-accent">
                <Loader2 size={14} class="animate-spin" />
                {installMessage}
              </span>
              <span class="font-mono text-[11px] font-bold text-fg">
                {installPercent.toFixed(1)}%
                {#if installSpeedMbS > 0}
                  · {installSpeedMbS.toFixed(1)} MB/s
                {/if}
                {#if installTotalMb > 0 && installStage === 'models'}
                  ({installDownloadedMb.toFixed(0)} / {installTotalMb.toFixed(0)} MB)
                {/if}
              </span>
            </div>
            <div class="h-2 w-full overflow-hidden rounded-full bg-ink-2">
              <div class="h-full bg-accent transition-all duration-200" style="width: {installPercent}%"></div>
            </div>
          </div>
        {/if}

        <div class="flex flex-wrap items-center justify-between gap-3 pt-1">
          <div class="flex items-center gap-2">
            {#if envStatus?.runtimeReady || envStatus?.modelsReady}
              {#if uninstallConfirm}
                <button
                  type="button"
                  onclick={handleUninstall}
                  class="rounded-lg bg-err px-3 py-2 text-xs font-bold text-white hover:opacity-90"
                >
                  {isEn ? 'Confirm Delete Extension Folder' : '确认彻底删除扩展文件夹'}
                </button>
                <button
                  type="button"
                  onclick={() => (uninstallConfirm = false)}
                  class="rounded-lg border border-line bg-ink-2 px-3 py-2 text-xs text-fg-muted"
                >
                  {isEn ? 'Cancel' : '取消'}
                </button>
              {:else}
                <button
                  type="button"
                  disabled={isInstalling}
                  onclick={() => (uninstallConfirm = true)}
                  class="flex items-center gap-1.5 rounded-lg border border-err/40 bg-err/10 px-3 py-2 text-xs font-semibold text-err hover:bg-err/20"
                >
                  <Trash2 size={13} />
                  <span>{isEn ? 'Uninstall & Free Disk' : '卸载扩展释放空间'}</span>
                </button>
              {/if}
            {/if}
          </div>

          <div class="flex items-center gap-2.5">
            {#if isInstalling}
              <button
                type="button"
                onclick={handleCancelInstall}
                class="rounded-lg border border-err/50 bg-err/10 px-4 py-2 text-xs font-bold text-err hover:bg-err/20"
              >
                {isEn ? 'Cancel Download' : '取消安装'}
              </button>
            {:else}
              <button
                type="button"
                disabled={!envStatus?.hasEnoughDisk}
                onclick={handleStartInstall}
                class="flex items-center gap-2 rounded-xl bg-accent px-5 py-2.5 text-xs font-bold text-accent-ink shadow-md transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-45"
              >
                <Download size={15} />
                <span>
                  {envStatus?.ready
                    ? isEn
                      ? 'Verify / Re-sync Extension'
                      : '校验 / 补齐缺失文件'
                    : isEn
                      ? 'Download & Install to Selected Folder'
                      : '开始下载并安装到选定目录'}
                </span>
              </button>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {:else}
    <!-- Main 3-Column Layer Decomposition Studio -->
    <div class="flex flex-1 overflow-hidden">
      <!-- Left Column: Input Image, Inference Config & History -->
      <aside class="flex w-72 shrink-0 flex-col gap-3 overflow-y-auto border-r border-line bg-ink-1/70 p-3.5">
        <div class="flex items-center justify-between">
          <span class="text-xs font-bold text-fg">{isEn ? 'Source Illustration' : '待拆角色立绘'}</span>
          <button
            type="button"
            onclick={() => fileInputEl?.click()}
            class="flex items-center gap-1 rounded-md border border-line bg-ink-2 px-2 py-1 text-[11px] font-semibold text-fg-muted hover:border-accent hover:text-fg"
          >
            <Upload size={12} />
            <span>{isEn ? 'Upload' : '导入本地图'}</span>
          </button>
          <input
            bind:this={fileInputEl}
            type="file"
            accept="image/png,image/jpeg,image/webp"
            class="hidden"
            onchange={handleSelectLocalFile}
          />
        </div>

        <button
          type="button"
          onclick={() => fileInputEl?.click()}
          class="group relative flex aspect-[3/4] w-full items-center justify-center overflow-hidden rounded-xl border border-dashed border-line-strong bg-ink-0/80 transition-colors hover:border-accent"
        >
          {#if inputPreviewSrc}
            <img src={inputPreviewSrc} alt="Source" class="h-full w-full object-contain p-1" />
          {:else}
            <div class="flex flex-col items-center gap-2 px-4 text-center text-fg-subtle">
              <Upload size={22} />
              <span class="text-[11px]">
                {isEn
                  ? 'Click to upload or pick a character from the bottom filmstrip'
                  : '点击导入立绘，或直接在下方胶片栏点选人物资产'}
              </span>
            </div>
          {/if}
        </button>

        <!-- Decompose Settings -->
        <div class="flex flex-col gap-2.5 rounded-xl border border-line bg-ink-2/40 p-3 text-xs">
          <div class="flex items-center justify-between gap-2">
            <span class="shrink-0 font-semibold text-fg-muted">{isEn ? 'Quality Preset' : '精度与速度'}</span>
            <select
              bind:value={runPreset}
              disabled={isRunning}
              class="min-w-0 flex-1 rounded-md border border-line bg-ink-0 px-2 py-1 text-[11px] text-fg"
            >
              <option value="896_16">{isEn ? 'Turbo Cool (896px · 16s)' : '极速低温 (896px · 16步)'}</option>
              <option value="1024_20">{isEn ? 'Balanced (1024px · 20s)' : '标准平衡 (1024px · 20步 · 推荐)'}</option>
              <option value="1280_30">{isEn ? 'High Detail (1280px · 30s)' : '高清精细 (1280px · 30步 · 耗时长)'}</option>
            </select>
          </div>

          <label class="flex cursor-pointer items-center justify-between text-[11px] text-fg-muted">
            <span>{isEn ? 'Auto Remove Solid BG (BiRefNet)' : '实底图自动前置去背景'}</span>
            <input type="checkbox" bind:checked={runAutoRemoveBg} disabled={isRunning} class="accent-accent" />
          </label>

          <label class="flex cursor-pointer items-center justify-between text-[11px] text-fg-muted">
            <span>{isEn ? 'Split Left/Right Parts' : '自动拆离左右手眼耳'}</span>
            <input type="checkbox" bind:checked={runTblrSplit} disabled={isRunning} class="accent-accent" />
          </label>

          <label
            class="flex cursor-pointer items-center justify-between text-[11px] text-fg-muted"
            title={isEn ? 'Auto enables Group Offload when free VRAM is tight to prevent 10x Windows paging slowdowns' : '当可用显存偏紧时自动开启分块卸载，避免突破显存导致掉速烤机'}
          >
            <span>{isEn ? 'VRAM Paging Guard (Auto Offload)' : '显存防换页保护 (Auto Offload)'}</span>
            <input type="checkbox" bind:checked={runAutoGroupOffload} disabled={isRunning} class="accent-accent" />
          </label>

          <label
            class="flex cursor-pointer items-center justify-between text-[11px] text-fg-muted"
            title={isEn ? 'Brief 60ms pause between diffusion steps to keep GPU cooler & quieter' : '每步之间微歇 60ms，降低持续满载峰值温度'}
          >
            <span>{isEn ? 'Thermal Cool Mode (60ms/step)' : '温控护卡模式 (每步微歇降温)'}</span>
            <input type="checkbox" bind:checked={runCoolMode} disabled={isRunning} class="accent-accent" />
          </label>

          <label class="flex cursor-pointer items-center justify-between text-[11px] text-fg-muted">
            <span>{isEn ? 'Force Full CPU Offload (<8GB)' : '强制全模型 CPU Offload (<8G)'}</span>
            <input type="checkbox" bind:checked={runCpuOffload} disabled={isRunning} class="accent-accent" />
          </label>
        </div>

        {#if isRunning}
          <div class="flex gap-2">
            <button
              type="button"
              disabled
              class="flex flex-1 items-center justify-center gap-2 rounded-xl bg-accent/80 py-2.5 text-xs font-bold text-accent-ink shadow-md"
            >
              <Loader2 size={15} class="animate-spin" />
              <span>{runPercent}% · {runElapsedSec}s</span>
            </button>
            <button
              type="button"
              onclick={handleCancelDecomposeRun}
              class="flex items-center justify-center gap-1 rounded-xl border border-err/60 bg-err/15 px-3 py-2.5 text-xs font-bold text-err hover:bg-err/25"
              title={isEn ? 'Abort inference and free GPU VRAM' : '立即中止拆图并释放显存'}
            >
              <Square size={12} fill="currentColor" />
              <span>{isEn ? 'Stop' : '中止'}</span>
            </button>
          </div>

          <div class="flex flex-col gap-1 rounded-lg border border-accent/30 bg-accent/5 p-2.5">
            <div class="h-1.5 w-full overflow-hidden rounded-full bg-ink-2">
              <div class="h-full bg-accent transition-all duration-200" style="width: {runPercent}%"></div>
            </div>
            <span class="text-[10px] leading-tight text-accent">{runMessage || runStage}</span>
          </div>
        {:else}
          <button
            type="button"
            disabled={!inputFilePath}
            onclick={handleRunDecompose}
            class="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-xs font-bold text-accent-ink shadow-md transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-45"
          >
            <Sparkles size={15} />
            <span>{isEn ? 'Decompose Into 23 Layers' : '开始 23 层智能拆图'}</span>
          </button>
        {/if}

        <!-- History List -->
        <div class="mt-1 flex flex-col gap-1.5 border-t border-line pt-3">
          <div class="flex items-center gap-1.5 text-[11px] font-bold text-fg-muted">
            <History size={12} />
            <span>{isEn ? 'Recent Decompose Jobs' : '历史拆图工程'}</span>
          </div>
          {#if historyList.length === 0}
            <span class="text-[11px] text-fg-subtle">{isEn ? 'No history yet.' : '暂无历史拆图记录'}</span>
          {:else}
            <div class="flex max-h-40 flex-col gap-1 overflow-y-auto pr-1">
              {#each historyList as item}
                <button
                  type="button"
                  onclick={() => handleLoadHistoryJob(item)}
                  class="flex items-center gap-2 rounded-lg border px-2 py-1.5 text-left transition-colors {currentJob?.jobId ===
                  item.jobId
                    ? 'border-accent bg-accent/10'
                    : 'border-line bg-ink-2/40 hover:border-accent/50'}"
                >
                  <img
                    src={assetUrl(item.reconstructionPath)}
                    alt={item.jobId}
                    class="h-8 w-8 shrink-0 rounded bg-ink-0 object-contain"
                  />
                  <div class="min-w-0 flex-1">
                    <div class="truncate font-mono text-[10px] font-bold text-fg">{item.jobId}</div>
                    <div class="text-[10px] text-fg-muted">
                      {item.layerCount} {isEn ? 'layers' : '层'} · {item.width}×{item.height}
                    </div>
                  </div>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </aside>

      <!-- Center Column: Interactive Layered Viewport + Bottom Project Filmstrip -->
      <div class="flex flex-1 flex-col overflow-hidden">
        <!-- Canvas Toolbar -->
        <div class="flex h-10 shrink-0 items-center justify-between border-b border-line bg-ink-1/40 px-3">
          <div class="flex items-center gap-1.5">
            {#each [['checker', isEn ? 'Grid' : '棋盘'], ['dark', isEn ? 'Dark' : '深灰'], ['light', isEn ? 'Light' : '纯白'], ['magenta', 'Magenta']] as [mode, label]}
              <button
                type="button"
                onclick={() => (backdropMode = mode as any)}
                class="rounded px-2 py-0.5 text-[11px] font-semibold transition-colors {backdropMode === mode
                  ? 'bg-accent text-accent-ink'
                  : 'bg-ink-2 text-fg-muted hover:text-fg'}"
              >
                {label}
              </button>
            {/each}

            <div class="mx-1 h-4 w-px bg-line"></div>

            <button
              type="button"
              disabled={!selectedLayer}
              onclick={() => (soloMode = !soloMode)}
              class="rounded px-2 py-0.5 text-[11px] font-semibold transition-colors {soloMode
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted hover:text-fg disabled:opacity-40'}"
            >
              {isEn ? 'Solo Selected' : '仅看选中层'}
            </button>

            <button
              type="button"
              disabled={!selectedLayer?.depthPath}
              onclick={() => (viewDepthMap = !viewDepthMap)}
              class="rounded px-2 py-0.5 text-[11px] font-semibold transition-colors {viewDepthMap
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted hover:text-fg disabled:opacity-40'}"
            >
              {isEn ? 'Depth Map' : '伪深度图'}
            </button>

            <div class="mx-1 h-4 w-px bg-line"></div>

            <button
              type="button"
              onclick={() => (interactionMode = 'pan')}
              class="flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-semibold transition-colors {interactionMode === 'pan'
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted hover:text-fg'}"
              title={isEn ? 'Drag canvas viewport (Q)' : '纯平移画布视图，隐藏坐标轴 (Q)'}
            >
              <Hand size={11} />
              <span>{isEn ? 'Pan (Q)' : '平移画布 (Q)'}</span>
            </button>

            <button
              type="button"
              disabled={!selectedLayer}
              onclick={() => (interactionMode = 'move')}
              class="flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-semibold transition-colors {interactionMode === 'move'
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted hover:text-fg disabled:opacity-40'}"
              title={isEn
                ? 'Unity-style XY Gizmo (W): Red arrow = X only, Green arrow = Y only, Blue square = free XY, Arrow keys = 1px nudge'
                : 'Unity 式 XY 坐标轴移动 (W)：红箭头锁定 X 轴，绿箭头锁定 Y 轴，浅蓝方块自由移动，方向键 1px 微调'}
            >
              <Move size={11} />
              <span>{isEn ? 'XY Gizmo (W)' : 'XY 轴移动 (W)'}</span>
            </button>

            {#if hasAnyLayerOffset}
              <button
                type="button"
                onclick={resetAllLayerOffsets}
                class="rounded border border-warn/40 bg-warn/10 px-2 py-0.5 text-[10px] font-semibold text-warn hover:bg-warn/20"
              >
                {isEn ? 'Reset All Pos' : '全部归位'}
              </button>
            {/if}
          </div>

          <div class="flex items-center gap-1.5 font-mono text-[11px] text-fg-muted">
            <button type="button" onclick={() => (zoom = Math.max(0.25, +(zoom - 0.15).toFixed(2)))} class="rounded bg-ink-2 p-1 hover:text-fg">
              <ZoomOut size={13} />
            </button>
            <span class="w-11 text-center">{Math.round(zoom * 100)}%</span>
            <button type="button" onclick={() => (zoom = Math.min(6, +(zoom + 0.15).toFixed(2)))} class="rounded bg-ink-2 p-1 hover:text-fg">
              <ZoomIn size={13} />
            </button>
            <button type="button" onclick={resetView} class="rounded bg-ink-2 p-1 hover:text-fg" title={isEn ? 'Reset View' : '复位视图'}>
              <RotateCcw size={13} />
            </button>
          </div>
        </div>

        <!-- Viewport Stage -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          role="region"
          aria-label="Decompose Viewport"
          onwheel={handleWheel}
          onmousedown={handleMouseDown}
          class="relative flex flex-1 items-center justify-center overflow-hidden {isMovingLayer
            ? moveAxis === 'x'
              ? 'cursor-ew-resize'
              : moveAxis === 'y'
                ? 'cursor-ns-resize'
                : 'cursor-move'
            : isPanning
              ? 'cursor-grabbing'
              : 'cursor-grab'} {backdropMode === 'dark'
            ? 'bg-[#12151c]'
            : backdropMode === 'light'
              ? 'bg-white'
              : backdropMode === 'magenta'
                ? 'bg-[#ff00ff]'
                : 'checker-bg'}"
        >
          {#if currentJob && layers.length > 0}
            <div
              class="relative select-none shadow-2xl"
              style="width: {stageDisplay.w}px; height: {stageDisplay.h}px; transform: translate({panX}px, {panY}px) scale({zoom});"
            >
              {#if viewDepthMap && selectedLayer?.depthPath}
                <img
                  src={layerImgSrc(selectedLayer, true)}
                  alt={selectedLayer.name}
                  class="pointer-events-none absolute inset-0 h-full w-full object-contain"
                />
              {:else if soloMode && selectedLayer}
                <img
                  src={layerImgSrc(selectedLayer, false)}
                  alt={selectedLayer.name}
                  style="opacity: {selectedLayer.opacity}; transform: translate({((selectedLayer.offsetX ?? 0) / currentJob.width) * 100}%, {((selectedLayer.offsetY ?? 0) / currentJob.height) * 100}%);"
                  class="pointer-events-none absolute inset-0 h-full w-full object-contain"
                />
              {:else}
                {#each layersBottomUp as layer (layer.id)}
                  {#if layer.visible}
                    <img
                      src={layerImgSrc(layer, false)}
                      alt={layer.name}
                      style="opacity: {layer.opacity}; transform: translate({((layer.offsetX ?? 0) / currentJob.width) * 100}%, {((layer.offsetY ?? 0) / currentJob.height) * 100}%); filter: {selectedLayerId === layer.id ? 'drop-shadow(0 0 6px rgba(132,204,22,0.55))' : 'none'};"
                      class="pointer-events-none absolute inset-0 h-full w-full object-contain"
                    />
                  {/if}
                {/each}
              {/if}

              {#if interactionMode === 'move' && selectedLayer && selectedGizmo && !viewDepthMap}
                <!-- Selected layer dashed bounding box -->
                <div
                  class="pointer-events-none absolute z-20"
                  style="left: {selectedGizmo.leftPct}%; top: {selectedGizmo.topPct}%; width: {selectedGizmo.widthPct}%; height: {selectedGizmo.heightPct}%; border: {Math.max(0.75, 1 / zoom)}px dashed rgba(96, 165, 250, 0.75);"
                ></div>

                <!-- Unity-style 2D XY Move Gizmo (constant screen size independent of zoom) -->
                <div
                  class="pointer-events-none absolute z-30"
                  style="left: {selectedGizmo.cxPct}%; top: {selectedGizmo.cyPct}%; transform: scale({1 / zoom}); transform-origin: 0 0;"
                >
                  <svg
                    width="160"
                    height="160"
                    viewBox="-24 -136 160 160"
                    style="position: absolute; left: -24px; top: -136px; overflow: visible;"
                    class="pointer-events-none select-none"
                  >
                    {#if moveAxis === 'x'}
                      <line
                        x1="-2000"
                        y1="0"
                        x2="2000"
                        y2="0"
                        stroke="rgba(239, 68, 68, 0.5)"
                        stroke-width="1"
                        stroke-dasharray="5 4"
                      />
                    {/if}
                    {#if moveAxis === 'y'}
                      <line
                        x1="0"
                        y1="-2000"
                        x2="0"
                        y2="2000"
                        stroke="rgba(101, 230, 67, 0.5)"
                        stroke-width="1"
                        stroke-dasharray="5 4"
                      />
                    {/if}

                    <!-- Light-Blue Corner Square: Free XY Plane Move -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <rect
                      x="1"
                      y="-23"
                      width="22"
                      height="22"
                      fill={gizmoXyActive ? 'rgba(250, 204, 21, 0.35)' : 'rgba(96, 165, 250, 0.22)'}
                      stroke={gizmoXyActive ? '#fde047' : '#60a5fa'}
                      stroke-width="1.5"
                      class="pointer-events-auto cursor-move"
                      onmousedown={(e) => startGizmoDrag(e, 'xy')}
                      onmouseenter={() => (hoveredAxis = 'xy')}
                      onmouseleave={() => {
                        if (hoveredAxis === 'xy') hoveredAxis = null;
                      }}
                    >
                      <title>{isEn ? 'Free XY Move' : 'XY 平面自由拖动'}</title>
                    </rect>

                    <!-- Green Up Arrow: Y-Axis Constrained Move -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <g
                      class="pointer-events-auto cursor-ns-resize"
                      onmousedown={(e) => startGizmoDrag(e, 'y')}
                      onmouseenter={() => (hoveredAxis = 'y')}
                      onmouseleave={() => {
                        if (hoveredAxis === 'y') hoveredAxis = null;
                      }}
                    >
                      <title>{isEn ? 'Move Y Axis Only' : '仅沿 Y 轴上下移动'}</title>
                      <line x1="0" y1="-4" x2="0" y2="-96" stroke="transparent" stroke-width="18" />
                      <line
                        x1="0"
                        y1="0"
                        x2="0"
                        y2="-68"
                        stroke="rgba(0,0,0,0.55)"
                        stroke-width="5"
                        stroke-linecap="round"
                      />
                      <line
                        x1="0"
                        y1="0"
                        x2="0"
                        y2="-68"
                        stroke={gizmoYColor}
                        stroke-width="2.75"
                        stroke-linecap="round"
                      />
                      <polygon
                        points="0,-88 -7.5,-65 7.5,-65"
                        fill={gizmoYColor}
                        stroke="rgba(0,0,0,0.45)"
                        stroke-width="1"
                        stroke-linejoin="round"
                      />
                    </g>

                    <!-- Red Right Arrow: X-Axis Constrained Move -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <g
                      class="pointer-events-auto cursor-ew-resize"
                      onmousedown={(e) => startGizmoDrag(e, 'x')}
                      onmouseenter={() => (hoveredAxis = 'x')}
                      onmouseleave={() => {
                        if (hoveredAxis === 'x') hoveredAxis = null;
                      }}
                    >
                      <title>{isEn ? 'Move X Axis Only' : '仅沿 X 轴左右移动'}</title>
                      <line x1="4" y1="0" x2="96" y2="0" stroke="transparent" stroke-width="18" />
                      <line
                        x1="0"
                        y1="0"
                        x2="68"
                        y2="0"
                        stroke="rgba(0,0,0,0.55)"
                        stroke-width="5"
                        stroke-linecap="round"
                      />
                      <line
                        x1="0"
                        y1="0"
                        x2="68"
                        y2="0"
                        stroke={gizmoXColor}
                        stroke-width="2.75"
                        stroke-linecap="round"
                      />
                      <polygon
                        points="88,0 65,-7.5 65,7.5"
                        fill={gizmoXColor}
                        stroke="rgba(0,0,0,0.45)"
                        stroke-width="1"
                        stroke-linejoin="round"
                      />
                    </g>

                    <!-- Origin Pivot Dot -->
                    <circle
                      cx="0"
                      cy="0"
                      r="2.8"
                      fill="#ffffff"
                      stroke="rgba(0,0,0,0.65)"
                      stroke-width="1.2"
                    />
                  </svg>

                  {#if isMovingLayer || hoveredAxis}
                    <div
                      class="pointer-events-none absolute flex items-center gap-1.5 whitespace-nowrap rounded border border-white/15 bg-black/80 px-1.5 py-0.5 font-mono text-[10px] text-white shadow-md backdrop-blur-sm"
                      style="left: 14px; top: 14px;"
                    >
                      {#if (moveAxis ?? hoveredAxis) === 'x'}
                        <span class="font-bold text-[#f87171]">X</span>
                      {:else if (moveAxis ?? hoveredAxis) === 'y'}
                        <span class="font-bold text-[#4ade80]">Y</span>
                      {:else}
                        <span class="font-bold text-[#60a5fa]">XY</span>
                      {/if}
                      <span>X:{selectedGizmo.ox} Y:{selectedGizmo.oy}</span>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          {:else}
            <div class="flex max-w-md flex-col items-center gap-3 rounded-2xl border border-line bg-ink-1/90 p-6 text-center backdrop-blur">
              <Layers size={28} class="text-accent" />
              <h3 class="text-sm font-bold text-fg">
                {isEn ? '2.5D Layer Decompose Canvas' : '2.5D 立绘分层工作台'}
              </h3>
              <p class="text-xs leading-relaxed text-fg-muted">
                {isEn
                  ? 'Select a character portrait from the left panel or bottom filmstrip, then click "Decompose Into 23 Layers" to extract occluded body/hair/outfit layers and layered PSD.'
                  : '在左侧或下方项目胶片栏选择角色立绘，点击「开始 23 层智能拆图」，即可自动透视补全被遮挡区域并生成可调顺序的 PSD 与散件。'}
              </p>
            </div>
          {/if}
        </div>

        <!-- Bottom Project Asset Filmstrip -->
        <div class="flex h-28 shrink-0 flex-col border-t border-line bg-ink-1/80 px-3 py-2">
          <div class="mb-1.5 flex items-center justify-between">
            <div class="flex items-center gap-2 text-[11px] font-bold text-fg-muted">
              <span>{isEn ? 'Project Assets (Click to load)' : '项目资产胶片栏（点击设为待拆立绘）'}</span>
              <select
                bind:value={filmstripProjectFilter}
                onchange={() => refreshHistoryAndAssets()}
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
              onclick={() => refreshHistoryAndAssets()}
              class="text-fg-muted hover:text-accent"
              title={isEn ? 'Refresh' : '刷新'}
            >
              <RefreshCw size={12} />
            </button>
          </div>
          <div class="flex flex-1 gap-2 overflow-x-auto pb-1">
            {#each libraryAssets as asset (asset.id)}
              <button
                type="button"
                onclick={() => selectAssetForDecompose(asset)}
                class="group relative aspect-square h-full shrink-0 overflow-hidden rounded-lg border transition-all {inputFilePath ===
                asset.filePath
                  ? 'border-accent ring-1 ring-accent'
                  : 'border-line bg-ink-0 hover:border-accent/60'}"
              >
                <img src={assetUrl(asset.filePath)} alt={asset.prompt} class="h-full w-full object-contain" />
              </button>
            {/each}
          </div>
        </div>
      </div>

      <!-- Right Column: 23-Layer Tree, Z-Order Drag/Controls, Defringe & PSD Export -->
      <aside class="flex w-80 shrink-0 flex-col border-l border-line bg-ink-1/70">
        <div class="flex items-center justify-between border-b border-line px-3.5 py-2.5">
          <span class="text-xs font-bold text-fg">
            {isEn ? `Semantic Layers (${layers.length})` : `语义图层栈 (${layers.length} 层 · 顶层优先)`}
          </span>
          {#if selectedLayer}
            <span class="rounded bg-accent/15 px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent">
              Z:{selectedLayer.zIndex}
            </span>
          {/if}
        </div>

        <!-- Layer Stack List -->
        <div class="flex flex-1 flex-col gap-1 overflow-y-auto p-2">
          {#if layersTopDown.length === 0}
            <div class="flex flex-1 items-center justify-center p-4 text-center text-xs text-fg-subtle">
              {isEn ? 'Run decomposition or select a history job to inspect layers.' : '拆图完成后将在此展示可调深度顺序的 23 个透明图层'}
            </div>
          {:else}
            {#each layersTopDown as layer (layer.id)}
              <div
                role="listitem"
                draggable="true"
                ondragstart={() => (draggingLayerId = layer.id)}
                ondragover={(e) => {
                  e.preventDefault();
                  dragOverLayerId = layer.id;
                }}
                ondragleave={() => {
                  if (dragOverLayerId === layer.id) dragOverLayerId = null;
                }}
                ondrop={(e) => {
                  e.preventDefault();
                  if (draggingLayerId) reorderLayerDrop(draggingLayerId, layer.id);
                  draggingLayerId = null;
                  dragOverLayerId = null;
                }}
                ondragend={() => {
                  draggingLayerId = null;
                  dragOverLayerId = null;
                }}
                class="flex items-center gap-1.5 rounded-lg border p-1.5 transition-colors {selectedLayerId ===
                layer.id
                  ? 'border-accent bg-accent/10'
                  : dragOverLayerId === layer.id
                    ? 'border-accent/80 bg-accent/5'
                    : 'border-line/60 bg-ink-2/30 hover:border-line-strong'}"
              >
                <span class="cursor-grab text-fg-subtle hover:text-fg-muted" title={isEn ? 'Drag to reorder Z-Order' : '按住上下拖拽调整图层前后顺序'}>
                  <GripVertical size={12} />
                </span>

                <button
                  type="button"
                  onclick={() => toggleLayerVisibility(layer.id)}
                  class="rounded p-1 text-fg-muted hover:text-fg"
                  title={layer.visible ? 'Hide' : 'Show'}
                >
                  {#if layer.visible}
                    <Eye size={13} class="text-accent" />
                  {:else}
                    <EyeOff size={13} class="text-fg-subtle" />
                  {/if}
                </button>

                <button
                  type="button"
                  onclick={() => (selectedLayerId = layer.id)}
                  class="flex min-w-0 flex-1 items-center gap-2 text-left"
                >
                  <img
                    src={layerImgSrc(layer, false)}
                    alt={layer.name}
                    class="h-8 w-8 shrink-0 rounded border border-line bg-ink-0 object-contain"
                  />
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-[11px] font-semibold text-fg">
                      {layerDisplayName(layer.name)}
                    </div>
                    <div class="font-mono text-[9px] text-fg-subtle">
                      {layer.right - layer.left}×{layer.bottom - layer.top}px · {Math.round(layer.opacity * 100)}%
                      {#if (layer.offsetX ?? 0) !== 0 || (layer.offsetY ?? 0) !== 0}
                        <span class="text-accent"> · Δ{layer.offsetX ?? 0},{layer.offsetY ?? 0}</span>
                      {/if}
                    </div>
                  </div>
                </button>

                <div class="flex shrink-0 flex-col gap-0.5">
                  <button
                    type="button"
                    onclick={() => moveLayerOrder(layer.id, 1)}
                    class="rounded bg-ink-2 p-0.5 text-fg-muted hover:text-accent"
                    title={isEn ? 'Bring Forward (Z+1)' : '上移一层 (Z+1)'}
                  >
                    <ArrowUp size={11} />
                  </button>
                  <button
                    type="button"
                    onclick={() => moveLayerOrder(layer.id, -1)}
                    class="rounded bg-ink-2 p-0.5 text-fg-muted hover:text-accent"
                    title={isEn ? 'Send Backward (Z-1)' : '下移一层 (Z-1)'}
                  >
                    <ArrowDown size={11} />
                  </button>
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <!-- Selected Layer Actions & Export Footer -->
        <div class="flex flex-col gap-2.5 border-t border-line bg-ink-1 p-3">
          {#if selectedLayer}
            <div class="flex flex-col gap-1.5 rounded-lg border border-line bg-ink-2/40 p-2.5">
              <div class="flex items-center justify-between text-[11px]">
                <span class="font-semibold text-fg">{layerDisplayName(selectedLayer.name)}</span>
                <span class="font-mono text-fg-muted">{Math.round(selectedLayer.opacity * 100)}%</span>
              </div>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={selectedLayer.opacity}
                oninput={(e) => setLayerOpacity(selectedLayer!.id, parseFloat((e.target as HTMLInputElement).value))}
                class="w-full accent-accent"
              />

              <!-- 4-way Layer Offset Nudge & Coordinate Controls -->
              <div class="mt-0.5 flex items-center justify-between gap-2 rounded-md border border-line/70 bg-ink-0/60 p-1.5">
                <div class="grid grid-cols-3 gap-0.5">
                  <div></div>
                  <button
                    type="button"
                    onclick={() => nudgeLayerOffset(selectedLayer!.id, 0, -layerMoveStep)}
                    class="flex h-6 w-6 items-center justify-center rounded bg-ink-2 text-fg-muted hover:text-accent"
                    title={isEn ? 'Move Up' : '上移'}
                  >
                    <ArrowUp size={11} />
                  </button>
                  <div></div>
                  <button
                    type="button"
                    onclick={() => nudgeLayerOffset(selectedLayer!.id, -layerMoveStep, 0)}
                    class="flex h-6 w-6 items-center justify-center rounded bg-ink-2 text-fg-muted hover:text-accent"
                    title={isEn ? 'Move Left' : '左移'}
                  >
                    <ArrowLeft size={11} />
                  </button>
                  <button
                    type="button"
                    onclick={() => setLayerOffset(selectedLayer!.id, 0, 0)}
                    class="flex h-6 w-6 items-center justify-center rounded bg-ink-2 font-mono text-[9px] text-fg-muted hover:text-accent"
                    title={isEn ? 'Reset to 0,0' : '归零复位'}
                  >
                    0
                  </button>
                  <button
                    type="button"
                    onclick={() => nudgeLayerOffset(selectedLayer!.id, layerMoveStep, 0)}
                    class="flex h-6 w-6 items-center justify-center rounded bg-ink-2 text-fg-muted hover:text-accent"
                    title={isEn ? 'Move Right' : '右移'}
                  >
                    <ArrowRight size={11} />
                  </button>
                  <div></div>
                  <button
                    type="button"
                    onclick={() => nudgeLayerOffset(selectedLayer!.id, 0, layerMoveStep)}
                    class="flex h-6 w-6 items-center justify-center rounded bg-ink-2 text-fg-muted hover:text-accent"
                    title={isEn ? 'Move Down' : '下移'}
                  >
                    <ArrowDown size={11} />
                  </button>
                  <div></div>
                </div>

                <div class="flex flex-1 flex-col gap-1">
                  <div class="flex items-center justify-between gap-1 text-[10px]">
                    <span class="text-fg-subtle">{isEn ? 'Step' : '步长'}</span>
                    <div class="flex gap-1">
                      {#each [1, 5, 20] as s}
                        <button
                          type="button"
                          onclick={() => (layerMoveStep = s as 1 | 5 | 20)}
                          class="rounded px-1.5 py-0.5 font-mono text-[9px] {layerMoveStep === s
                            ? 'bg-accent text-accent-ink font-bold'
                            : 'bg-ink-2 text-fg-muted hover:text-fg'}"
                        >
                          {s}
                        </button>
                      {/each}
                    </div>
                  </div>
                  <div class="grid grid-cols-2 gap-1 font-mono text-[10px]">
                    <label class="flex items-center gap-1 rounded border border-line bg-ink-1 px-1.5 py-0.5 text-fg-muted">
                      <span class="font-bold text-[#f87171]">X</span>
                      <input
                        type="number"
                        value={selectedLayer.offsetX ?? 0}
                        onchange={(e) =>
                          setLayerOffset(
                            selectedLayer!.id,
                            parseInt((e.target as HTMLInputElement).value || '0', 10) || 0,
                            selectedLayer!.offsetY ?? 0
                          )}
                        class="w-full bg-transparent text-right text-fg focus:outline-none"
                      />
                    </label>
                    <label class="flex items-center gap-1 rounded border border-line bg-ink-1 px-1.5 py-0.5 text-fg-muted">
                      <span class="font-bold text-[#4ade80]">Y</span>
                      <input
                        type="number"
                        value={selectedLayer.offsetY ?? 0}
                        onchange={(e) =>
                          setLayerOffset(
                            selectedLayer!.id,
                            selectedLayer!.offsetX ?? 0,
                            parseInt((e.target as HTMLInputElement).value || '0', 10) || 0
                          )}
                        class="w-full bg-transparent text-right text-fg focus:outline-none"
                      />
                    </label>
                  </div>
                </div>
              </div>
              <div class="mt-1 grid grid-cols-2 gap-1.5">
                <button
                  type="button"
                  disabled={defringeBusy}
                  onclick={handleDefringeSelected}
                  class="flex items-center justify-center gap-1 rounded-md border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent disabled:opacity-50"
                >
                  <Scissors size={12} />
                  <span>{defringeBusy ? '...' : isEn ? 'Anti-Halo' : '去白边净化'}</span>
                </button>
                <button
                  type="button"
                  onclick={handleSendLayerToStudio}
                  class="flex items-center justify-center gap-1 rounded-md border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent"
                >
                  <Wand2 size={12} />
                  <span>{isEn ? 'Open Studio' : '送精修工坊'}</span>
                </button>
                <button
                  type="button"
                  disabled={!!splitBusy}
                  onclick={() => handleSplitSelectedLayer('lr')}
                  class="flex items-center justify-center gap-1 rounded-md border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent disabled:opacity-50"
                  title={isEn ? 'Split connected layer horizontally into Left/Right parts' : '将该层按左右连通域/中缝拆分为左右两个图层（如双腿、双翼）'}
                >
                  <FlipHorizontal2 size={12} />
                  <span>{splitBusy === 'lr' ? '...' : isEn ? 'Split L/R' : '左右拆分'}</span>
                </button>
                <button
                  type="button"
                  disabled={!!splitBusy || !selectedLayer.depthPath}
                  onclick={() => handleSplitSelectedLayer('depth')}
                  class="flex items-center justify-center gap-1 rounded-md border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent disabled:opacity-45"
                  title={isEn ? 'Split front/back sub-layers via pseudo-depth clustering' : '按伪深度聚类将该层切分为前景/后景两层（如斗篷与内搭）'}
                >
                  <Layers2 size={12} />
                  <span>{splitBusy === 'depth' ? '...' : isEn ? 'Split Depth' : '深度二拆'}</span>
                </button>
              </div>
            </div>
          {/if}

          <div class="grid grid-cols-2 gap-2">
            <button
              type="button"
              disabled={!currentJob || layers.length === 0 || isExporting}
              onclick={handleExportPsd}
              class="flex items-center justify-center gap-1.5 rounded-xl bg-accent py-2 text-xs font-bold text-accent-ink shadow-sm transition-opacity hover:opacity-90 disabled:opacity-45"
            >
              <FileOutput size={14} />
              <span>{isEn ? 'Export .PSD' : '导出分层 PSD'}</span>
            </button>
            <button
              type="button"
              disabled={!currentJob || layers.length === 0 || isExporting}
              onclick={handleExportPartsBundle}
              class="flex items-center justify-center gap-1.5 rounded-xl border border-line bg-ink-2 py-2 text-xs font-semibold text-fg transition-colors hover:border-accent disabled:opacity-45"
            >
              <PackageOpen size={14} />
              <span>{isEn ? 'Parts + JSON' : '散件 + JSON'}</span>
            </button>
          </div>
        </div>
      </aside>
    </div>
  {/if}
</div>

<style>
  .checker-bg {
    background-color: #181a20;
    background-image: linear-gradient(45deg, #23262f 25%, transparent 25%),
      linear-gradient(-45deg, #23262f 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #23262f 75%),
      linear-gradient(-45deg, transparent 75%, #23262f 75%);
    background-size: 24px 24px;
    background-position:
      0 0,
      0 12px,
      12px -12px,
      -12px 0;
  }
</style>
