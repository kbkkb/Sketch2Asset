<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    Users,
    Shirt,
    Film,
    Smile,
    Sparkles,
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Upload,
    Download,
    Save,
    Loader2,
    RefreshCw,
    Sliders,
    FolderOpen,
    X,
    Columns3,
    Wand2,
    ShieldCheck,
    PlusCircle,
    ZoomIn,
    ZoomOut,
    RotateCcw
  } from '@lucide/svelte';
  import { api, assetUrl } from '$lib/api';
  import type { AssetItem } from '$lib/api/types';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n } from '$lib/i18n/index.svelte';
  import { loadPersistedState, savePersistedState } from '$lib/stores/workspace';
  import ProviderModelPicker from '$lib/components/ui/ProviderModelPicker.svelte';
  import { STYLE_PRESETS } from '$features/generate/presets';
  import {
    CHARACTER_MODULES,
    SPRITE_ACTION_PRESETS,
    buildSafeGridGuideDataUrl,
    type CharacterModuleId
  } from './modules';
  import {
    detectTurnaroundSplits,
    sliceTurnaroundViews,
    sliceAndAlignSpriteSheet,
    composeAlignedSpriteSheet,
    type TurnaroundSliceItem,
    type SpriteFrameItem,
    type FrameTweak
  } from '$lib/imaging/spritesheet';

  // ============================================================================
  // Modular Hub Navigation State (Extensible Registry Architecture)
  // ============================================================================
  let activeModule = $state<CharacterModuleId>('spritesheet');
  let isGenerating = $state(false);
  let selectedStyle = $state('anime');
  let charDraftHydrated = $state(false);

  // Shared Filmstrip Gallery (Read-only asset picker; each module has its OWN independent slots!)
  let charProjectFilter = $state<string>('_all');
  let galleryAssets = $state<AssetItem[]>([]);

  // ============================================================================
  // MODULE 1: Turnaround & Skinning Studio (三视图与角色换肤 — 独立状态)
  // ============================================================================
  let designTab = $state<'turnaround' | 'skin'>('turnaround');
  let designAnchorDataUrl = $state<string>('');
  let designAnchorLabel = $state<string>('');
  let designAnchorAssetId = $state<string>('');
  let designAnchorInputEl = $state<HTMLInputElement | null>(null);

  // Sub-panel 1A: Turnaround Sheet (Clean prompt with virtual placeholder)
  let turnaroundLayout = $state<'3view_horiz' | 'single_portrait'>('3view_horiz');
  let turnaroundPrompt = $state('');
  let activeTurnaroundTags = $state<string[]>(['apose', 'consistent_ratio', 'clean_layer']);
  let turnaroundCanvas: HTMLCanvasElement | null = null;
  let turnaroundDataUrl = $state<string>('');
  let turnaroundWidth = $state(0);
  let turnaroundHeight = $state(0);
  let split1 = $state(0.3333);
  let split2 = $state(0.6667);
  let showSplitGuides = $state(true);
  let autoCutoutSlices = $state(true);
  let slicedViews = $state<TurnaroundSliceItem[]>([]);
  let isSavingSlices = $state(false);

  const TURNAROUND_MODIFIERS = [
    {
      id: 'apose',
      label: '标准 A-Pose 站姿',
      labelEn: 'Standard A-Pose',
      snippet: 'symmetrical A-pose standing posture, arms slightly angled at 30 degrees, clear limb separation'
    },
    {
      id: 'tpose',
      label: '绑定 T-Pose 站姿',
      labelEn: 'Rigging T-Pose',
      snippet: 'strictly horizontal T-pose arms for 3D/2D rigging reference'
    },
    {
      id: 'consistent_ratio',
      label: '严格对齐头身比基线',
      labelEn: 'Aligned Head/Foot Baselines',
      snippet: 'strictly aligned head-top and foot-bottom horizontal reference baselines across all views'
    },
    {
      id: 'chibi',
      label: 'Q版二头身比例 (Chibi)',
      labelEn: '2-Head Chibi Ratio',
      snippet: '2-head-tall cute chibi game character proportion, expressive oversized head'
    },
    {
      id: 'clean_layer',
      label: '分层服饰结构清晰',
      labelEn: 'Clean Layered Costume',
      snippet: 'clear costume material separation, distinct armor, cloth, belt, and accessory silhouettes'
    },
    {
      id: 'pixel_strict',
      label: '严格等大像素格 (Pixel)',
      labelEn: 'Strict 1:1 Pixel Grid',
      snippet: 'strictly uniform 1:1 square pixel grid, zero mixels, crisp 1px dark outline'
    }
  ];

  // Sub-panel 1B: Skin / Outfit Variant (Clean prompt with virtual placeholder)
  let skinPrompt = $state('');
  let skinLockMode = $state<'keep_layout' | 'new_turnaround' | 'new_portrait'>('keep_layout');
  let skinExtraOutfitRefDataUrl = $state<string>('');
  let skinResultDataUrl = $state<string>('');
  let skinResultAsset = $state<AssetItem | null>(null);
  let skinCompareSideBySide = $state(true);

  const SKIN_PRESET_THEMES = [
    {
      label: '新手冒险者皮甲',
      labelEn: 'Adventurer Leather Armor',
      text: '轻便的棕色皮甲与麻布披风、腰挂小型生命药水与旅行包',
      textEn: 'Light brown leather armor with linen cloak, belt-hung health potion and travel pouch'
    },
    {
      label: '皇家圣骑士金甲',
      labelEn: 'Royal Paladin Gold Armor',
      text: '华丽的纯白与暗金雕花圣骑士全身重甲、蓝宝石肩甲与神圣披风',
      textEn: 'Ornate white and dark-gold engraved paladin full plate armor, sapphire pauldrons and holy cape'
    },
    {
      label: '暗影刺客夜行衣',
      labelEn: 'Shadow Assassin Attire',
      text: '修身的玄黑潜行皮衣、暗紫发光符文护腕、兜帽与双匕首挂饰',
      textEn: 'Sleek obsidian stealth leather suit, glowing purple rune bracers, hood and dual daggers'
    },
    {
      label: '赛博霓虹机能服',
      labelEn: 'Cyberpunk Techwear',
      text: '赛博朋克高科技战术机能风衣、青色霓虹导光条、半透明战术护目镜',
      textEn: 'High-tech cyberpunk tactical trench coat, cyan neon light strips, translucent tactical visor'
    },
    {
      label: '东方仙侠流云袍',
      labelEn: 'Xianxia Cloud Robe',
      text: '飘逸的青白渐变仙侠广袖长袍、金色云纹刺绣、腰间玉佩流苏',
      textEn: 'Flowing cyan-white gradient immortal wide-sleeve robe, gold cloud embroidery, jade pendant tassel'
    },
    {
      label: '重装机甲外骨骼',
      labelEn: 'Heavy Mecha Exoskeleton',
      text: '硬核科幻动力外骨骼装甲、蜂巢能量核心护胸、机械关节护腿',
      textEn: 'Hardcore sci-fi powered exoskeleton armor, honeycomb energy core chestplate, mechanical greaves'
    }
  ];

  // ============================================================================
  // MODULE 2: Sprite Sheet & Pivot Animation Studio (序列帧动画工坊 — 完全独立状态!)
  // ============================================================================
  // Slot A: 序列帧生图专用的「角色外观参考图」
  let spriteCharRefDataUrl = $state<string>('');
  let spriteCharRefLabel = $state<string>('');
  let spriteCharRefAssetId = $state<string>('');
  let spriteCharRefInputEl = $state<HTMLInputElement | null>(null);

  // Slot B: 待切帧与动画预览的「序列帧网格大图」
  let spriteSheetCanvas: HTMLCanvasElement | null = null;
  let spriteSheetDataUrl = $state<string>('');
  let spriteSheetAssetId = $state<string>('');
  let sheetFileInputEl = $state<HTMLInputElement | null>(null);

  // Action Preset (Default is 'default' —自由模板，避免被内置动作提示词污染!)
  let actionPreset = $state('default');
  let gridPreset = $state('2x4');
  let sheetRows = $state(2);
  let sheetCols = $state(4);
  let spriteExtraPrompt = $state('');
  let injectSafeGridGuide = $state(true);

  // Anti-Out-of-Bounds Slicing & Pivot Alignment Options
  let spriteRemoveBg = $state(true);
  let spriteSmartIsland = $state(true);
  let spritePreventClip = $state(true);
  let spriteAlignPivot = $state(true);
  let spritePixelClean = $state(false);
  let spriteBaselineRatio = $state(0.86);
  let frameTweaks = $state<FrameTweak[]>([]);
  let spriteFrames = $state<SpriteFrameItem[]>([]);

  // Animation Player + Wheel Zoom & Drag Pan state
  let isPlaying = $state(true);
  let fps = $state(8);
  let pingPongLoop = $state(false);
  let onionSkin = $state(false);
  let showPivotGuide = $state(true);
  let previewBackdrop = $state<'checker' | 'dark' | 'light'>('checker');
  let currentFrameIdx = $state(0);
  let animDirection = 1;
  let animTimer: ReturnType<typeof setInterval> | null = null;

  let previewZoom = $state(1);
  let previewPanX = $state(0);
  let previewPanY = $state(0);
  let previewDragging = $state(false);
  let previewDragStart = { x: 0, y: 0, px: 0, py: 0 };

  // ============================================================================
  // MODULE 3: Expression Sheet Studio (角色表情差分集 — 独立状态)
  // ============================================================================
  let exprCharRefDataUrl = $state<string>('');
  let exprCharRefLabel = $state<string>('');
  let exprGrid = $state<'2x2' | '2x3'>('2x3');
  let exprPrompt = $state('');
  let exprSheetCanvas: HTMLCanvasElement | null = null;
  let exprSheetDataUrl = $state<string>('');
  let exprFrames = $state<SpriteFrameItem[]>([]);

  let currentActionPresetObj = $derived(
    SPRITE_ACTION_PRESETS.find((a) => a.id === actionPreset) ?? SPRITE_ACTION_PRESETS[0]!
  );

  let enabledFrames = $derived(
    spriteFrames.filter((_, idx) => frameTweaks[idx]?.enabled ?? true)
  );
  let activeDisplayFrame = $derived(
    enabledFrames[currentFrameIdx % Math.max(1, enabledFrames.length)] ?? spriteFrames[0] ?? null
  );
  let prevOnionFrame = $derived(
    enabledFrames.length > 1
      ? enabledFrames[(currentFrameIdx - 1 + enabledFrames.length) % enabledFrames.length] ?? null
      : null
  );

  function getStylePrompt(): string {
    const s = STYLE_PRESETS.find((x) => x.id === selectedStyle);
    return s?.promptSuffix ?? '';
  }

  function handlePreviewWheel(e: WheelEvent) {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.18 : 1 / 1.18;
    const nextZoom = Math.min(8, Math.max(0.4, Number((previewZoom * factor).toFixed(2))));
    previewZoom = nextZoom;
  }

  function handlePreviewPointerDown(e: PointerEvent) {
    if (e.button !== 0 && e.button !== 1) return;
    previewDragging = true;
    previewDragStart = {
      x: e.clientX,
      y: e.clientY,
      px: previewPanX,
      py: previewPanY
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handlePreviewPointerMove(e: PointerEvent) {
    if (!previewDragging) return;
    previewPanX = previewDragStart.px + (e.clientX - previewDragStart.x);
    previewPanY = previewDragStart.py + (e.clientY - previewDragStart.y);
  }

  function handlePreviewPointerUp() {
    previewDragging = false;
  }

  function resetPreviewTransform() {
    previewZoom = 1;
    previewPanX = 0;
    previewPanY = 0;
  }

  onMount(async () => {
    const saved = loadPersistedState('character', {
      activeModule: 'spritesheet' as CharacterModuleId,
      designTab: 'turnaround' as 'turnaround' | 'skin',
      selectedStyle: 'anime',
      designAnchorAssetId: '',
      turnaroundLayout: '3view_horiz' as '3view_horiz' | 'single_portrait',
      turnaroundPrompt: '',
      activeTurnaroundTags: ['apose', 'consistent_ratio', 'clean_layer'] as string[],
      split1: 0.3333,
      split2: 0.6667,
      showSplitGuides: true,
      autoCutoutSlices: true,
      skinPrompt: '',
      skinLockMode: 'keep_layout' as 'keep_layout' | 'new_turnaround' | 'new_portrait',
      skinCompareSideBySide: true,
      spriteCharRefAssetId: '',
      spriteSheetAssetId: '',
      actionPreset: 'default',
      gridPreset: '2x4',
      sheetRows: 2,
      sheetCols: 4,
      spriteExtraPrompt: '',
      injectSafeGridGuide: true,
      spriteRemoveBg: true,
      spriteSmartIsland: true,
      spritePreventClip: true,
      spriteAlignPivot: true,
      spritePixelClean: false,
      spriteBaselineRatio: 0.86,
      fps: 8,
      pingPongLoop: false,
      onionSkin: false,
      showPivotGuide: true,
      previewBackdrop: 'checker' as 'checker' | 'dark' | 'light',
      exprGrid: '2x3' as '2x2' | '2x3',
      exprPrompt: ''
    });

    activeModule = saved.activeModule || 'spritesheet';
    designTab = saved.designTab || 'turnaround';
    selectedStyle = saved.selectedStyle || 'anime';
    turnaroundLayout = saved.turnaroundLayout || '3view_horiz';
    turnaroundPrompt = saved.turnaroundPrompt || '';
    activeTurnaroundTags = Array.isArray(saved.activeTurnaroundTags)
      ? saved.activeTurnaroundTags
      : ['apose', 'consistent_ratio', 'clean_layer'];
    split1 = saved.split1 ?? 0.3333;
    split2 = saved.split2 ?? 0.6667;
    showSplitGuides = saved.showSplitGuides ?? true;
    autoCutoutSlices = saved.autoCutoutSlices ?? true;
    skinPrompt = saved.skinPrompt || '';
    skinLockMode = saved.skinLockMode || 'keep_layout';
    skinCompareSideBySide = saved.skinCompareSideBySide ?? true;

    actionPreset = saved.actionPreset || 'default';
    gridPreset = saved.gridPreset || '2x4';
    sheetRows = saved.sheetRows ?? 2;
    sheetCols = saved.sheetCols ?? 4;
    spriteExtraPrompt = saved.spriteExtraPrompt || '';
    injectSafeGridGuide = saved.injectSafeGridGuide ?? true;
    spriteRemoveBg = saved.spriteRemoveBg ?? true;
    spriteSmartIsland = saved.spriteSmartIsland ?? true;
    spritePreventClip = saved.spritePreventClip ?? true;
    spriteAlignPivot = saved.spriteAlignPivot ?? true;
    spritePixelClean = saved.spritePixelClean ?? false;
    spriteBaselineRatio = saved.spriteBaselineRatio ?? 0.86;
    fps = saved.fps ?? 8;
    pingPongLoop = saved.pingPongLoop ?? false;
    onionSkin = saved.onionSkin ?? false;
    showPivotGuide = saved.showPivotGuide ?? true;
    previewBackdrop = saved.previewBackdrop || 'checker';
    exprGrid = saved.exprGrid || '2x3';
    exprPrompt = saved.exprPrompt || '';

    if (appStore.settings?.activeProject) {
      charProjectFilter = appStore.settings.activeProject;
    }
    await refreshGallery();

    if (appStore.characterPendingAsset) {
      const pending = appStore.characterPendingAsset;
      appStore.characterPendingAsset = null;
      await routeAssetFromExternal(pending, appStore.characterSubMode);
    } else if (galleryAssets.length > 0) {
      const designItem = saved.designAnchorAssetId
        ? galleryAssets.find((a) => a.id === saved.designAnchorAssetId)
        : null;
      if (designItem) {
        await setDesignAnchorFromAsset(designItem, false);
      }

      const spriteRefItem = saved.spriteCharRefAssetId
        ? galleryAssets.find((a) => a.id === saved.spriteCharRefAssetId)
        : null;
      if (spriteRefItem) {
        await setSpriteCharRefFromAsset(spriteRefItem, false);
      }

      const sheetItem = saved.spriteSheetAssetId
        ? galleryAssets.find((a) => a.id === saved.spriteSheetAssetId)
        : galleryAssets.find((a) => a.archetype === 'sprite_sheet') ?? galleryAssets[0] ?? null;
      if (sheetItem) {
        await loadSpriteSheetAsset(sheetItem, false);
      }
    }

    charDraftHydrated = true;
    restartAnimTimer();
  });

  onDestroy(() => {
    if (animTimer) clearInterval(animTimer);
  });

  $effect(() => {
    if (appStore.characterPendingAsset) {
      const pending = appStore.characterPendingAsset;
      appStore.characterPendingAsset = null;
      routeAssetFromExternal(pending, appStore.characterSubMode);
    }
  });

  $effect(() => {
    // 监听视图切入或全局资产版本变动，确保角色工坊底栏资产实时刷新
    const isChar = appStore.activeView === 'character';
    const _rev = appStore.assetsRevision;
    if (isChar && charDraftHydrated) {
      refreshGallery();
    }
  });

  $effect(() => {
    const _fps = fps;
    const _playing = isPlaying;
    restartAnimTimer();
    return () => {
      if (animTimer) clearInterval(animTimer);
    };
  });

  $effect(() => {
    if (!charDraftHydrated) return;
    savePersistedState('character', {
      activeModule,
      designTab,
      selectedStyle,
      designAnchorAssetId,
      turnaroundLayout,
      turnaroundPrompt,
      activeTurnaroundTags,
      split1,
      split2,
      showSplitGuides,
      autoCutoutSlices,
      skinPrompt,
      skinLockMode,
      skinCompareSideBySide,
      spriteCharRefAssetId,
      spriteSheetAssetId,
      actionPreset,
      gridPreset,
      sheetRows,
      sheetCols,
      spriteExtraPrompt,
      injectSafeGridGuide,
      spriteRemoveBg,
      spriteSmartIsland,
      spritePreventClip,
      spriteAlignPivot,
      spritePixelClean,
      spriteBaselineRatio,
      fps,
      pingPongLoop,
      onionSkin,
      showPivotGuide,
      previewBackdrop,
      exprGrid,
      exprPrompt
    });
  });

  async function routeAssetFromExternal(
    asset: AssetItem,
    mode: 'turnaround' | 'outfit' | 'spritesheet'
  ) {
    if (mode === 'spritesheet') {
      activeModule = 'spritesheet';
      if (asset.archetype === 'sprite_sheet') {
        await loadSpriteSheetAsset(asset, true);
      } else {
        await setSpriteCharRefFromAsset(asset, true);
      }
    } else {
      activeModule = 'turnaround_skin';
      designTab = mode === 'outfit' ? 'skin' : 'turnaround';
      await setDesignAnchorFromAsset(asset, true);
    }
  }

  function restartAnimTimer() {
    if (animTimer) clearInterval(animTimer);
    if (!isPlaying) return;
    const intervalMs = Math.max(30, Math.round(1000 / Math.max(1, fps)));
    animTimer = setInterval(() => {
      const total = enabledFrames.length;
      if (total <= 1) {
        currentFrameIdx = 0;
        return;
      }
      if (pingPongLoop) {
        let next = currentFrameIdx + animDirection;
        if (next >= total) {
          animDirection = -1;
          next = Math.max(0, total - 2);
        } else if (next < 0) {
          animDirection = 1;
          next = Math.min(total - 1, 1);
        }
        currentFrameIdx = next;
      } else {
        currentFrameIdx = (currentFrameIdx + 1) % total;
      }
    }, intervalMs);
  }

  async function refreshGallery() {
    try {
      const proj = charProjectFilter === '_all' ? undefined : charProjectFilter;
      galleryAssets = await api.listAssets(proj, 80);
    } catch {
      // ignore
    }
  }

  function loadHtmlImage(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error('Failed to decode image'));
      img.src = src;
    });
  }

  function getSliceViewLabel(item: TurnaroundSliceItem): string {
    return item.key === 'front'
      ? i18n.t('char_view_front')
      : item.key === 'side'
        ? i18n.t('char_view_side')
        : i18n.t('char_view_back');
  }

  // ============================================================================
  // Independent Asset Loaders for Module 1, Module 2, Module 3
  // ============================================================================
  async function setDesignAnchorFromAsset(asset: AssetItem, showToast = true) {
    try {
      const durl = await api.readImageDataUrl(asset.filePath);
      designAnchorDataUrl = durl;
      designAnchorLabel = asset.prompt || asset.id;
      designAnchorAssetId = asset.id;
      await loadTurnaroundFromDataUrl(durl);
      if (showToast) {
        appStore.toast(
          'ok',
          i18n.locale === 'en'
            ? 'Loaded as Turnaround & Skinning anchor character'
            : '已载入「三视图与换肤工坊」设为基准角色'
        );
      }
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to load character image' : '加载角色图像失败')
      );
    }
  }

  async function setSpriteCharRefFromAsset(asset: AssetItem, showToast = true) {
    try {
      const durl = await api.readImageDataUrl(asset.filePath);
      spriteCharRefDataUrl = durl;
      spriteCharRefLabel = asset.prompt || asset.id;
      spriteCharRefAssetId = asset.id;
      if (showToast) {
        appStore.toast(
          'ok',
          i18n.locale === 'en'
            ? 'Set as Sprite Sheet character reference'
            : '已设为「序列帧生图」的角色外观参考图'
        );
      }
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to load reference image' : '加载参考图失败')
      );
    }
  }

  async function loadSpriteSheetAsset(asset: AssetItem, showToast = true) {
    try {
      const durl = await api.readImageDataUrl(asset.filePath);
      spriteSheetAssetId = asset.id;
      await loadSpriteSheetFromDataUrl(durl);
      if (showToast) {
        appStore.toast(
          'ok',
          i18n.locale === 'en'
            ? 'Loaded sprite sheet and applied smart anti-clip slicing'
            : '已载入序列帧大图并执行防出界智能切帧'
        );
      }
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to load sprite sheet' : '加载序列帧大图失败')
      );
    }
  }

  async function setExprCharRefFromAsset(asset: AssetItem, showToast = true) {
    try {
      const durl = await api.readImageDataUrl(asset.filePath);
      exprCharRefDataUrl = durl;
      exprCharRefLabel = asset.prompt || asset.id;
      if (showToast) {
        appStore.toast(
          'ok',
          i18n.locale === 'en'
            ? 'Set as Expression Sheet character reference'
            : '已设为「表情差分」角色参考图'
        );
      }
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to load reference image' : '加载参考图失败')
      );
    }
  }

  // ============================================================================
  // Module 1 Logic: Turnaround & Skinning
  // ============================================================================
  async function loadTurnaroundFromDataUrl(durl: string) {
    const im = await loadHtmlImage(durl);
    const cv = document.createElement('canvas');
    cv.width = im.naturalWidth || im.width;
    cv.height = im.naturalHeight || im.height;
    cv.getContext('2d', { willReadFrequently: true })!.drawImage(im, 0, 0);

    turnaroundCanvas = cv;
    turnaroundDataUrl = durl;
    turnaroundWidth = cv.width;
    turnaroundHeight = cv.height;

    const [s1, s2] = detectTurnaroundSplits(cv);
    split1 = s1;
    split2 = s2;
    rebuildTurnaroundSlices();
  }

  function rebuildTurnaroundSlices() {
    if (!turnaroundCanvas) {
      slicedViews = [];
      return;
    }
    slicedViews = sliceTurnaroundViews(turnaroundCanvas, split1, split2, autoCutoutSlices, 34);
  }

  function handleAutoDetectSplits() {
    if (!turnaroundCanvas) return;
    const [s1, s2] = detectTurnaroundSplits(turnaroundCanvas);
    split1 = s1;
    split2 = s2;
    rebuildTurnaroundSlices();
    appStore.toast(
      'ok',
      i18n.locale === 'en'
        ? 'Automatically detected Front / Side / Back split guides'
        : '已自动识别正 / 侧 / 背空白分界线'
    );
  }

  async function handleSaveTurnaroundSlicesToLib() {
    if (slicedViews.length === 0 || isSavingSlices) return;
    isSavingSlices = true;
    try {
      const targetProj =
        charProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : charProjectFilter;
      let firstSaved: AssetItem | null = null;
      for (const item of slicedViews) {
        const viewLabel = getSliceViewLabel(item);
        const saved = await api.saveStudioAsset(
          item.dataUrl,
          targetProj,
          'character_card',
          `${turnaroundPrompt.trim() || 'Character'} [${viewLabel}]`
        );
        if (!firstSaved) firstSaved = saved;
      }
      await Promise.all([refreshGallery(), appStore.refreshProjects()]);
      appStore.notifyAssetsChanged();
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? 'Saved Front / Side / Back sliced views to project library'
          : '已将正 / 侧 / 背三张独立透明底视图存入项目资产库'
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to save sliced views' : '保存切分视图失败')
      );
    } finally {
      isSavingSlices = false;
    }
  }

  async function handleExportSingleView(item: TurnaroundSliceItem) {
    try {
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const outPath = await api.exportStudioPng(
        item.dataUrl,
        `char_${item.key}_${item.width}x${item.height}_${ts}.png`,
        true
      );
      const viewLabel = getSliceViewLabel(item);
      appStore.toast(
        'ok',
        i18n.locale === 'en' ? `Exported ${viewLabel}: ${outPath}` : `已导出 ${viewLabel}: ${outPath}`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Export failed' : '导出失败'));
    }
  }

  function toggleTurnaroundTag(id: string) {
    if (activeTurnaroundTags.includes(id)) {
      activeTurnaroundTags = activeTurnaroundTags.filter((x) => x !== id);
    } else {
      activeTurnaroundTags = [...activeTurnaroundTags, id];
    }
  }

  async function handleGenerateTurnaround() {
    if (isGenerating) return;
    const prof = appStore.activeProfile;
    if (!prof || !prof.hasKey) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please configure a valid API Key in Settings first'
          : '请先在设置中配置有效的生图 API Key'
      );
      appStore.openSettings('providers');
      return;
    }
    if (!turnaroundPrompt.trim() && !designAnchorDataUrl) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please enter a character description or select a reference image first'
          : '请输入角色设定描述，或先选定一张角色参考图'
      );
      return;
    }

    isGenerating = true;
    try {
      const targetProj =
        charProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : charProjectFilter;

      const tagRules = TURNAROUND_MODIFIERS.filter((m) => activeTurnaroundTags.includes(m.id))
        .map((m) => m.snippet)
        .join(', ');

      const layoutRule =
        turnaroundLayout === '3view_horiz'
          ? 'Character turnaround reference sheet, three full-body views of the EXACT SAME character arranged side-by-side horizontally from left to right: [Front View] on the left, [Side Profile View] in the center, and [Back View] on the right. Equal head-to-toe height across all 3 views, generous white margin between views, complete head and feet visible without clipping.'
          : 'Single full-body game character concept design illustration, centered standing pose, complete head-to-toe view with safe margin.';

      const compiledPrompt = [
        layoutRule,
        tagRules,
        turnaroundPrompt.trim() || 'Generate a clean full-body turnaround sheet based on the reference character.'
      ]
        .filter(Boolean)
        .join('\n');

      const res = await api.generateCharacterAsset({
        profileId: prof.id,
        model: prof.apiModel,
        subMode: 'turnaround',
        prompt: compiledPrompt,
        stylePrompt: getStylePrompt(),
        size: turnaroundLayout === '3view_horiz' ? '1536x1024' : '1024x1024',
        project: targetProj,
        archetype: 'character_card',
        anchorDataUrl: designAnchorDataUrl || null,
        secondaryRefDataUrl: null
      });

      await Promise.all([refreshGallery(), appStore.refreshProjects()]);
      appStore.notifyAssetsChanged();
      designAnchorDataUrl = res.imageDataUrl;
      designAnchorLabel =
        turnaroundPrompt.trim() || (i18n.locale === 'en' ? 'Character Turnaround' : '角色三视图设定');
      designAnchorAssetId = res.asset.id;
      await loadTurnaroundFromDataUrl(res.imageDataUrl);
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? 'Character turnaround generated and auto-split into Front / Side / Back'
          : '角色三视图设定生成完成，已自动识别正/侧/背切分线'
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Turnaround generation failed' : '三视图生成失败')
      );
    } finally {
      isGenerating = false;
    }
  }

  async function handleGenerateSkinVariant() {
    if (isGenerating) return;
    const prof = appStore.activeProfile;
    if (!prof || !prof.hasKey) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please configure a valid API Key in Settings first'
          : '请先在设置中配置有效的生图 API Key'
      );
      appStore.openSettings('providers');
      return;
    }
    if (!designAnchorDataUrl) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please select an Identity Anchor character from the filmstrip or upload one first'
          : '请先在上方或从底部资产栏选定一张「基准角色图」作为换肤锚点'
      );
      return;
    }
    if (!skinPrompt.trim() && !skinExtraOutfitRefDataUrl) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please enter a new outfit description or select a skin theme preset'
          : '请输入新服装/皮肤描述，或点选上方快捷换肤主题'
      );
      return;
    }

    isGenerating = true;
    try {
      const targetProj =
        charProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : charProjectFilter;

      const targetSize =
        skinLockMode === 'new_turnaround'
          ? '1536x1024'
          : skinLockMode === 'new_portrait'
            ? '1024x1024'
            : turnaroundWidth > turnaroundHeight * 1.2
              ? '1536x1024'
              : '1024x1024';

      const layoutConstraint =
        skinLockMode === 'keep_layout'
          ? 'Strictly preserve the exact layout, camera angle, pose, and view arrangement from Image 1.'
          : skinLockMode === 'new_turnaround'
            ? 'Arrange as a horizontal 3-view character turnaround sheet (Front, Side, Back views side-by-side).'
            : 'Render as a single full-body standing character concept portrait.';

      const compiledPrompt = [
        'CHARACTER IDENTITY LOCK: Strictly preserve the character identity from Image 1 — keep the exact same face, facial features, hairstyle, hair color, body proportions, head-to-body ratio, and art style.',
        layoutConstraint,
        `OUTFIT / SKIN REPLACEMENT: Replace ONLY the character's outfit, armor, and accessories with: ${skinPrompt.trim() || 'the outfit design shown in Image 2'}.`
      ].join('\n');

      const res = await api.generateCharacterAsset({
        profileId: prof.id,
        model: prof.apiModel,
        subMode: 'outfit',
        prompt: compiledPrompt,
        stylePrompt: getStylePrompt(),
        size: targetSize,
        project: targetProj,
        archetype: 'character_card',
        anchorDataUrl: designAnchorDataUrl,
        secondaryRefDataUrl: skinExtraOutfitRefDataUrl || null
      });

      await Promise.all([refreshGallery(), appStore.refreshProjects()]);
      appStore.notifyAssetsChanged();
      skinResultAsset = res.asset;
      skinResultDataUrl = res.imageDataUrl;
      appStore.toast(
        'ok',
        i18n.locale === 'en' ? 'Identity-locked skin variant generated' : '角色同模换肤派生完成'
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Skin variant generation failed' : '换肤派生生成失败')
      );
    } finally {
      isGenerating = false;
    }
  }

  // ============================================================================
  // Module 2 Logic: Sprite Sheet & Anti-Out-of-Bounds Slicer
  // ============================================================================
  function handleGridPresetChange(preset: string) {
    gridPreset = preset;
    if (preset === '1x4') {
      sheetRows = 1;
      sheetCols = 4;
    } else if (preset === '1x6') {
      sheetRows = 1;
      sheetCols = 6;
    } else if (preset === '2x2') {
      sheetRows = 2;
      sheetCols = 2;
    } else if (preset === '2x3') {
      sheetRows = 2;
      sheetCols = 3;
    } else if (preset === '2x4') {
      sheetRows = 2;
      sheetCols = 4;
    } else if (preset === '3x3') {
      sheetRows = 3;
      sheetCols = 3;
    }
    rebuildSpriteFrames();
  }

  async function loadSpriteSheetFromDataUrl(durl: string) {
    const im = await loadHtmlImage(durl);
    const cv = document.createElement('canvas');
    cv.width = im.naturalWidth || im.width;
    cv.height = im.naturalHeight || im.height;
    cv.getContext('2d', { willReadFrequently: true })!.drawImage(im, 0, 0);

    spriteSheetCanvas = cv;
    spriteSheetDataUrl = durl;
    frameTweaks = Array.from({ length: sheetRows * sheetCols }, () => ({
      dx: 0,
      dy: 0,
      enabled: true,
      expandLeft: 0,
      expandRight: 0,
      expandTop: 0,
      expandBottom: 0
    }));
    rebuildSpriteFrames();
  }

  function rebuildSpriteFrames() {
    if (!spriteSheetCanvas) {
      spriteFrames = [];
      return;
    }
    const total = sheetRows * sheetCols;
    if (frameTweaks.length !== total) {
      frameTweaks = Array.from({ length: total }, (_, i) =>
        frameTweaks[i] ?? {
          dx: 0,
          dy: 0,
          enabled: true,
          expandLeft: 0,
          expandRight: 0,
          expandTop: 0,
          expandBottom: 0
        }
      );
    }
    spriteFrames = sliceAndAlignSpriteSheet(spriteSheetCanvas, {
      rows: sheetRows,
      cols: sheetCols,
      removeBg: spriteRemoveBg,
      bgTolerance: 34,
      alignPivot: spriteAlignPivot,
      baselineRatio: spriteBaselineRatio,
      pixelClean: spritePixelClean,
      smartIslandSlice: spriteSmartIsland,
      preventClipping: spritePreventClip,
      frameTweaks
    });
    if (currentFrameIdx >= enabledFrames.length) {
      currentFrameIdx = 0;
    }
  }

  function nudgeCurrentFrame(dx: number, dy: number) {
    const active = activeDisplayFrame;
    if (!active) return;
    const idx = active.index;
    frameTweaks = frameTweaks.map((t, i) =>
      i === idx ? { ...t, dx: t.dx + dx, dy: t.dy + dy } : t
    );
    rebuildSpriteFrames();
  }

  function resetCurrentFramePos() {
    const active = activeDisplayFrame;
    if (!active) return;
    const idx = active.index;
    frameTweaks = frameTweaks.map((t, i) =>
      i === idx ? { ...t, dx: 0, dy: 0 } : t
    );
    rebuildSpriteFrames();
  }

  function adjustCurrentFrameCrop(side: 'left' | 'right' | 'top' | 'bottom', delta: number) {
    const active = activeDisplayFrame;
    if (!active) return;
    const idx = active.index;
    frameTweaks = frameTweaks.map((t, i) => {
      if (i !== idx) return t;
      const curL = t.expandLeft ?? 0;
      const curR = t.expandRight ?? 0;
      const curT = t.expandTop ?? 0;
      const curB = t.expandBottom ?? 0;
      return {
        ...t,
        expandLeft: side === 'left' ? Math.max(-200, Math.min(300, curL + delta)) : curL,
        expandRight: side === 'right' ? Math.max(-200, Math.min(300, curR + delta)) : curR,
        expandTop: side === 'top' ? Math.max(-200, Math.min(300, curT + delta)) : curT,
        expandBottom: side === 'bottom' ? Math.max(-200, Math.min(300, curB + delta)) : curB
      };
    });
    rebuildSpriteFrames();
  }

  function resetCurrentFrameCrop() {
    const active = activeDisplayFrame;
    if (!active) return;
    const idx = active.index;
    frameTweaks = frameTweaks.map((t, i) =>
      i === idx
        ? {
            ...t,
            expandLeft: 0,
            expandRight: 0,
            expandTop: 0,
            expandBottom: 0
          }
        : t
    );
    rebuildSpriteFrames();
  }

  function toggleFrameEnabled(idx: number) {
    frameTweaks = frameTweaks.map((t, i) =>
      i === idx ? { ...t, enabled: !t.enabled } : t
    );
  }

  async function handleGenerateSpriteSheet() {
    if (isGenerating) return;
    const prof = appStore.activeProfile;
    if (!prof || !prof.hasKey) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please configure a valid API Key in Settings first'
          : '请先在设置中配置有效的生图 API Key'
      );
      appStore.openSettings('providers');
      return;
    }
    if (actionPreset === 'default' && !spriteExtraPrompt.trim()) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Default Template selected: please enter your character & action description below'
          : '当前为「默认模板」，请在下方提示词框输入想要生成的角色与动作'
      );
      return;
    }
    if (!spriteCharRefDataUrl && !spriteExtraPrompt.trim()) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please select a character reference or enter a character & action description'
          : '请选定一张「角色外观参考图」或在提示词框输入角色与动作描述'
      );
      return;
    }

    isGenerating = true;
    try {
      const targetProj =
        charProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : charProjectFilter;

      const totalFrames = sheetRows * sheetCols;
      const sheetSize = sheetCols > sheetRows ? '1536x1024' : '1024x1024';
      const [targetW, targetH] = sheetSize === '1536x1024' ? [1536, 1024] : [1024, 1024];

      let primaryRef: string | null = spriteCharRefDataUrl || null;
      let secondaryRef: string | null = null;
      if (injectSafeGridGuide) {
        const gridGuideUrl = buildSafeGridGuideDataUrl(sheetRows, sheetCols, targetW, targetH);
        if (primaryRef) {
          secondaryRef = gridGuideUrl;
        } else {
          primaryRef = gridGuideUrl;
        }
      }

      // 若用户选择了「默认模板 (default)」，不注入任何内置动作套路，完全按用户输入的提示词描述动作！
      const actionRule = currentActionPresetObj.rule;
      const promptParts = [
        actionRule,
        spriteExtraPrompt.trim() ? `USER ACTION & CHARACTER SPECIFICATION: ${spriteExtraPrompt.trim()}` : '',
        `GRID & SAFE MARGIN RULE: Arrange strictly as a clean ${sheetRows} rows by ${sheetCols} columns grid (${totalFrames} sequential frames total, reading left-to-right, top-to-bottom). Keep each frame's character AND extended weapon/slash effects strictly inside the center 70% safe zone of its own cell with at least 15% white padding around all 4 sides of every cell — NEVER touch the canvas border or cross into adjacent cells! Do NOT draw visible grid lines or numbers.`,
        spriteCharRefDataUrl
          ? 'Strictly keep the exact same character design, proportions, colors, and outfit from the reference character image across all frames.'
          : ''
      ].filter(Boolean);

      const res = await api.generateCharacterAsset({
        profileId: prof.id,
        model: prof.apiModel,
        subMode: 'spritesheet',
        prompt: promptParts.join('\n'),
        stylePrompt: getStylePrompt(),
        size: sheetSize,
        project: targetProj,
        archetype: 'sprite_sheet',
        anchorDataUrl: primaryRef,
        secondaryRefDataUrl: secondaryRef
      });

      await Promise.all([refreshGallery(), appStore.refreshProjects()]);
      appStore.notifyAssetsChanged();
      spriteSheetAssetId = res.asset.id;
      await loadSpriteSheetFromDataUrl(res.imageDataUrl);
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? `Sprite sheet generated & aligned (${enabledFrames.length} frames)`
          : `序列帧生成完成：已自动执行连通域防越界切帧与安全画布校准 (${enabledFrames.length} 帧)`
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Sprite sheet generation failed' : '序列帧生成失败')
      );
    } finally {
      isGenerating = false;
    }
  }

  async function handleExportAlignedSpriteSheet() {
    if (enabledFrames.length === 0) return;
    try {
      const sheetCv = composeAlignedSpriteSheet(enabledFrames, sheetCols);
      const durl = sheetCv.toDataURL('image/png');
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const outPath = await api.exportStudioPng(
        durl,
        `spritesheet_${actionPreset}_${enabledFrames.length}f_${sheetCv.width}x${sheetCv.height}_${ts}.png`,
        true
      );
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? `Exported aligned sprite sheet (${sheetCv.width}×${sheetCv.height}): ${outPath}`
          : `已导出标准对齐精灵表 (${sheetCv.width}×${sheetCv.height}): ${outPath}`
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to export sprite sheet' : '导出精灵表失败')
      );
    }
  }

  async function handleSaveAlignedSheetToLibrary() {
    if (enabledFrames.length === 0) return;
    try {
      const sheetCv = composeAlignedSpriteSheet(enabledFrames, sheetCols);
      const durl = sheetCv.toDataURL('image/png');
      const targetProj =
        charProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : charProjectFilter;
      await api.saveStudioAsset(
        durl,
        targetProj,
        'sprite_sheet',
        `SpriteSheet [${actionPreset} ${enabledFrames.length}F Aligned]`
      );
      await Promise.all([refreshGallery(), appStore.refreshProjects()]);
      appStore.notifyAssetsChanged();
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? 'Saved aligned sprite sheet to project library'
          : '已将防出界校准后的透明底精灵表存入项目资产库'
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Save failed' : '保存失败'));
    }
  }

  async function handleExportCurrentFrame() {
    if (!activeDisplayFrame) return;
    try {
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const outPath = await api.exportStudioPng(
        activeDisplayFrame.dataUrl,
        `frame_${activeDisplayFrame.index + 1}_${activeDisplayFrame.width}x${activeDisplayFrame.height}_${ts}.png`,
        true
      );
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? `Exported frame #${activeDisplayFrame.index + 1}: ${outPath}`
          : `已导出第 #${activeDisplayFrame.index + 1} 帧: ${outPath}`
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to export frame' : '导出单帧失败')
      );
    }
  }

  // ============================================================================
  // Module 3 Logic: Expression Sheet Studio
  // ============================================================================
  async function loadExpressionSheetFromDataUrl(durl: string) {
    const im = await loadHtmlImage(durl);
    const cv = document.createElement('canvas');
    cv.width = im.naturalWidth || im.width;
    cv.height = im.naturalHeight || im.height;
    cv.getContext('2d', { willReadFrequently: true })!.drawImage(im, 0, 0);
    exprSheetCanvas = cv;
    exprSheetDataUrl = durl;
    rebuildExpressionSlices();
  }

  function rebuildExpressionSlices() {
    if (!exprSheetCanvas) {
      exprFrames = [];
      return;
    }
    const [r, c] = exprGrid === '2x2' ? [2, 2] : [2, 3];
    exprFrames = sliceAndAlignSpriteSheet(exprSheetCanvas, {
      rows: r,
      cols: c,
      removeBg: true,
      bgTolerance: 34,
      alignPivot: false,
      pixelClean: false,
      smartIslandSlice: true,
      preventClipping: true
    });
  }

  async function handleGenerateExpressionSheet() {
    if (isGenerating) return;
    const prof = appStore.activeProfile;
    if (!prof || !prof.hasKey) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please configure a valid API Key in Settings first'
          : '请先在设置中配置有效的生图 API Key'
      );
      appStore.openSettings('providers');
      return;
    }
    if (!exprCharRefDataUrl && !exprPrompt.trim()) {
      appStore.toast(
        'err',
        i18n.locale === 'en'
          ? 'Please select a character reference or enter expression descriptions'
          : '请先选定一张角色参考图，或输入角色与表情描述'
      );
      return;
    }

    isGenerating = true;
    try {
      const targetProj =
        charProjectFilter === '_all'
          ? appStore.settings?.activeProject || ''
          : charProjectFilter;

      const [r, c] = exprGrid === '2x2' ? [2, 2] : [2, 3];
      const exprDesc =
        exprPrompt.trim() ||
        (exprGrid === '2x2'
          ? '1. Joyful smile, 2. Angry frown, 3. Sad tearful, 4. Shocked surprised'
          : '1. Joyful smile, 2. Angry frown, 3. Sad tearful, 4. Shocked surprised, 5. Shy blushing, 6. Smug confident');

      const compiled = [
        `2D game character expression sheet arranged as a clean ${r} rows by ${c} columns grid (${r * c} distinct facial expressions of the exact same character bust/head).`,
        'Strictly preserve the character identity, face shape, hairstyle, hair color, and collar outfit across all cells. Keep each bust centered inside its cell with generous white padding and NO grid border lines or text.',
        `Expressions to depict: ${exprDesc}`
      ].join('\n');

      const res = await api.generateCharacterAsset({
        profileId: prof.id,
        model: prof.apiModel,
        subMode: 'turnaround',
        prompt: compiled,
        stylePrompt: getStylePrompt(),
        size: exprGrid === '2x3' ? '1536x1024' : '1024x1024',
        project: targetProj,
        archetype: 'character_card',
        anchorDataUrl: exprCharRefDataUrl || null,
        secondaryRefDataUrl: null
      });

      await Promise.all([refreshGallery(), appStore.refreshProjects()]);
      appStore.notifyAssetsChanged();
      await loadExpressionSheetFromDataUrl(res.imageDataUrl);
      appStore.toast(
        'ok',
        i18n.locale === 'en'
          ? 'Character expression sheet generated and auto-sliced'
          : '角色多表情差分集生成并自动切分完成'
      );
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Expression sheet generation failed' : '表情差分生成失败')
      );
    } finally {
      isGenerating = false;
    }
  }
</script>

<div class="flex h-full w-full overflow-hidden bg-ink-0 select-none">
  <!-- LEFT SIDEBAR: Modular Character Forge Hub -->
  <aside
    class="flex w-60 shrink-0 flex-col justify-between border-r border-line bg-ink-1/95 p-3"
  >
    <div class="flex flex-col gap-3">
      <div class="flex items-center justify-between px-1 pt-0.5">
        <div class="flex items-center gap-2">
          <Users size={15} class="text-accent" />
          <span class="text-xs font-extrabold tracking-wide text-fg">
            {i18n.t('char_hub_title')}
          </span>
        </div>
        <span class="rounded bg-accent/15 px-1.5 py-0.5 font-mono text-[9px] font-bold text-accent">
          MODULAR
        </span>
      </div>

      <div class="flex flex-col gap-2">
        {#each CHARACTER_MODULES as mod (mod.id)}
          {@const isActive = activeModule === mod.id}
          <button
            type="button"
            onclick={() => (activeModule = mod.id)}
            class="group flex flex-col gap-1 rounded-xl border p-2.5 text-left transition-all {isActive
              ? 'border-accent bg-accent/12 shadow-sm'
              : 'border-line bg-ink-0/60 hover:border-fg-muted hover:bg-ink-2/60'}"
          >
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-1.5">
                {#if mod.id === 'turnaround_skin'}
                  <Shirt size={14} class={isActive ? 'text-accent' : 'text-fg-muted'} />
                {:else if mod.id === 'spritesheet'}
                  <Film size={14} class={isActive ? 'text-accent' : 'text-fg-muted'} />
                {:else}
                  <Smile size={14} class={isActive ? 'text-accent' : 'text-fg-muted'} />
                {/if}
                <span class="text-xs font-extrabold {isActive ? 'text-accent' : 'text-fg'}">
                  {i18n.locale === 'en' ? mod.titleEn : mod.title}
                </span>
              </div>
              <span
                class="rounded px-1.5 py-0.5 font-mono text-[9px] font-bold {isActive
                  ? 'bg-accent text-accent-ink'
                  : 'bg-ink-2 text-fg-muted'}"
              >
                {i18n.locale === 'en' ? mod.badgeEn : mod.badge}
              </span>
            </div>
            <p class="text-[10px] leading-relaxed text-fg-muted">
              {i18n.locale === 'en' ? mod.subtitleEn : mod.subtitle}
            </p>
          </button>
        {/each}
      </div>
    </div>

    <!-- Bottom Global Model/Style Card -->
    <div class="flex flex-col gap-2.5 border-t border-line pt-3">
      <div class="flex flex-col gap-1.5">
        <span class="text-[10px] font-bold text-fg-muted">{i18n.t('char_engine_style_label')}</span>
        <ProviderModelPicker />
        <select
          bind:value={selectedStyle}
          class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs font-semibold text-fg outline-none focus:border-accent"
        >
          {#each STYLE_PRESETS as st (st.id)}
            <option value={st.id}>{i18n.t(st.labelKey)}</option>
          {/each}
        </select>
      </div>

      <div class="rounded-lg border border-dashed border-line/80 bg-ink-0/40 px-2.5 py-2 text-[10px] text-fg-dim">
        <div class="flex items-center gap-1.5 font-bold text-fg-muted">
          <PlusCircle size={11} class="text-accent" />
          <span>{i18n.t('char_modular_title')}</span>
        </div>
        <p class="mt-0.5 leading-normal">
          {i18n.t('char_modular_desc')}
        </p>
      </div>
    </div>
  </aside>

  <!-- CENTER & RIGHT: Active Module Workbench + Bottom Asset Filmstrip -->
  <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
    <!-- ====================================================================== -->
    <!-- MODULE 1: Turnaround & Skinning Studio                                 -->
    <!-- ====================================================================== -->
    {#if activeModule === 'turnaround_skin'}
      <div class="flex min-h-0 flex-1 overflow-hidden">
        <div class="flex min-w-0 flex-1 flex-col overflow-hidden p-4">
          <div class="mb-3 flex items-center justify-between rounded-xl border border-line bg-ink-1 px-3 py-2">
            <div class="flex items-center gap-2">
              <button
                type="button"
                onclick={() => (designTab = 'turnaround')}
                class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-extrabold transition-all {designTab ===
                'turnaround'
                  ? 'bg-accent text-accent-ink shadow-sm'
                  : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
              >
                <Users size={14} />
                <span>{i18n.t('char_tab_turnaround')}</span>
              </button>
              <button
                type="button"
                onclick={() => (designTab = 'skin')}
                class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-extrabold transition-all {designTab ===
                'skin'
                  ? 'bg-accent text-accent-ink shadow-sm'
                  : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
              >
                <Shirt size={14} />
                <span>{i18n.t('char_tab_skin')}</span>
              </button>
            </div>

            <div class="flex items-center gap-2">
              <input
                bind:this={designAnchorInputEl}
                type="file"
                accept="image/*"
                class="hidden"
                onchange={(e) => {
                  const f = (e.currentTarget as HTMLInputElement).files?.[0];
                  if (f) {
                    const rd = new FileReader();
                    rd.onload = () => {
                      const durl = String(rd.result || '');
                      designAnchorDataUrl = durl;
                      designAnchorLabel = f.name.replace(/\.[^.]+$/, '');
                      loadTurnaroundFromDataUrl(durl);
                    };
                    rd.readAsDataURL(f);
                  }
                  (e.currentTarget as HTMLInputElement).value = '';
                }}
              />
              {#if designAnchorDataUrl}
                <div class="flex items-center gap-2 rounded-lg border border-accent/40 bg-accent/10 px-2.5 py-1">
                  <img
                    src={designAnchorDataUrl}
                    alt="Design Anchor"
                    class="h-5 w-5 rounded object-cover"
                  />
                  <span class="max-w-[140px] truncate text-[11px] font-bold text-accent">
                    {i18n.t('char_anchor_prefix')}: {designAnchorLabel || 'Locked'}
                  </span>
                  <button
                    type="button"
                    onclick={() => {
                      designAnchorDataUrl = '';
                      designAnchorLabel = '';
                      designAnchorAssetId = '';
                    }}
                    class="text-fg-muted hover:text-err"
                  >
                    <X size={12} />
                  </button>
                </div>
              {/if}
              <button
                type="button"
                onclick={() => designAnchorInputEl?.click()}
                class="flex items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-xs font-semibold text-fg hover:border-accent"
              >
                <Upload size={12} />
                <span>{i18n.t('char_upload_ref')}</span>
              </button>
            </div>
          </div>

          {#if designTab === 'turnaround'}
            {#if turnaroundDataUrl}
              <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
                <div class="flex flex-col rounded-2xl border border-line bg-ink-1/90 p-3.5 shadow-lg">
                  <div class="mb-2.5 flex flex-wrap items-center justify-between gap-2">
                    <div class="flex items-center gap-2">
                      <Columns3 size={15} class="text-accent" />
                      <span class="text-xs font-extrabold text-fg">
                        {i18n.t('char_split_title')} ({turnaroundWidth}×{turnaroundHeight})
                      </span>
                    </div>
                    <div class="flex flex-wrap items-center gap-2">
                      <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted">
                        <input
                          type="checkbox"
                          bind:checked={showSplitGuides}
                          class="accent-accent"
                        />
                        <span>{i18n.t('char_show_splits')}</span>
                      </label>
                      <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted">
                        <input
                          type="checkbox"
                          bind:checked={autoCutoutSlices}
                          onchange={() => rebuildTurnaroundSlices()}
                          class="accent-accent"
                        />
                        <span>{i18n.t('char_auto_cutout')}</span>
                      </label>
                      <button
                        type="button"
                        onclick={handleAutoDetectSplits}
                        class="flex items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-xs font-semibold text-fg hover:border-accent"
                      >
                        <Wand2 size={12} class="text-accent" />
                        <span>{i18n.t('char_auto_detect_splits')}</span>
                      </button>
                      <button
                        type="button"
                        onclick={() => {
                          designTab = 'skin';
                          appStore.toast('info', i18n.t('char_send_to_skin'));
                        }}
                        class="flex items-center gap-1 rounded-lg bg-accent px-2.5 py-1 text-xs font-extrabold text-accent-ink"
                      >
                        <Shirt size={12} />
                        <span>{i18n.t('char_send_to_skin')}</span>
                      </button>
                    </div>
                  </div>

                  <div class="relative mx-auto flex max-h-[260px] items-center justify-center overflow-hidden rounded-xl border border-line bg-ink-0 p-2">
                    <div class="relative inline-block">
                      <img
                        src={turnaroundDataUrl}
                        alt="Turnaround Sheet"
                        class="max-h-[240px] w-auto rounded object-contain"
                      />
                      {#if showSplitGuides}
                        <div
                          class="pointer-events-none absolute top-0 bottom-0 w-0.5 bg-accent shadow-[0_0_8px_#00E5FF]"
                          style="left: {split1 * 100}%;"
                        ></div>
                        <div
                          class="pointer-events-none absolute top-0 bottom-0 w-0.5 bg-accent shadow-[0_0_8px_#00E5FF]"
                          style="left: {split2 * 100}%;"
                        ></div>
                      {/if}
                    </div>
                  </div>

                  <div class="mt-2.5 grid grid-cols-2 gap-4 rounded-xl bg-ink-0/60 px-3 py-2 text-xs">
                    <div class="flex items-center gap-2">
                      <span class="font-mono text-fg-muted">{i18n.t('char_left_split')}: {(split1 * 100).toFixed(1)}%</span>
                      <input
                        type="range"
                        min="0.2"
                        max="0.48"
                        step="0.005"
                        bind:value={split1}
                        oninput={() => rebuildTurnaroundSlices()}
                        class="flex-1 accent-accent"
                      />
                    </div>
                    <div class="flex items-center gap-2">
                      <span class="font-mono text-fg-muted">{i18n.t('char_right_split')}: {(split2 * 100).toFixed(1)}%</span>
                      <input
                        type="range"
                        min="0.52"
                        max="0.8"
                        step="0.005"
                        bind:value={split2}
                        oninput={() => rebuildTurnaroundSlices()}
                        class="flex-1 accent-accent"
                      />
                    </div>
                  </div>
                </div>

                <!-- 3 Sliced Views -->
                <div class="flex flex-col rounded-2xl border border-line bg-ink-1/90 p-3.5 shadow-lg">
                  <div class="mb-2.5 flex items-center justify-between">
                    <span class="text-xs font-extrabold text-fg">{i18n.t('char_sliced_preview_title')}</span>
                    <button
                      type="button"
                      onclick={handleSaveTurnaroundSlicesToLib}
                      disabled={isSavingSlices || slicedViews.length === 0}
                      class="flex items-center gap-1.5 rounded-xl bg-accent px-3 py-1.5 text-xs font-extrabold text-accent-ink shadow-sm disabled:opacity-40"
                    >
                      {#if isSavingSlices}
                        <Loader2 size={13} class="animate-spin" />
                      {:else}
                        <Save size={13} />
                      {/if}
                      <span>{i18n.t('char_save_slices_btn')}</span>
                    </button>
                  </div>

                  <div class="grid grid-cols-3 gap-3">
                    {#each slicedViews as item (item.key)}
                      {@const viewLabel = getSliceViewLabel(item)}
                      <div class="flex flex-col overflow-hidden rounded-xl border border-line bg-ink-0">
                        <div class="flex items-center justify-between border-b border-line bg-ink-2/60 px-2.5 py-1 text-[11px]">
                          <span class="font-bold text-fg">{viewLabel}</span>
                          <span class="font-mono text-fg-muted">{item.width}×{item.height}</span>
                        </div>
                        <div
                          class="flex h-36 items-center justify-center p-2"
                          style="background-image: linear-gradient(45deg, #1b1e26 25%, transparent 25%), linear-gradient(-45deg, #1b1e26 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #1b1e26 75%), linear-gradient(-45deg, transparent 75%, #1b1e26 75%); background-size: 16px 16px; background-position: 0 0, 0 8px, 8px -8px, -8px 0;"
                        >
                          <img
                            src={item.dataUrl}
                            alt={viewLabel}
                            class="max-h-full max-w-full object-contain"
                          />
                        </div>
                        <div class="flex items-center justify-end gap-1.5 border-t border-line bg-ink-1 px-2 py-1.5">
                          <button
                            type="button"
                            onclick={() => {
                              spriteCharRefDataUrl = item.dataUrl;
                              spriteCharRefLabel = `${turnaroundPrompt.slice(0, 12) || 'Char'} [${viewLabel}]`;
                              activeModule = 'spritesheet';
                              appStore.toast('ok', `${viewLabel} -> ${i18n.t('char_mod2_title')}`);
                            }}
                            class="rounded-md border border-line bg-ink-2 px-2 py-1 text-[11px] font-semibold text-fg hover:border-accent hover:text-accent"
                          >
                            {i18n.t('char_send_to_sprite')}
                          </button>
                          <button
                            type="button"
                            onclick={() => handleExportSingleView(item)}
                            class="flex items-center gap-1 rounded-md bg-accent/15 px-2 py-1 text-[11px] font-bold text-accent hover:bg-accent hover:text-accent-ink"
                          >
                            <Download size={11} />
                            <span>{i18n.t('char_export_png')}</span>
                          </button>
                        </div>
                      </div>
                    {/each}
                  </div>
                </div>
              </div>
            {:else}
              <div class="flex flex-1 flex-col items-center justify-center rounded-2xl border border-dashed border-line bg-ink-1/40 p-8 text-center">
                <Users size={36} class="mb-2 text-accent" />
                <h3 class="text-sm font-extrabold text-fg">{i18n.t('char_turnaround_empty_title')}</h3>
                <p class="mt-1 max-w-md text-xs text-fg-muted">
                  {i18n.t('char_turnaround_empty_sub')}
                </p>
              </div>
            {/if}
          {:else}
            <!-- Skinning Comparison Stage -->
            <div class="flex min-h-0 flex-1 flex-col rounded-2xl border border-line bg-ink-1/90 p-4 shadow-lg">
              <div class="mb-3 flex items-center justify-between border-b border-line pb-2.5">
                <div class="flex items-center gap-2">
                  <Shirt size={16} class="text-accent" />
                  <span class="text-xs font-extrabold text-fg">
                    {i18n.t('char_skin_stage_title')}
                  </span>
                </div>
                {#if skinResultDataUrl}
                  <div class="flex items-center gap-2">
                    <label class="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted">
                      <input
                        type="checkbox"
                        bind:checked={skinCompareSideBySide}
                        class="accent-accent"
                      />
                      <span>{i18n.t('char_skin_side_by_side')}</span>
                    </label>
                    <button
                      type="button"
                      onclick={() => {
                        designAnchorDataUrl = skinResultDataUrl;
                        designAnchorLabel = `Skin: ${skinPrompt.slice(0, 12)}`;
                        loadTurnaroundFromDataUrl(skinResultDataUrl);
                        designTab = 'turnaround';
                      }}
                      class="rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-xs font-semibold text-fg hover:border-accent"
                    >
                      {i18n.t('char_skin_to_turnaround')}
                    </button>
                    {#if skinResultAsset}
                      <button
                        type="button"
                        onclick={() => appStore.openInStudio(skinResultAsset!)}
                        class="rounded-lg bg-accent px-2.5 py-1 text-xs font-extrabold text-accent-ink"
                      >
                        {i18n.t('asset_send_to_studio')}
                      </button>
                    {/if}
                  </div>
                {/if}
              </div>

              <div class="grid min-h-0 flex-1 {skinCompareSideBySide && skinResultDataUrl ? 'grid-cols-2' : 'grid-cols-1'} gap-4">
                {#if designAnchorDataUrl && (skinCompareSideBySide || !skinResultDataUrl)}
                  <div class="flex flex-col overflow-hidden rounded-xl border border-line bg-ink-0">
                    <div class="border-b border-line bg-ink-2/60 px-3 py-1.5 text-xs font-bold text-fg-muted">
                      {i18n.t('char_skin_orig_header')}
                    </div>
                    <div class="flex flex-1 items-center justify-center p-3">
                      <img
                        src={designAnchorDataUrl}
                        alt="Original Character"
                        class="max-h-[340px] max-w-full rounded object-contain"
                      />
                    </div>
                  </div>
                {/if}

                {#if skinResultDataUrl}
                  <div class="flex flex-col overflow-hidden rounded-xl border border-accent/50 bg-ink-0">
                    <div class="border-b border-line bg-accent/10 px-3 py-1.5 text-xs font-extrabold text-accent">
                      {i18n.t('char_skin_new_header')}
                    </div>
                    <div class="flex flex-1 items-center justify-center p-3">
                      <img
                        src={skinResultDataUrl}
                        alt="New Skin Result"
                        class="max-h-[340px] max-w-full rounded object-contain"
                      />
                    </div>
                  </div>
                {:else if !designAnchorDataUrl}
                  <div class="flex flex-col items-center justify-center text-center text-xs text-fg-muted">
                    {i18n.t('char_skin_empty_hint')}
                  </div>
                {/if}
              </div>
            </div>
          {/if}
        </div>

        <!-- Right Inspector for Module 1 (Virtual Placeholder, Clean UI) -->
        <aside class="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-line bg-ink-1/95 p-3.5">
          {#if designTab === 'turnaround'}
            <div class="flex flex-col gap-3">
              <div class="flex items-center justify-between">
                <span class="text-xs font-extrabold text-fg">{i18n.t('char_turnaround_cfg_title')}</span>
                <div class="flex rounded-lg border border-line bg-ink-0 p-0.5 text-[11px]">
                  <button
                    type="button"
                    onclick={() => (turnaroundLayout = '3view_horiz')}
                    class="rounded-md px-2 py-0.5 font-bold {turnaroundLayout === '3view_horiz'
                      ? 'bg-accent text-accent-ink'
                      : 'text-fg-muted'}"
                  >
                    {i18n.t('char_layout_3view')}
                  </button>
                  <button
                    type="button"
                    onclick={() => (turnaroundLayout = 'single_portrait')}
                    class="rounded-md px-2 py-0.5 font-bold {turnaroundLayout === 'single_portrait'
                      ? 'bg-accent text-accent-ink'
                      : 'text-fg-muted'}"
                  >
                    {i18n.t('char_layout_single')}
                  </button>
                </div>
              </div>

              <div class="flex flex-col gap-1">
                <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_constraint_tags_label')}</span>
                <div class="flex flex-wrap gap-1">
                  {#each TURNAROUND_MODIFIERS as mod (mod.id)}
                    {@const active = activeTurnaroundTags.includes(mod.id)}
                    <button
                      type="button"
                      onclick={() => toggleTurnaroundTag(mod.id)}
                      class="rounded-md border px-2 py-0.5 text-[10px] font-semibold transition-all {active
                        ? 'border-accent bg-accent/15 text-accent'
                        : 'border-line bg-ink-0 text-fg-muted hover:text-fg'}"
                    >
                      {i18n.locale === 'en' ? mod.labelEn : mod.label}
                    </button>
                  {/each}
                </div>
              </div>

              <div class="flex flex-col gap-1">
                <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_turnaround_prompt_label')}</span>
                <textarea
                  rows="5"
                  bind:value={turnaroundPrompt}
                  placeholder={i18n.t('char_turnaround_prompt_ph')}
                  class="w-full resize-y rounded-xl border border-line bg-ink-0 p-2.5 text-xs leading-relaxed text-fg outline-none focus:border-accent"
                ></textarea>
              </div>

              <button
                type="button"
                onclick={handleGenerateTurnaround}
                disabled={isGenerating}
                class="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-xs font-extrabold text-accent-ink shadow-md hover:brightness-105 disabled:opacity-40"
              >
                {#if isGenerating}
                  <Loader2 size={15} class="animate-spin" />
                  <span>{i18n.t('char_generating_turnaround')}</span>
                {:else}
                  <Sparkles size={15} />
                  <span>{i18n.t('char_generate_turnaround_btn')}</span>
                {/if}
              </button>
            </div>
          {:else}
            <div class="flex flex-col gap-3">
              <span class="text-xs font-extrabold text-fg">{i18n.t('char_skin_cfg_title')}</span>

              <div class="flex flex-col gap-1">
                <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_skin_lock_label')}</span>
                <select
                  bind:value={skinLockMode}
                  class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs font-semibold text-fg outline-none focus:border-accent"
                >
                  <option value="keep_layout">{i18n.t('char_skin_lock_keep')}</option>
                  <option value="new_turnaround">{i18n.t('char_skin_lock_3view')}</option>
                  <option value="new_portrait">{i18n.t('char_skin_lock_single')}</option>
                </select>
              </div>

              <div class="flex flex-col gap-1">
                <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_skin_presets_label')}</span>
                <div class="grid grid-cols-2 gap-1">
                  {#each SKIN_PRESET_THEMES as th (th.label)}
                    <button
                      type="button"
                      onclick={() => (skinPrompt = i18n.locale === 'en' ? th.textEn : th.text)}
                      class="truncate rounded-md border border-line bg-ink-0 px-2 py-1 text-left text-[10px] font-semibold text-fg-muted hover:border-accent hover:text-fg"
                    >
                      {i18n.locale === 'en' ? th.labelEn : th.label}
                    </button>
                  {/each}
                </div>
              </div>

              <div class="flex flex-col gap-1">
                <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_skin_prompt_label')}</span>
                <textarea
                  rows="5"
                  bind:value={skinPrompt}
                  placeholder={i18n.t('char_skin_prompt_ph')}
                  class="w-full resize-y rounded-xl border border-line bg-ink-0 p-2.5 text-xs leading-relaxed text-fg outline-none focus:border-accent"
                ></textarea>
              </div>

              <button
                type="button"
                onclick={handleGenerateSkinVariant}
                disabled={isGenerating || !designAnchorDataUrl}
                class="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-xs font-extrabold text-accent-ink shadow-md hover:brightness-105 disabled:opacity-40"
              >
                {#if isGenerating}
                  <Loader2 size={15} class="animate-spin" />
                  <span>{i18n.t('char_generating_skin')}</span>
                {:else}
                  <Shirt size={15} />
                  <span>{i18n.t('char_generate_skin_btn')}</span>
                {/if}
              </button>
            </div>
          {/if}
        </aside>
      </div>

    <!-- ====================================================================== -->
    <!-- MODULE 2: Sprite Sheet & Pivot Studio                                  -->
    <!-- ====================================================================== -->
    {:else if activeModule === 'spritesheet'}
      <div class="flex min-h-0 flex-1 overflow-hidden">
        <!-- Center Stage: Live Animation Preview (With Wheel Zoom!) + Raw Grid Slicer -->
        <div class="flex min-w-0 flex-1 flex-col overflow-y-auto p-4">
          {#if spriteSheetDataUrl}
            <div class="grid min-h-0 flex-1 grid-cols-12 gap-4">
              <!-- Left 7 Cols: Live Pivot-Aligned Animation Player + Frame Timeline -->
              <div class="col-span-7 flex flex-col justify-between rounded-2xl border border-line bg-ink-1/90 p-4 shadow-xl">
                <div class="flex flex-wrap items-center justify-between gap-2 border-b border-line pb-2.5">
                  <div class="flex items-center gap-2">
                    <Film size={15} class="text-accent" />
                    <span class="text-xs font-extrabold text-fg">
                      {i18n.t('char_sprite_player_title')}
                    </span>
                    {#if activeDisplayFrame}
                      <span class="rounded bg-accent/15 px-2 py-0.5 font-mono text-[11px] font-bold text-accent">
                        {i18n.t('char_frame_badge')} #{activeDisplayFrame.index + 1}/{spriteFrames.length} · {activeDisplayFrame.width}×{activeDisplayFrame.height}px
                      </span>
                    {/if}
                  </div>

                  <div class="flex flex-wrap items-center gap-2 text-xs">
                    <!-- Zoom Controls in Preview Header -->
                    <div class="flex items-center gap-1 rounded-lg border border-line bg-ink-0 px-1.5 py-0.5">
                      <button
                        type="button"
                        onclick={() => (previewZoom = Math.max(0.4, Number((previewZoom / 1.2).toFixed(2))))}
                        class="text-fg-muted hover:text-accent"
                      >
                        <ZoomOut size={12} />
                      </button>
                      <button
                        type="button"
                        onclick={resetPreviewTransform}
                        class="min-w-[42px] text-center font-mono text-[10px] font-bold text-accent hover:underline"
                      >
                        {Math.round(previewZoom * 100)}%
                      </button>
                      <button
                        type="button"
                        onclick={() => (previewZoom = Math.min(8, Number((previewZoom * 1.2).toFixed(2))))}
                        class="text-fg-muted hover:text-accent"
                      >
                        <ZoomIn size={12} />
                      </button>
                    </div>

                    <label class="flex cursor-pointer items-center gap-1 text-fg-muted">
                      <input type="checkbox" bind:checked={onionSkin} class="accent-accent" />
                      <span>{i18n.t('char_onion_skin')}</span>
                    </label>
                    <label class="flex cursor-pointer items-center gap-1 text-fg-muted">
                      <input type="checkbox" bind:checked={showPivotGuide} class="accent-accent" />
                      <span>{i18n.t('char_pivot_guide')}</span>
                    </label>
                    <label class="flex cursor-pointer items-center gap-1 text-fg-muted">
                      <input type="checkbox" bind:checked={pingPongLoop} class="accent-accent" />
                      <span>{i18n.t('char_pingpong')}</span>
                    </label>
                  </div>
                </div>

                <!-- Main Animation Canvas Stage (Supports Mouse Wheel Zoom & Drag Pan!) -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  onwheel={handlePreviewWheel}
                  onpointerdown={handlePreviewPointerDown}
                  onpointermove={handlePreviewPointerMove}
                  onpointerup={handlePreviewPointerUp}
                  onpointercancel={handlePreviewPointerUp}
                  ondblclick={resetPreviewTransform}
                  class="relative my-3 flex min-h-[260px] flex-1 cursor-grab items-center justify-center overflow-hidden rounded-xl border border-line active:cursor-grabbing"
                  style={previewBackdrop === 'checker'
                    ? 'background-image: linear-gradient(45deg, #1b1e26 25%, transparent 25%), linear-gradient(-45deg, #1b1e26 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #1b1e26 75%), linear-gradient(-45deg, transparent 75%, #1b1e26 75%); background-size: 20px 20px; background-position: 0 0, 0 10px, 10px -10px, -10px 0; background-color: #12141a;'
                    : previewBackdrop === 'light'
                      ? 'background-color: #f3f4f6;'
                      : 'background-color: #0d0f14;'}
                >
                  {#if activeDisplayFrame}
                    <div
                      class="relative flex h-64 w-64 items-center justify-center transition-transform duration-75"
                      style="transform: translate({previewPanX}px, {previewPanY}px) scale({previewZoom}); transform-origin: center center;"
                    >
                      {#if onionSkin && prevOnionFrame}
                        <img
                          src={prevOnionFrame.dataUrl}
                          alt="Onion Skin"
                          class="pointer-events-none absolute inset-0 h-full w-full object-contain opacity-30 saturate-50"
                          style="image-rendering: {spritePixelClean ? 'pixelated' : 'auto'};"
                        />
                      {/if}

                      <img
                        src={activeDisplayFrame.dataUrl}
                        alt="Current Frame"
                        class="pointer-events-none relative z-10 h-full w-full object-contain"
                        style="image-rendering: {spritePixelClean ? 'pixelated' : 'auto'};"
                      />

                      {#if showPivotGuide}
                        <div
                          class="pointer-events-none absolute right-2 left-2 z-20 border-t border-dashed border-accent/80"
                          style="top: {spriteBaselineRatio * 100}%;"
                        >
                          <span class="absolute -top-4 right-1 rounded bg-ink-0/80 px-1 font-mono text-[9px] font-bold text-accent">
                            GROUND ({Math.round(spriteBaselineRatio * 100)}%)
                          </span>
                        </div>
                        <div
                          class="pointer-events-none absolute top-4 bottom-4 left-1/2 z-20 -translate-x-1/2 border-l border-dashed border-accent/50"
                        ></div>
                      {/if}
                    </div>
                  {/if}

                  {#if previewZoom !== 1 || previewPanX !== 0 || previewPanY !== 0}
                    <button
                      type="button"
                      onclick={resetPreviewTransform}
                      class="absolute bottom-2.5 right-2.5 z-30 flex items-center gap-1 rounded-lg border border-line bg-ink-1/90 px-2 py-1 font-mono text-[10px] font-bold text-fg-muted shadow hover:border-accent hover:text-accent"
                    >
                      <RotateCcw size={10} />
                      <span>{i18n.t('char_reset_view')} ({Math.round(previewZoom * 100)}%)</span>
                    </button>
                  {/if}
                </div>

                <!-- Playback Controls + Frame Timeline -->
                <div class="flex flex-col gap-2">
                  <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line bg-ink-0/80 px-3 py-2">
                    <div class="flex items-center gap-1.5">
                      <button
                        type="button"
                        onclick={() => {
                          isPlaying = false;
                          currentFrameIdx =
                            (currentFrameIdx - 1 + Math.max(1, enabledFrames.length)) %
                            Math.max(1, enabledFrames.length);
                        }}
                        class="flex h-7 w-7 items-center justify-center rounded-lg border border-line bg-ink-2 text-fg hover:border-accent"
                      >
                        <SkipBack size={13} />
                      </button>
                      <button
                        type="button"
                        onclick={() => (isPlaying = !isPlaying)}
                        class="flex h-7 w-8 items-center justify-center rounded-lg bg-accent text-accent-ink shadow-sm"
                      >
                        {#if isPlaying}
                          <Pause size={14} />
                        {:else}
                          <Play size={14} />
                        {/if}
                      </button>
                      <button
                        type="button"
                        onclick={() => {
                          isPlaying = false;
                          currentFrameIdx =
                            (currentFrameIdx + 1) % Math.max(1, enabledFrames.length);
                        }}
                        class="flex h-7 w-7 items-center justify-center rounded-lg border border-line bg-ink-2 text-fg hover:border-accent"
                      >
                        <SkipForward size={13} />
                      </button>
                    </div>

                    <div class="flex flex-1 items-center gap-2 text-xs">
                      <span class="font-mono text-[11px] text-fg-muted">{i18n.t('char_fps_label')}</span>
                      <input
                        type="range"
                        min="2"
                        max="24"
                        step="1"
                        bind:value={fps}
                        class="flex-1 accent-accent"
                      />
                      <span class="w-12 font-mono text-xs font-bold text-accent">{fps} FPS</span>
                    </div>
                  </div>

                  {#if activeDisplayFrame}
                    {@const curTweak = frameTweaks[activeDisplayFrame.index]}
                    {@const expR = curTweak?.expandRight ?? 0}
                    {@const expL = curTweak?.expandLeft ?? 0}
                    {@const expT = curTweak?.expandTop ?? 0}
                    {@const expB = curTweak?.expandBottom ?? 0}
                    {@const curDx = curTweak?.dx ?? 0}
                    {@const curDy = curTweak?.dy ?? 0}
                    <div class="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-line bg-ink-0/60 px-2.5 py-1.5 text-xs">
                      <div class="flex items-center gap-2">
                        <span class="rounded bg-accent/20 px-2 py-0.5 font-mono text-[11px] font-bold text-accent">
                          {i18n.t('char_calib_frame')} #{activeDisplayFrame.index + 1}
                        </span>
                        <!-- Position Shift -->
                        <div class="flex items-center gap-1 border-l border-line/60 pl-2">
                          <span class="text-[11px] text-fg-muted">{i18n.t('char_nudge_pos')}:</span>
                          <button
                            type="button"
                            title="Left 2px"
                            onclick={() => nudgeCurrentFrame(-2, 0)}
                            class="flex h-5 w-5 items-center justify-center rounded border border-line bg-ink-2 font-mono text-[10px] font-bold hover:border-accent"
                          >
                            ←
                          </button>
                          <button
                            type="button"
                            title="Right 2px"
                            onclick={() => nudgeCurrentFrame(2, 0)}
                            class="flex h-5 w-5 items-center justify-center rounded border border-line bg-ink-2 font-mono text-[10px] font-bold hover:border-accent"
                          >
                            →
                          </button>
                          <button
                            type="button"
                            title="Up 2px"
                            onclick={() => nudgeCurrentFrame(0, -2)}
                            class="flex h-5 w-5 items-center justify-center rounded border border-line bg-ink-2 font-mono text-[10px] font-bold hover:border-accent"
                          >
                            ↑
                          </button>
                          <button
                            type="button"
                            title="Down 2px"
                            onclick={() => nudgeCurrentFrame(0, 2)}
                            class="flex h-5 w-5 items-center justify-center rounded border border-line bg-ink-2 font-mono text-[10px] font-bold hover:border-accent"
                          >
                            ↓
                          </button>
                          {#if curDx !== 0 || curDy !== 0}
                            <button
                              type="button"
                              title="Reset Position"
                              onclick={() => resetCurrentFramePos()}
                              class="ml-0.5 rounded px-1 py-0.5 font-mono text-[10px] text-accent hover:bg-ink-2"
                            >
                              ({curDx > 0 ? `+${curDx}` : curDx}, {curDy > 0 ? `+${curDy}` : curDy}) ⟲
                            </button>
                          {/if}
                        </div>
                      </div>

                      <!-- Crop Boundary Expansion (Recover slash / exclude debris) -->
                      <div class="flex items-center gap-1.5">
                        <span class="text-[11px] font-medium text-accent" title={i18n.t('char_nudge_crop')}>
                          {i18n.t('char_nudge_crop')}:
                        </span>
                        
                        <!-- Right Expand (Primary for sword slash) -->
                        <div class="flex items-center rounded border border-line bg-ink-2/80 px-1 py-0.5" title={i18n.t('char_crop_right')}>
                          <span class="mr-1 font-mono text-[10px] font-bold text-fg-muted">R</span>
                          <button
                            type="button"
                            onclick={() => adjustCurrentFrameCrop('right', -10)}
                            class="rounded px-1 text-[10px] font-mono hover:bg-ink-1"
                          >
                            -10
                          </button>
                          <button
                            type="button"
                            onclick={() => adjustCurrentFrameCrop('right', 10)}
                            class="rounded bg-accent/20 px-1 font-mono text-[10px] font-bold text-accent hover:bg-accent/30"
                          >
                            +10
                          </button>
                          {#if expR !== 0}
                            <span class="ml-1 font-mono text-[10px] font-bold text-accent">
                              {expR > 0 ? `+${expR}` : expR}
                            </span>
                          {/if}
                        </div>

                        <!-- Left Expand / Shrink (Exclude stray fragments or recover left slash) -->
                        <div class="flex items-center rounded border border-line bg-ink-2/80 px-1 py-0.5" title={i18n.t('char_crop_left')}>
                          <span class="mr-1 font-mono text-[10px] font-bold text-fg-muted">L</span>
                          <button
                            type="button"
                            onclick={() => adjustCurrentFrameCrop('left', -10)}
                            class="rounded px-1 text-[10px] font-mono hover:bg-ink-1"
                          >
                            -10
                          </button>
                          <button
                            type="button"
                            onclick={() => adjustCurrentFrameCrop('left', 10)}
                            class="rounded px-1 text-[10px] font-mono hover:bg-ink-1"
                          >
                            +10
                          </button>
                          {#if expL !== 0}
                            <span class="ml-1 font-mono text-[10px] font-bold text-accent">
                              {expL > 0 ? `+${expL}` : expL}
                            </span>
                          {/if}
                        </div>

                        <!-- Top & Bottom -->
                        <div class="flex items-center rounded border border-line bg-ink-2/80 px-1 py-0.5">
                          <button
                            type="button"
                            title="{i18n.t('char_crop_top')} +10px"
                            onclick={() => adjustCurrentFrameCrop('top', 10)}
                            class="rounded px-1 font-mono text-[10px] hover:bg-ink-1"
                          >
                            T+10
                          </button>
                          <button
                            type="button"
                            title="{i18n.t('char_crop_bot')} +10px"
                            onclick={() => adjustCurrentFrameCrop('bottom', 10)}
                            class="rounded px-1 font-mono text-[10px] hover:bg-ink-1"
                          >
                            B+10
                          </button>
                          {#if expT !== 0 || expB !== 0}
                            <span class="ml-1 font-mono text-[10px] font-bold text-accent">
                              T:{expT > 0 ? `+${expT}` : expT} B:{expB > 0 ? `+${expB}` : expB}
                            </span>
                          {/if}
                        </div>

                        {#if expR !== 0 || expL !== 0 || expT !== 0 || expB !== 0}
                          <button
                            type="button"
                            title={i18n.t('char_crop_reset')}
                            onclick={() => resetCurrentFrameCrop()}
                            class="rounded bg-accent/15 px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent hover:bg-accent/25"
                          >
                            {i18n.t('char_crop_reset')}
                          </button>
                        {/if}
                      </div>
                    </div>
                  {/if}

                  <div class="flex items-center gap-2 overflow-x-auto pt-1 pb-0.5">
                    {#each spriteFrames as frame (frame.index)}
                      {@const isFrameEnabled = frameTweaks[frame.index]?.enabled ?? true}
                      {@const isCurrent = activeDisplayFrame?.index === frame.index}
                      <div
                        class="group relative flex h-20 w-20 shrink-0 flex-col items-center justify-center rounded-xl border p-1 transition-all {isCurrent
                          ? 'border-accent bg-accent/15 ring-2 ring-accent/40'
                          : isFrameEnabled
                            ? 'border-line bg-ink-0 hover:border-fg-muted'
                            : 'border-line/40 bg-ink-0/30 opacity-40'}"
                      >
                        <button
                          type="button"
                          onclick={() => {
                            isPlaying = false;
                            const enabledIdx = enabledFrames.findIndex(
                              (f) => f.index === frame.index
                            );
                            if (enabledIdx >= 0) currentFrameIdx = enabledIdx;
                          }}
                          class="flex h-full w-full items-center justify-center"
                        >
                          <img
                            src={frame.dataUrl}
                            alt="Frame #{frame.index + 1}"
                            class="max-h-14 max-w-14 object-contain"
                            style="image-rendering: {spritePixelClean ? 'pixelated' : 'auto'};"
                          />
                        </button>
                        <span class="pointer-events-none absolute top-1 left-1.5 font-mono text-[10px] font-bold text-fg-muted">
                          #{frame.index + 1}
                        </span>
                        <button
                          type="button"
                          onclick={() => toggleFrameEnabled(frame.index)}
                          class="absolute top-1 right-1 rounded px-1 font-mono text-[9px] font-bold {isFrameEnabled
                            ? 'bg-ok/20 text-ok'
                            : 'bg-err/25 text-err'}"
                        >
                          {isFrameEnabled ? 'ON' : 'OFF'}
                        </button>
                      </div>
                    {/each}
                  </div>
                </div>
              </div>

              <!-- Right 5 Cols: Raw Sheet Overview + Anti-Clip Status + Export -->
              <div class="col-span-5 flex flex-col justify-between rounded-2xl border border-line bg-ink-1/90 p-4 shadow-xl">
                <div class="flex items-center justify-between border-b border-line pb-2.5 text-xs">
                  <span class="font-extrabold text-fg">
                    {i18n.t('char_raw_sheet_title')} ({sheetRows} × {sheetCols})
                  </span>
                  <input
                    bind:this={sheetFileInputEl}
                    type="file"
                    accept="image/*"
                    class="hidden"
                    onchange={(e) => {
                      const f = (e.currentTarget as HTMLInputElement).files?.[0];
                      if (f) {
                        const rd = new FileReader();
                        rd.onload = () => loadSpriteSheetFromDataUrl(String(rd.result || ''));
                        rd.readAsDataURL(f);
                      }
                      (e.currentTarget as HTMLInputElement).value = '';
                    }}
                  />
                  <button
                    type="button"
                    onclick={() => sheetFileInputEl?.click()}
                    class="flex items-center gap-1 text-accent hover:underline"
                  >
                    <Upload size={12} />
                    <span>{i18n.t('char_import_sheet_btn')}</span>
                  </button>
                </div>

                <div class="relative my-3 flex flex-1 items-center justify-center overflow-hidden rounded-xl border border-line bg-ink-0 p-2">
                  <div class="relative inline-block max-h-[280px]">
                    <img
                      src={spriteSheetDataUrl}
                      alt="Raw Sprite Sheet"
                      class="max-h-[280px] w-auto object-contain"
                    />
                    <div
                      class="absolute inset-0 grid border border-accent/60"
                      style="grid-template-rows: repeat({sheetRows}, minmax(0, 1fr)); grid-template-columns: repeat({sheetCols}, minmax(0, 1fr));"
                    >
                      {#each Array.from({ length: sheetRows * sheetCols }) as _, idx (idx)}
                        {@const isCur = activeDisplayFrame?.index === idx}
                        {@const tw = frameTweaks[idx]}
                        {@const hasExp = (tw?.expandLeft ?? 0) !== 0 || (tw?.expandRight ?? 0) !== 0 || (tw?.expandTop ?? 0) !== 0 || (tw?.expandBottom ?? 0) !== 0}
                        <button
                          type="button"
                          onclick={() => {
                            isPlaying = false;
                            const enIdx = enabledFrames.findIndex((f) => f.index === idx);
                            if (enIdx >= 0) currentFrameIdx = enIdx;
                          }}
                          class="group relative flex flex-col justify-between border p-1 text-left transition-all cursor-pointer {isCur
                            ? 'border-accent bg-accent/20 ring-2 ring-accent'
                            : 'border-dashed border-accent/40 hover:bg-accent/10'}"
                        >
                          <div class="pointer-events-none flex w-full items-center justify-between">
                            <span class="rounded bg-ink-0/80 px-1 font-mono text-[9px] font-bold {isCur ? 'text-accent' : 'text-fg-muted'}">
                              #{idx + 1}
                            </span>
                            {#if hasExp}
                              <span class="rounded bg-accent px-1 font-mono text-[8px] font-bold text-ink-0">
                                {tw?.expandRight ? `+${tw.expandRight}R` : 'EXP'}
                              </span>
                            {/if}
                          </div>
                        </button>
                      {/each}
                    </div>
                  </div>
                </div>

                <!-- Anti-Clipping & Slicing Controls Card -->
                <div class="mb-3 flex flex-col gap-1.5 rounded-xl border border-accent/30 bg-ink-0/75 p-2.5 text-xs">
                  <div class="flex items-center justify-between">
                    <span class="flex items-center gap-1 text-[11px] font-extrabold text-accent">
                      <ShieldCheck size={13} />
                      <span>{i18n.t('char_anticlip_title')}</span>
                    </span>
                  </div>
                  <label class="flex cursor-pointer items-center gap-2 text-[11px] text-fg">
                    <input
                      type="checkbox"
                      bind:checked={spriteSmartIsland}
                      onchange={() => rebuildSpriteFrames()}
                      class="accent-accent"
                    />
                    <span>{i18n.t('char_smart_island_label')}</span>
                  </label>
                  <label class="flex cursor-pointer items-center gap-2 text-[11px] text-fg">
                    <input
                      type="checkbox"
                      bind:checked={spritePreventClip}
                      onchange={() => rebuildSpriteFrames()}
                      class="accent-accent"
                    />
                    <span>{i18n.t('char_prevent_clip_label')}</span>
                  </label>
                  <div class="grid grid-cols-2 gap-2 pt-0.5">
                    <label class="flex cursor-pointer items-center gap-1.5 text-[11px] text-fg-muted">
                      <input
                        type="checkbox"
                        bind:checked={spriteRemoveBg}
                        onchange={() => rebuildSpriteFrames()}
                        class="accent-accent"
                      />
                      <span>{i18n.t('char_remove_bg_label')}</span>
                    </label>
                    <label class="flex cursor-pointer items-center gap-1.5 text-[11px] text-fg-muted">
                      <input
                        type="checkbox"
                        bind:checked={spriteAlignPivot}
                        onchange={() => rebuildSpriteFrames()}
                        class="accent-accent"
                      />
                      <span>{i18n.t('char_align_pivot_label')}</span>
                    </label>
                  </div>
                  <div class="flex items-center gap-2 pt-1 text-[11px]">
                    <span class="text-fg-muted">{i18n.t('char_ground_line_label')}</span>
                    <input
                      type="range"
                      min="0.68"
                      max="0.95"
                      step="0.01"
                      bind:value={spriteBaselineRatio}
                      oninput={() => rebuildSpriteFrames()}
                      class="flex-1 accent-accent"
                    />
                    <span class="font-mono font-bold text-accent">
                      {Math.round(spriteBaselineRatio * 100)}%
                    </span>
                  </div>
                </div>

                <!-- Export Buttons -->
                <div class="flex flex-col gap-2">
                  <div class="flex gap-2">
                    <button
                      type="button"
                      onclick={handleExportAlignedSpriteSheet}
                      disabled={enabledFrames.length === 0}
                      class="flex flex-1 items-center justify-center gap-1.5 rounded-xl bg-accent py-2 text-xs font-extrabold text-accent-ink shadow-sm hover:brightness-105 disabled:opacity-40"
                    >
                      <Download size={14} />
                      <span>{i18n.t('char_export_sheet_btn')}</span>
                    </button>
                    <button
                      type="button"
                      onclick={handleSaveAlignedSheetToLibrary}
                      disabled={enabledFrames.length === 0}
                      class="flex items-center gap-1 rounded-xl border border-line bg-ink-2 px-3 py-2 text-xs font-bold text-fg hover:border-accent"
                    >
                      <Save size={13} />
                      <span>{i18n.t('char_save_lib_short')}</span>
                    </button>
                  </div>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      onclick={handleExportCurrentFrame}
                      disabled={!activeDisplayFrame}
                      class="flex flex-1 items-center justify-center gap-1 rounded-lg border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent"
                    >
                      <Download size={12} />
                      <span>{i18n.t('char_export_frame_btn')}</span>
                    </button>
                    <button
                      type="button"
                      onclick={async () => {
                        if (enabledFrames.length === 0) return;
                        const sheetCv = composeAlignedSpriteSheet(enabledFrames, sheetCols);
                        const durl = sheetCv.toDataURL('image/png');
                        const targetProj =
                          charProjectFilter === '_all'
                            ? appStore.settings?.activeProject || ''
                            : charProjectFilter;
                        const saved = await api.saveStudioAsset(
                          durl,
                          targetProj,
                          'sprite_sheet',
                          `SpriteSheet [${actionPreset} ${enabledFrames.length}F]`
                        );
                        appStore.notifyAssetsChanged();
                        appStore.openInStudio(saved);
                      }}
                      class="flex flex-1 items-center justify-center gap-1 rounded-lg border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent"
                    >
                      <Sliders size={12} />
                      <span>{i18n.t('char_sheet_to_studio_btn')}</span>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          {:else}
            <div class="flex flex-1 flex-col items-center justify-center rounded-2xl border border-dashed border-line bg-ink-1/40 p-8 text-center">
              <Film size={36} class="mb-2 text-accent" />
              <h3 class="text-sm font-extrabold text-fg">{i18n.t('char_sprite_empty_title')}</h3>
              <p class="mt-1 max-w-md text-xs text-fg-muted">
                {i18n.t('char_sprite_empty_sub')}
              </p>
            </div>
          {/if}
        </div>

        <!-- Right Inspector for Module 2: Clean Action Preset Dropdown (Default first) + Virtual Placeholder Prompt -->
        <aside class="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-line bg-ink-1/95 p-3.5">
          <div class="flex items-center justify-between">
            <span class="text-xs font-extrabold text-fg">{i18n.t('char_sprite_cfg_title')}</span>
            <span class="rounded bg-ok/15 px-1.5 py-0.5 font-mono text-[9px] font-bold text-ok">
              {i18n.t('char_anticlip_badge')}
            </span>
          </div>

          <!-- Dedicated Slot: Character Reference Image for Sprite Sheet Generation -->
          <div class="flex flex-col gap-1.5 rounded-xl border border-line bg-ink-0/70 p-2.5">
            <div class="flex items-center justify-between text-[11px]">
              <span class="font-bold text-fg">{i18n.t('char_sprite_ref_slot')}</span>
              <input
                bind:this={spriteCharRefInputEl}
                type="file"
                accept="image/*"
                class="hidden"
                onchange={(e) => {
                  const f = (e.currentTarget as HTMLInputElement).files?.[0];
                  if (f) {
                    const rd = new FileReader();
                    rd.onload = () => {
                      spriteCharRefDataUrl = String(rd.result || '');
                      spriteCharRefLabel = f.name.replace(/\.[^.]+$/, '');
                    };
                    rd.readAsDataURL(f);
                  }
                  (e.currentTarget as HTMLInputElement).value = '';
                }}
              />
              <button
                type="button"
                onclick={() => spriteCharRefInputEl?.click()}
                class="text-accent hover:underline"
              >
                {spriteCharRefDataUrl ? i18n.t('char_change_ref') : i18n.t('char_upload_portrait')}
              </button>
            </div>

            {#if spriteCharRefDataUrl}
              <div class="flex items-center justify-between gap-2 rounded-lg border border-accent/40 bg-accent/10 px-2 py-1.5">
                <div class="flex min-w-0 items-center gap-2">
                  <img
                    src={spriteCharRefDataUrl}
                    alt="Sprite Character Ref"
                    class="h-9 w-9 shrink-0 rounded border border-line bg-ink-0 object-contain"
                  />
                  <div class="flex min-w-0 flex-col">
                    <span class="truncate text-[11px] font-bold text-accent">
                      {spriteCharRefLabel || 'Locked'}
                    </span>
                    <span class="text-[10px] text-fg-muted">{i18n.t('char_ref_locked_sub')}</span>
                  </div>
                </div>
                <button
                  type="button"
                  onclick={() => {
                    spriteCharRefDataUrl = '';
                    spriteCharRefLabel = '';
                    spriteCharRefAssetId = '';
                  }}
                  class="text-fg-muted hover:text-err"
                >
                  <X size={13} />
                </button>
              </div>
            {:else}
              <div class="text-[10px] text-fg-dim">
                {i18n.t('char_ref_empty_hint')}
              </div>
            {/if}
          </div>

          <!-- Action Preset Dropdown (Default option first to avoid built-in prompt pollution) -->
          <div class="flex flex-col gap-1 text-xs">
            <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_action_preset_label')}</span>
            <select
              bind:value={actionPreset}
              class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs font-semibold text-fg outline-none focus:border-accent"
            >
              {#each SPRITE_ACTION_PRESETS as act (act.id)}
                <option value={act.id}>{i18n.locale === 'en' ? act.labelEn : act.label}</option>
              {/each}
            </select>
          </div>

          <div class="flex flex-col gap-1 text-xs">
            <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_grid_spec_label')}</span>
            <select
              value={gridPreset}
              onchange={(e) => handleGridPresetChange((e.currentTarget as HTMLSelectElement).value)}
              class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs font-semibold text-fg outline-none focus:border-accent"
            >
              <option value="2x4">2 × 4 (8 Frames)</option>
              <option value="2x3">2 × 3 (6 Frames)</option>
              <option value="2x2">2 × 2 (4 Frames)</option>
              <option value="1x4">1 × 4 (4 Frames Row)</option>
              <option value="1x6">1 × 6 (6 Frames Row)</option>
              <option value="3x3">3 × 3 (9 Frames)</option>
            </select>
          </div>

          <div class="grid grid-cols-2 gap-2 text-xs">
            <div class="flex items-center justify-between rounded-lg border border-line bg-ink-0 px-2.5 py-1">
              <span class="text-[11px] text-fg-muted">{i18n.t('char_rows_label')}</span>
              <input
                type="number"
                min="1"
                max="6"
                bind:value={sheetRows}
                onchange={() => rebuildSpriteFrames()}
                class="w-12 bg-transparent text-right font-mono text-xs font-bold text-fg outline-none"
              />
            </div>
            <div class="flex items-center justify-between rounded-lg border border-line bg-ink-0 px-2.5 py-1">
              <span class="text-[11px] text-fg-muted">{i18n.t('char_cols_label')}</span>
              <input
                type="number"
                min="1"
                max="8"
                bind:value={sheetCols}
                onchange={() => rebuildSpriteFrames()}
                class="w-12 bg-transparent text-right font-mono text-xs font-bold text-fg outline-none"
              />
            </div>
          </div>

          <!-- Clean Prompt Textarea with Virtual Placeholder (Disappears on typing!) -->
          <div class="flex flex-col gap-1">
            <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_sprite_prompt_label')}</span>
            <textarea
              rows="5"
              bind:value={spriteExtraPrompt}
              placeholder={i18n.locale === 'en' ? currentActionPresetObj.placeholderEn : currentActionPresetObj.placeholder}
              class="w-full resize-y rounded-xl border border-line bg-ink-0 p-2.5 text-xs leading-relaxed text-fg outline-none focus:border-accent"
            ></textarea>
          </div>

          <!-- Generation-Time Safe Grid Guide Option -->
          <label class="flex cursor-pointer items-center gap-2 rounded-lg border border-line bg-ink-0/60 px-2.5 py-1.5 text-[11px] text-fg">
            <input type="checkbox" bind:checked={injectSafeGridGuide} class="accent-accent" />
            <span>{i18n.t('char_inject_grid_guide')}</span>
          </label>

          <button
            type="button"
            onclick={handleGenerateSpriteSheet}
            disabled={isGenerating}
            class="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-xs font-extrabold text-accent-ink shadow-md hover:brightness-105 disabled:opacity-40"
          >
            {#if isGenerating}
              <Loader2 size={15} class="animate-spin" />
              <span>{i18n.t('char_generating_sprite')} ({sheetRows * sheetCols}F)...</span>
            {:else}
              <Sparkles size={15} />
              <span>{i18n.t('char_generate_sprite_btn')} ({sheetRows * sheetCols}F)</span>
            {/if}
          </button>
        </aside>
      </div>

    <!-- ====================================================================== -->
    <!-- MODULE 3: Expression Sheet Studio                                      -->
    <!-- ====================================================================== -->
    {:else if activeModule === 'expression'}
      <div class="flex min-h-0 flex-1 overflow-hidden">
        <div class="flex min-w-0 flex-1 flex-col overflow-y-auto p-4">
          {#if exprSheetDataUrl}
            <div class="flex flex-col gap-4 rounded-2xl border border-line bg-ink-1/90 p-4 shadow-lg">
              <div class="flex items-center justify-between border-b border-line pb-2.5">
                <div class="flex items-center gap-2">
                  <Smile size={16} class="text-accent" />
                  <span class="text-xs font-extrabold text-fg">
                    {i18n.t('char_expr_preview_title')} ({exprFrames.length} {i18n.t('char_expr_unit')})
                  </span>
                </div>
              </div>
              <div class="grid grid-cols-3 gap-3">
                {#each exprFrames as ef (ef.index)}
                  <div class="flex flex-col items-center rounded-xl border border-line bg-ink-0 p-3">
                    <img src={ef.dataUrl} alt="Expr #{ef.index + 1}" class="h-32 w-32 object-contain" />
                    <div class="mt-2 flex w-full items-center justify-between border-t border-line pt-2 text-[11px]">
                      <span class="font-mono font-bold text-fg-muted">{i18n.t('char_expr_item')} #{ef.index + 1}</span>
                      <button
                        type="button"
                        onclick={async () => {
                          const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
                          await api.exportStudioPng(ef.dataUrl, `expr_${ef.index + 1}_${ts}.png`, true);
                          appStore.toast('ok', `Exported #${ef.index + 1}`);
                        }}
                        class="text-accent hover:underline"
                      >
                        {i18n.t('char_export_png')}
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {:else}
            <div class="flex flex-1 flex-col items-center justify-center rounded-2xl border border-dashed border-line bg-ink-1/40 p-8 text-center">
              <Smile size={36} class="mb-2 text-accent" />
              <h3 class="text-sm font-extrabold text-fg">{i18n.t('char_expr_empty_title')}</h3>
              <p class="mt-1 max-w-md text-xs text-fg-muted">
                {i18n.t('char_expr_empty_sub')}
              </p>
            </div>
          {/if}
        </div>

        <aside class="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-line bg-ink-1/95 p-3.5">
          <span class="text-xs font-extrabold text-fg">{i18n.t('char_expr_cfg_title')}</span>
          <div class="flex flex-col gap-1">
            <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_grid_spec_label')}</span>
            <select
              bind:value={exprGrid}
              onchange={() => rebuildExpressionSlices()}
              class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs font-semibold text-fg"
            >
              <option value="2x3">2 × 3 (6 Expressions)</option>
              <option value="2x2">2 × 2 (4 Expressions)</option>
            </select>
          </div>

          <div class="flex flex-col gap-1">
            <span class="text-[11px] font-semibold text-fg-muted">{i18n.t('char_expr_prompt_label')}</span>
            <textarea
              rows="5"
              bind:value={exprPrompt}
              placeholder={i18n.t('char_expr_prompt_ph')}
              class="w-full resize-y rounded-xl border border-line bg-ink-0 p-2.5 text-xs leading-relaxed text-fg outline-none focus:border-accent"
            ></textarea>
          </div>

          <button
            type="button"
            onclick={handleGenerateExpressionSheet}
            disabled={isGenerating}
            class="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-xs font-extrabold text-accent-ink shadow-md"
          >
            <Sparkles size={15} />
            <span>{i18n.t('char_generate_expr_btn')}</span>
          </button>
        </aside>
      </div>
    {/if}

    <!-- ====================================================================== -->
    <!-- BOTTOM FILMSTRIP: Explicit Role-Separated Actions per Module!          -->
    <!-- ====================================================================== -->
    <div class="flex h-28 shrink-0 flex-col border-t border-line bg-ink-1/95 px-4 py-2">
      <div class="mb-1.5 flex items-center justify-between text-xs">
        <div class="flex items-center gap-2">
          <FolderOpen size={13} class="text-accent" />
          <span class="font-bold text-fg">
            {#if activeModule === 'spritesheet'}
              {i18n.t('char_filmstrip_sprite_hint')}
            {:else}
              {i18n.t('char_filmstrip_default_hint')}
            {/if}
          </span>
          <select
            bind:value={charProjectFilter}
            onchange={() => refreshGallery()}
            class="rounded-md border border-line bg-ink-0 px-2 py-0.5 text-[11px] font-semibold text-fg outline-none focus:border-accent"
          >
            <option value="_all">{i18n.t('project_all')}</option>
            {#each appStore.projects as p (p.name)}
              <option value={p.name}>{p.name} ({p.assetCount})</option>
            {/each}
          </select>
        </div>

        <button
          type="button"
          onclick={() => refreshGallery()}
          class="flex items-center gap-1 text-[11px] text-fg-muted hover:text-fg"
        >
          <RefreshCw size={11} />
          <span>{i18n.t('studio_refresh')}</span>
        </button>
      </div>

      <div class="flex flex-1 items-center gap-2.5 overflow-x-auto pb-1">
        {#each galleryAssets as item (item.id)}
          <div
            class="group relative h-16 w-16 shrink-0 overflow-hidden rounded-xl border border-line bg-ink-0 transition-all hover:border-accent"
          >
            <img
              src={assetUrl(item.filePath)}
              alt={item.prompt}
              class="h-full w-full object-contain p-1"
            />
            {#if activeModule === 'spritesheet'}
              <div
                class="absolute inset-0 flex flex-col items-center justify-center gap-1 bg-ink-0/90 p-1 opacity-0 transition-opacity group-hover:opacity-100"
              >
                <button
                  type="button"
                  onclick={() => setSpriteCharRefFromAsset(item, true)}
                  class="w-full rounded bg-accent px-1 py-0.5 text-[9px] font-extrabold text-accent-ink hover:brightness-110"
                >
                  {i18n.t('char_btn_set_ref')}
                </button>
                <button
                  type="button"
                  onclick={() => loadSpriteSheetAsset(item, true)}
                  class="w-full rounded border border-line bg-ink-2 px-1 py-0.5 text-[9px] font-bold text-fg hover:border-accent hover:text-accent"
                >
                  {i18n.t('char_btn_load_slice')}
                </button>
              </div>
            {:else if activeModule === 'turnaround_skin'}
              <button
                type="button"
                onclick={() => setDesignAnchorFromAsset(item, true)}
                class="absolute inset-0 flex items-center justify-center bg-ink-0/80 text-[10px] font-extrabold text-accent opacity-0 transition-opacity group-hover:opacity-100"
              >
                {i18n.t('char_btn_set_design')}
              </button>
            {:else}
              <button
                type="button"
                onclick={() => setExprCharRefFromAsset(item, true)}
                class="absolute inset-0 flex items-center justify-center bg-ink-0/80 text-[10px] font-extrabold text-accent opacity-0 transition-opacity group-hover:opacity-100"
              >
                {i18n.t('char_btn_set_expr')}
              </button>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>
