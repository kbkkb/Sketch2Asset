<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Sparkles,
    ImagePlus,
    X,
    FolderKanban,
    FolderPlus,
    FolderOpen,
    Copy,
    Trash2,
    Layers,
    AlertTriangle,
    Loader2,
    Check,
    PanelRightClose,
    PanelRightOpen,
    Pin,
    SlidersHorizontal,
    ShieldAlert,
    Wand2,
    Star,
    Pencil,
    Maximize2,
    RefreshCw
  } from '@lucide/svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { jobsStore, type ActiveJob } from '$lib/stores/jobs.svelte';
  import { loadPersistedState, savePersistedState } from '$lib/stores/workspace';
  import { i18n } from '$lib/i18n/index.svelte';
  import { api, assetUrl } from '$lib/api';
  import type { AssetItem } from '$lib/api/types';
  import ProviderModelPicker from '$lib/components/ui/ProviderModelPicker.svelte';
  import { snapDataUrlToVirtualGrid } from '$lib/imaging/pixel-clean';
  import {
    ARCHETYPE_PRESETS,
    STYLE_PRESETS,
    BACKGROUND_PRESETS,
    NEGATIVE_GUARDRAILS,
    expandPromptVariables
  } from './presets';

  let selectedArchetype = $state('item_icon');
  let galleryArchetypeFilter = $state('all');
  let selectedStyle = $state('default');
  let selectedBg = $state('white');
  let selectedSize = $state('1024x1024');
  let customSizeW = $state(1920);
  let customSizeH = $state(1080);
  let batchCount = $state(1);

  // Structured Prompt Craft & Guardrails state
  let craftBarOpen = $state(true);
  let activeModifierIds = $state<string[]>([]);
  let activeNegativeIds = $state<string[]>(['no_text', 'no_crop']);
  let customNegativeDraft = $state('');

  // Project Hub & Style Reference Anchors state
  let drawerOpen = $state(true);
  let projectStyleDraft = $state('');
  let projectRefImages = $state<string[]>([]);
  let autoInjectStyleRef = $state(true);
  let creatingProject = $state(false);
  let newProjectNameDraft = $state('');
  let renamingProjectName = $state<string | null>(null);
  let renameProjectDraft = $state('');

  let lightboxAsset = $state<AssetItem | null>(null);
  let contextMenu = $state<{ x: number; y: number; asset: AssetItem } | null>(null);
  let fileInputEl = $state<HTMLInputElement | null>(null);
  let projRefInputEl = $state<HTMLInputElement | null>(null);

  let currentArchetypeObj = $derived(
    ARCHETYPE_PRESETS.find((a) => a.id === selectedArchetype) ?? ARCHETYPE_PRESETS[0]!
  );

  let filteredAssets = $derived(
    galleryArchetypeFilter === 'all'
      ? jobsStore.assets
      : jobsStore.assets.filter(
          (a) => (a.archetype || 'item_icon') === galleryArchetypeFilter
        )
  );

  let expandedPrompts = $derived(expandPromptVariables(jobsStore.promptDraft, 16));

  let settingsHydrated = $state(false);
  let draftHydrated = $state(false);
  let lastLoadedProject = $state<string | null>(null);

  $effect(() => {
    const s = appStore.settings;
    if (!s) return;
    if (!settingsHydrated) {
      const savedDraft = loadPersistedState('generate', {
        promptDraft: '',
        selectedArchetype: s.defaultArchetype || 'item_icon',
        selectedStyle: s.defaultStyle || 'default',
        selectedBg: s.defaultBackground || 'white',
        selectedSize: s.defaultSize || '1024x1024',
        batchCount: 1,
        craftBarOpen: true,
        drawerOpen: true,
        activeModifierIds: [] as string[],
        activeNegativeIds: ['no_text', 'no_crop'] as string[],
        customNegativeDraft: '',
        autoInjectStyleRef: true
      });
      if (savedDraft.promptDraft && !jobsStore.promptDraft) {
        jobsStore.promptDraft = savedDraft.promptDraft;
      }
      selectedArchetype = savedDraft.selectedArchetype || s.defaultArchetype || 'item_icon';
      selectedStyle = savedDraft.selectedStyle || s.defaultStyle || 'default';
      selectedBg = savedDraft.selectedBg || s.defaultBackground || 'white';
      selectedSize = savedDraft.selectedSize || s.defaultSize || '1024x1024';
      batchCount = savedDraft.batchCount || 1;
      craftBarOpen = savedDraft.craftBarOpen ?? true;
      drawerOpen = savedDraft.drawerOpen ?? true;
      activeModifierIds = Array.isArray(savedDraft.activeModifierIds) ? savedDraft.activeModifierIds : [];
      activeNegativeIds = Array.isArray(savedDraft.activeNegativeIds)
        ? savedDraft.activeNegativeIds
        : ['no_text', 'no_crop'];
      customNegativeDraft = savedDraft.customNegativeDraft || '';
      autoInjectStyleRef = savedDraft.autoInjectStyleRef ?? true;
      settingsHydrated = true;
      draftHydrated = true;
    }
    const targetProj = s.activeProject ?? '';
    if (lastLoadedProject !== targetProj) {
      lastLoadedProject = targetProj;
      syncProjectDraft();
      jobsStore.loadAssets(targetProj);
    }
  });

  $effect(() => {
    if (!draftHydrated) return;
    savePersistedState('generate', {
      promptDraft: jobsStore.promptDraft,
      selectedArchetype,
      selectedStyle,
      selectedBg,
      selectedSize,
      batchCount,
      craftBarOpen,
      drawerOpen,
      activeModifierIds,
      activeNegativeIds,
      customNegativeDraft,
      autoInjectStyleRef
    });
  });

  $effect(() => {
    // Re-sync project style & reference images once appStore.projects finishes loading
    if (appStore.projects.length >= 0 && appStore.settings) {
      syncProjectDraft();
    }
  });

  $effect(() => {
    // 监听视图切入或全局资产版本变动，确保生图工坊画廊实时刷新
    const isGen = appStore.activeView === 'generate';
    const _rev = appStore.assetsRevision;
    if (isGen && appStore.settings && settingsHydrated) {
      const targetProj = appStore.settings.activeProject ?? '';
      jobsStore.loadAssets(targetProj);
    }
  });

  onMount(async () => {
    if (appStore.settings) {
      const targetProj = appStore.settings.activeProject ?? '';
      lastLoadedProject = targetProj;
      syncProjectDraft();
      await jobsStore.loadAssets(targetProj);
    }
  });

  function syncProjectDraft() {
    const info = appStore.activeProjectInfo;
    projectStyleDraft = info?.stylePrompt ?? '';
    projectRefImages = [...(info?.referenceImages ?? [])];
    if (info?.style) {
      selectedStyle = info.style;
    }
  }

  function toggleModifierTag(id: string) {
    if (activeModifierIds.includes(id)) {
      activeModifierIds = activeModifierIds.filter((x) => x !== id);
    } else {
      activeModifierIds = [...activeModifierIds, id];
    }
  }

  function toggleNegativeId(id: string) {
    if (activeNegativeIds.includes(id)) {
      activeNegativeIds = activeNegativeIds.filter((x) => x !== id);
    } else {
      activeNegativeIds = [...activeNegativeIds, id];
    }
  }

  function handleArchetypeChange(archId: string) {
    selectedArchetype = archId;
    activeModifierIds = [];
    const arch = ARCHETYPE_PRESETS.find((a) => a.id === archId);
    if (archId === 'pixel_sprite') selectedStyle = 'pixel';
    if (arch?.recommendedSize && arch.recommendedSize !== 'auto') {
      selectedSize = arch.recommendedSize;
    }
  }

  async function selectProject(projName: string) {
    renamingProjectName = null;
    lastLoadedProject = projName;
    await appStore.saveSettings({ activeProject: projName });
    syncProjectDraft();
    await jobsStore.loadAssets(projName);
  }

  function startNewProject() {
    creatingProject = true;
    newProjectNameDraft = '';
  }

  async function confirmNewProject() {
    const clean = newProjectNameDraft.trim();
    if (!clean) {
      creatingProject = false;
      return;
    }
    try {
      appStore.projects = await api.upsertProject(clean, selectedStyle, '', []);
      creatingProject = false;
      newProjectNameDraft = '';
      await selectProject(clean);
      appStore.toast('ok', i18n.t('toast_project_saved'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to create project');
    }
  }

  function startRenameProject(projName: string, e: MouseEvent) {
    e.stopPropagation();
    renamingProjectName = projName;
    renameProjectDraft = projName;
  }

  async function confirmRenameProject(oldName: string) {
    const next = renameProjectDraft.trim();
    if (!next || next === oldName) {
      renamingProjectName = null;
      return;
    }
    try {
      appStore.projects = await api.renameProject(oldName, next);
      renamingProjectName = null;
      await selectProject(next);
      appStore.toast('ok', i18n.t('toast_project_saved'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to rename project');
    }
  }

  async function handleSaveProjectStyle() {
    const cur = appStore.settings?.activeProject;
    if (!cur) return;
    try {
      appStore.projects = await api.upsertProject(
        cur,
        selectedStyle,
        projectStyleDraft,
        projectRefImages
      );
      appStore.toast('ok', i18n.t('toast_project_saved'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to save project style');
    }
  }

  async function pinAssetToProjectStyle(asset: AssetItem) {
    let curProj = appStore.settings?.activeProject;
    if (!curProj) {
      // If currently in uncategorized, use asset's project or prompt to create/select one
      curProj = asset.project || 'DefaultProject';
      await appStore.saveSettings({ activeProject: curProj });
    }
    const existing = projectRefImages.filter((p) => p !== asset.filePath);
    const updated = [asset.filePath, ...existing].slice(0, 4);
    projectRefImages = updated;
    try {
      appStore.projects = await api.upsertProject(
        curProj,
        selectedStyle,
        projectStyleDraft,
        updated
      );
      appStore.toast('ok', i18n.t('toast_project_saved'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to pin reference image');
    }
  }

  function handleAddProjectRefFile(file: File) {
    if (!file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = async () => {
      if (typeof reader.result === 'string') {
        const updated = [reader.result, ...projectRefImages].slice(0, 4);
        projectRefImages = updated;
        const cur = appStore.settings?.activeProject;
        if (cur) {
          appStore.projects = await api.upsertProject(
            cur,
            selectedStyle,
            projectStyleDraft,
            updated
          );
          appStore.toast('ok', i18n.t('toast_project_saved'));
        }
      }
    };
    reader.readAsDataURL(file);
  }

  async function setPrimaryProjectRef(idx: number) {
    if (idx <= 0 || idx >= projectRefImages.length) return;
    const picked = projectRefImages[idx]!;
    const rest = projectRefImages.filter((_, i) => i !== idx);
    projectRefImages = [picked, ...rest];
    const cur = appStore.settings?.activeProject;
    if (cur) {
      appStore.projects = await api.upsertProject(
        cur,
        selectedStyle,
        projectStyleDraft,
        projectRefImages
      );
    }
  }

  async function removeProjectRef(idx: number) {
    projectRefImages = projectRefImages.filter((_, i) => i !== idx);
    const cur = appStore.settings?.activeProject;
    if (cur) {
      appStore.projects = await api.upsertProject(
        cur,
        selectedStyle,
        projectStyleDraft,
        projectRefImages
      );
    }
  }

  function handleSketchFile(file: File) {
    if (!file.type.startsWith('image/')) return;
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        jobsStore.sketchPreview = reader.result;
      }
    };
    reader.readAsDataURL(file);
  }

  function handlePaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of items) {
      if (item.type.startsWith('image/')) {
        const file = item.getAsFile();
        if (file) {
          handleSketchFile(file);
          break;
        }
      }
    }
  }

  async function triggerGenerate() {
    const prof = appStore.activeProfile;
    if (!prof) return;
    if (!prof.hasKey) {
      appStore.activeView = 'settings';
      appStore.toast('err', i18n.t('key_missing'));
      return;
    }
    const promptText = jobsStore.promptDraft.trim();
    if (!promptText && !jobsStore.sketchPreview && projectRefImages.length === 0) {
      return;
    }

    const archObj = ARCHETYPE_PRESETS.find((a) => a.id === selectedArchetype);
    const styleObj = STYLE_PRESETS.find((s) => s.id === selectedStyle);
    const bgObj = BACKGROUND_PRESETS.find((b) => b.id === selectedBg);

    // Collect active modifier snippets
    const modSnippets = (archObj?.modifiers ?? [])
      .filter((m) => activeModifierIds.includes(m.id))
      .map((m) => m.promptSnippet);

    const combinedArchetypePrompt = [archObj?.promptRule ?? '', ...modSnippets]
      .filter(Boolean)
      .join(' ');

    const isPixelMode = selectedArchetype === 'pixel_sprite' || selectedStyle === 'pixel';

    // Collect active negative guardrails + custom negative input
    const negParts = NEGATIVE_GUARDRAILS.filter((g) => activeNegativeIds.includes(g.id)).map(
      (g) => g.negRule
    );
    if (isPixelMode) {
      negParts.push(
        'mixels, inconsistent pixel sizes, sub-pixel details, anti-aliasing blur, soft airbrush gradients'
      );
    }
    if (customNegativeDraft.trim()) {
      negParts.push(customNegativeDraft.trim());
    }
    const compiledNegative = negParts.join(', ');

    // Determine reference image: explicit sketch slot takes priority, else project primary style reference if autoInjectStyleRef is on
    let effectiveSketchDataUrl: string | null = jobsStore.sketchPreview;
    let effectiveSketchPath: string | null = null;
    let extraStyleRefNote = '';

    // 像素模式下：对用户上传的草图/参考图预先做虚拟像素网格对齐，从源头引导大模型保持统一网格步长
    if (isPixelMode && effectiveSketchDataUrl) {
      const targetGridDim = activeModifierIds.includes('px32') ? 32 : 64;
      effectiveSketchDataUrl = await snapDataUrlToVirtualGrid(effectiveSketchDataUrl, targetGridDim);
    }

    if (
      !effectiveSketchDataUrl &&
      autoInjectStyleRef &&
      appStore.settings?.activeProject &&
      projectRefImages.length > 0
    ) {
      const primaryRef = projectRefImages[0]!;
      if (primaryRef.startsWith('data:')) {
        effectiveSketchDataUrl = primaryRef;
      } else {
        effectiveSketchPath = primaryRef;
      }
      extraStyleRefNote =
        'Match the artistic rendering style, brushwork, color palette, and shading language of the reference image.';
    }

    const finalProjectStylePrompt = [projectStyleDraft.trim(), extraStyleRefNote]
      .filter(Boolean)
      .join(' ');

    const clampedCount = Math.max(1, Math.min(16, Math.floor(Number(batchCount) || 1)));
    batchCount = clampedCount;

    const effectiveSize =
      selectedSize === 'custom'
        ? `${Math.max(64, Math.min(4096, Math.round(Number(customSizeW) || 1920)))}x${Math.max(64, Math.min(4096, Math.round(Number(customSizeH) || 1080)))}`
        : selectedSize;

    await appStore.saveSettings({
      defaultArchetype: selectedArchetype,
      defaultStyle: selectedStyle,
      defaultBackground: selectedBg,
      defaultSize: effectiveSize
    });

    const promptsToRun =
      expandedPrompts.length > 0 ? expandedPrompts : [promptText];

    for (const singlePrompt of promptsToRun) {
      jobsStore.enqueueGeneration(
        {
          profileId: prof.id,
          model: prof.apiModel,
          prompt: singlePrompt,
          archetype: selectedArchetype,
          negativePrompt: compiledNegative,
          archetypePrompt: combinedArchetypePrompt,
          stylePrompt: styleObj?.promptSuffix ?? '',
          projectStylePrompt: finalProjectStylePrompt,
          backgroundPrompt: bgObj?.promptRule ?? '',
          size: effectiveSize,
          project: appStore.settings?.activeProject ?? '',
          count: clampedCount,
          sketchDataUrl: effectiveSketchDataUrl,
          sketchPath: effectiveSketchPath
        },
        {
          archetype: selectedArchetype,
          style: selectedStyle,
          bg: selectedBg,
          size: selectedSize,
          count: clampedCount,
          modifierIds: [...activeModifierIds],
          negativeIds: [...activeNegativeIds],
          customNegative: customNegativeDraft,
          sketchPreview: jobsStore.sketchPreview
        }
      );
    }
  }

  async function handleReEditJob(job: ActiveJob) {
    jobsStore.promptDraft = job.prompt;
    if (job.uiSnapshot) {
      selectedArchetype = job.uiSnapshot.archetype;
      selectedStyle = job.uiSnapshot.style;
      selectedBg = job.uiSnapshot.bg;
      selectedSize = job.uiSnapshot.size;
      batchCount = job.uiSnapshot.count;
      activeModifierIds = [...job.uiSnapshot.modifierIds];
      activeNegativeIds = [...job.uiSnapshot.negativeIds];
      customNegativeDraft = job.uiSnapshot.customNegative;
      jobsStore.sketchPreview = job.uiSnapshot.sketchPreview;
    } else if (job.payload) {
      selectedArchetype = job.payload.archetype || selectedArchetype;
      selectedSize = job.payload.size || selectedSize;
      batchCount = job.payload.count || batchCount;
    }
    if (job.payload?.profileId && job.payload?.model) {
      await appStore.selectProviderAndModel(job.payload.profileId, job.payload.model);
    }
    jobsStore.dismissJob(job.id);
    appStore.toast('info', i18n.t('toast_job_restored'));
  }

  async function useAssetAsSketch(asset: AssetItem) {
    jobsStore.sketchPreview = assetUrl(asset.filePath);
    // Keep filePath reference via data url or path
    try {
      const resp = await fetch(assetUrl(asset.filePath));
      const blob = await resp.blob();
      handleSketchFile(new File([blob], 'ref.png', { type: 'image/png' }));
    } catch {
      // ignore
    }
  }

  async function removeAsset(asset: AssetItem) {
    try {
      await api.deleteAsset(asset.id);
      jobsStore.assets = jobsStore.assets.filter((a) => a.id !== asset.id);
      appStore.notifyAssetsChanged();
      await appStore.refreshProjects();
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to delete asset');
    }
  }

  async function changeAssetArchetype(asset: AssetItem, newArchId: string) {
    try {
      await api.updateAssetArchetype(asset.id, newArchId);
      jobsStore.assets = jobsStore.assets.map((a) =>
        a.id === asset.id ? { ...a, archetype: newArchId } : a
      );
      appStore.notifyAssetsChanged();
      appStore.toast('ok', i18n.t('toast_archetype_updated'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to update category');
    }
  }
</script>

<svelte:window
  onpaste={handlePaste}
  onclick={() => {
    if (contextMenu) contextMenu = null;
  }}
/>

<div class="relative flex flex-1 overflow-hidden">
  <!-- Main Stage: Timeline Stream + Floating Command Dock -->
  <div class="stage-backdrop relative flex flex-1 flex-col overflow-hidden">
    <!-- Top Sub-bar: Gallery Category Filter & Project Drawer Toggle -->
    <div
      class="flex items-center justify-between border-b border-line bg-ink-0/70 px-5 py-2.5 backdrop-blur-sm"
    >
      <div class="flex flex-wrap items-center gap-1.5">
        <span class="mr-1 font-mono text-[10px] font-semibold tracking-wider whitespace-nowrap text-fg-muted uppercase">
          {i18n.t('arch_filter_label')}
        </span>
        <button
          type="button"
          onclick={() => (galleryArchetypeFilter = 'all')}
          class="rounded-full border px-3 py-1 text-xs font-medium whitespace-nowrap transition-all {galleryArchetypeFilter ===
          'all'
            ? 'border-accent bg-accent text-accent-ink font-semibold shadow-sm'
            : 'border-line bg-ink-1 text-fg-muted hover:border-line-strong hover:text-fg'}"
        >
          {i18n.t('arch_all')}
        </button>
        {#each ARCHETYPE_PRESETS as arch (arch.id)}
          <button
            type="button"
            onclick={() => {
              galleryArchetypeFilter = arch.id;
              handleArchetypeChange(arch.id);
            }}
            class="rounded-full border px-3 py-1 text-xs font-medium whitespace-nowrap transition-all {galleryArchetypeFilter ===
            arch.id
              ? 'border-accent bg-accent text-accent-ink font-semibold shadow-sm'
              : 'border-line bg-ink-1 text-fg-muted hover:border-line-strong hover:text-fg'}"
          >
            {i18n.t(arch.labelKey)}
          </button>
        {/each}
      </div>

      <button
        type="button"
        onclick={() => (drawerOpen = !drawerOpen)}
        class="flex shrink-0 items-center gap-1.5 rounded-md border border-line bg-ink-1 px-2.5 py-1 text-xs font-medium whitespace-nowrap text-fg-muted transition-colors hover:border-line-strong hover:text-fg"
      >
        {#if drawerOpen}
          <PanelRightClose size={14} strokeWidth={1.75} />
        {:else}
          <PanelRightOpen size={14} strokeWidth={1.75} />
        {/if}
        <span>{i18n.t('project_hub_title')}</span>
      </button>
    </div>

    <!-- Scrollable Asset Stream -->
    <div class="flex-1 overflow-y-auto px-6 pt-5 pb-48">
      {#if jobsStore.activeJobs.length === 0 && filteredAssets.length === 0}
        <!-- Avant-garde Stage Empty Hero -->
        <div class="mx-auto flex h-full max-w-2xl flex-col items-center justify-center text-center">
          <div
            class="mb-3 rounded-full border border-accent/30 bg-accent/10 px-3.5 py-1 font-mono text-[11px] font-semibold tracking-widest text-accent"
          >
            {i18n.t('stage_empty_eyebrow')}
          </div>
          <h2
            class="font-display text-3xl font-extrabold tracking-tight text-fg sm:text-4xl"
          >
            {i18n.t('stage_empty_title')}
          </h2>
          <p class="mt-3 max-w-lg text-sm leading-relaxed text-fg-muted">
            {i18n.t('stage_empty_sub')}
          </p>
        </div>
      {:else}
        <div
          class="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5"
        >
          <!-- Active Running / Error Jobs -->
          {#each jobsStore.activeJobs as job (job.id)}
            <div
              class="relative flex aspect-square flex-col justify-between overflow-hidden rounded-xl border {job.status ===
              'error'
                ? 'border-err/50 bg-err/10'
                : 'border-accent/40 bg-ink-1'} p-4"
            >
              {#if job.status === 'running'}
                <div class="flex items-center justify-between">
                  <span
                    class="flex items-center gap-1.5 font-mono text-xs font-semibold text-accent"
                  >
                    <Loader2 size={14} class="animate-spin" />
                    <span>{job.elapsedSec}s</span>
                  </span>
                  <span
                    class="rounded bg-ink-2 px-2 py-0.5 font-mono text-[10px] text-fg-muted"
                  >
                    ×{job.count}
                  </span>
                </div>
                <div class="my-auto flex flex-col items-center text-center">
                  <div class="text-xs font-semibold text-fg">{i18n.t('job_running_title')}</div>
                  <div class="mt-1 line-clamp-2 text-[11px] text-fg-muted">{job.prompt}</div>
                </div>
                <div class="truncate font-mono text-[10px] text-fg-subtle">{job.model}</div>
              {:else}
                <!-- Top Bar: Error Title + X Dismiss Button -->
                <div class="flex items-center justify-between text-err">
                  <div class="flex items-center gap-1.5 text-xs font-bold">
                    <AlertTriangle size={15} class="shrink-0" />
                    <span>{i18n.t('job_failed_title')}</span>
                  </div>
                  <button
                    type="button"
                    onclick={() => jobsStore.dismissJob(job.id)}
                    title={i18n.t('job_dismiss_title')}
                    class="rounded-md p-1 text-fg-muted transition-colors hover:bg-err hover:text-white"
                  >
                    <X size={14} strokeWidth={2} />
                  </button>
                </div>

                <!-- Middle: Error Details & Original Prompt Preview -->
                <div class="my-auto flex flex-col gap-1.5 overflow-hidden py-1">
                  <p class="line-clamp-3 font-mono text-[11px] leading-snug text-err/90">
                    {job.errorMsg}
                  </p>
                  {#if job.prompt}
                    <div
                      class="line-clamp-2 rounded bg-ink-0/60 px-2 py-1 text-[10px] text-fg-muted"
                    >
                      {job.prompt}
                    </div>
                  {/if}
                </div>

                <!-- Bottom Action Buttons: Retry (重新生成) + Re-edit (重新编辑覆盖下方设置) -->
                <div class="flex flex-col gap-1.5">
                  <div class="truncate font-mono text-[10px] text-fg-subtle">{job.model}</div>
                  <div class="grid grid-cols-2 gap-1.5">
                    <button
                      type="button"
                      onclick={() => jobsStore.retryJob(job.id)}
                      class="flex items-center justify-center gap-1 rounded-lg bg-accent px-2 py-1.5 text-[11px] font-bold text-accent-ink transition-colors hover:bg-accent-hover"
                    >
                      <RefreshCw size={12} strokeWidth={2.2} />
                      <span>{i18n.t('job_retry_btn')}</span>
                    </button>
                    <button
                      type="button"
                      onclick={() => handleReEditJob(job)}
                      class="flex items-center justify-center gap-1 rounded-lg border border-line-strong bg-ink-2 px-2 py-1.5 text-[11px] font-semibold text-fg transition-colors hover:border-accent hover:text-accent"
                    >
                      <Pencil size={12} strokeWidth={2} />
                      <span>{i18n.t('job_reedit_btn')}</span>
                    </button>
                  </div>
                </div>
              {/if}
            </div>
          {/each}

          <!-- Generated / Archived Assets -->
          {#each filteredAssets as asset (asset.id)}
            {@const archMeta = ARCHETYPE_PRESETS.find((a) => a.id === (asset.archetype || 'item_icon'))}
            <div
              role="presentation"
              oncontextmenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                contextMenu = {
                  x: Math.min(e.clientX, window.innerWidth - 240),
                  y: Math.min(e.clientY, window.innerHeight - 340),
                  asset
                };
              }}
              class="group relative flex aspect-square flex-col overflow-hidden rounded-xl border border-line bg-ink-1 transition-all duration-200 hover:-translate-y-0.5 hover:border-accent/60 hover:shadow-xl"
            >
              <!-- Image Viewport -->
              <button
                type="button"
                onclick={() => (lightboxAsset = asset)}
                class="checkerboard relative flex-1 overflow-hidden"
              >
                <img
                  src={assetUrl(asset.filePath)}
                  alt={asset.prompt || asset.id}
                  loading="lazy"
                  draggable="false"
                  class="h-full w-full object-contain transition-transform duration-300 group-hover:scale-[1.03]"
                />
              </button>

              <!-- Hover Quick Actions Bar -->
              <div
                class="absolute top-2 right-2 flex items-center gap-1 rounded-lg border border-line-strong bg-ink-0/85 p-1 opacity-0 backdrop-blur-md transition-opacity group-hover:opacity-100"
              >
                <button
                  type="button"
                  onclick={() => appStore.openInStudio(asset)}
                  title={i18n.t('asset_send_to_studio')}
                  class="rounded bg-accent/15 p-1.5 text-accent hover:bg-accent hover:text-accent-ink"
                >
                  <SlidersHorizontal size={13} strokeWidth={2} />
                </button>
                <button
                  type="button"
                  onclick={() => pinAssetToProjectStyle(asset)}
                  title={i18n.t('asset_pin_style_ref')}
                  class="rounded p-1.5 text-fg-muted hover:bg-ink-2 hover:text-accent"
                >
                  <Pin size={13} strokeWidth={1.8} />
                </button>
                <button
                  type="button"
                  onclick={() => useAssetAsSketch(asset)}
                  title={i18n.t('asset_use_as_sketch')}
                  class="rounded p-1.5 text-fg-muted hover:bg-ink-2 hover:text-accent"
                >
                  <ImagePlus size={13} strokeWidth={1.8} />
                </button>
                {#if asset.prompt}
                  <button
                    type="button"
                    onclick={() => {
                      jobsStore.promptDraft = asset.prompt;
                      appStore.toast('ok', i18n.t('toast_copied'));
                    }}
                    title={i18n.t('asset_copy_prompt')}
                    class="rounded p-1.5 text-fg-muted hover:bg-ink-2 hover:text-fg"
                  >
                    <Copy size={13} strokeWidth={1.8} />
                  </button>
                {/if}
                <button
                  type="button"
                  onclick={() => api.revealInExplorer(asset.filePath)}
                  title={i18n.t('asset_open_folder')}
                  class="rounded p-1.5 text-fg-muted hover:bg-ink-2 hover:text-fg"
                >
                  <FolderOpen size={13} strokeWidth={1.8} />
                </button>
                <button
                  type="button"
                  onclick={() => removeAsset(asset)}
                  title={i18n.t('asset_delete')}
                  class="rounded p-1.5 text-fg-muted hover:bg-err hover:text-white"
                >
                  <Trash2 size={13} strokeWidth={1.8} />
                </button>
              </div>

              <!-- Footer Metadata -->
              <div class="flex items-center justify-between gap-1 border-t border-line bg-ink-1 px-2.5 py-1.5">
                <div class="flex items-center gap-1.5 truncate">
                  {#if archMeta}
                    <span class="rounded bg-ink-2 px-1.5 py-0.5 text-[10px] font-medium text-fg-muted">
                      {i18n.t(archMeta.labelKey)}
                    </span>
                  {/if}
                  <span class="truncate font-mono text-[10px] text-fg-subtle">
                    {asset.width}×{asset.height}
                  </span>
                </div>
                {#if asset.project}
                  <span
                    class="max-w-[90px] truncate rounded bg-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-accent"
                  >
                    {asset.project}
                  </span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Bottom Floating Command Dock -->
    <div
      class="pointer-events-none absolute right-0 bottom-4 left-0 flex justify-center px-6"
    >
      <div
        class="pointer-events-auto flex w-full max-w-5xl flex-col gap-2.5 rounded-2xl border border-line-strong bg-ink-1/95 p-3.5 shadow-2xl backdrop-blur-xl"
      >
        <!-- Row 1: Unified Provider/Model Picker + Archetype + Style + Background + Size + Custom Batch Count -->
        <div class="flex flex-wrap items-center gap-2 text-xs">
          <!-- Single Unified Provider-Grouped Model Picker -->
          <ProviderModelPicker />

          <div class="h-4 w-px bg-line"></div>

          <!-- Asset Archetype (生图品类) Selector right in Command Dock! -->
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-[10px] font-bold text-accent uppercase">{i18n.t('archetype_label')}</span>
            <select
              value={selectedArchetype}
              onchange={(e) => handleArchetypeChange((e.currentTarget as HTMLSelectElement).value)}
              class="rounded-md border border-accent/40 bg-ink-2 px-2 py-1 text-xs font-semibold text-fg outline-none focus:border-accent"
            >
              {#each ARCHETYPE_PRESETS as arch (arch.id)}
                <option value={arch.id}>{i18n.t(arch.labelKey)}</option>
              {/each}
            </select>
          </div>

          <!-- Art Style -->
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-[10px] text-fg-muted uppercase">{i18n.t('style_label')}</span>
            <select
              bind:value={selectedStyle}
              class="rounded-md border border-line bg-ink-2 px-2 py-1 text-xs text-fg outline-none focus:border-accent"
            >
              {#each STYLE_PRESETS as st (st.id)}
                <option value={st.id}>{i18n.t(st.labelKey)}</option>
              {/each}
            </select>
          </div>

          <!-- Background Rule -->
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-[10px] text-fg-muted uppercase">{i18n.t('bg_label')}</span>
            <select
              bind:value={selectedBg}
              class="rounded-md border border-line bg-ink-2 px-2 py-1 text-xs text-fg outline-none focus:border-accent"
            >
              {#each BACKGROUND_PRESETS as bg (bg.id)}
                <option value={bg.id}>{i18n.t(bg.labelKey)}</option>
              {/each}
            </select>
          </div>

          <!-- Aspect Size -->
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-[10px] text-fg-muted uppercase">{i18n.t('size_label')}</span>
            <select
              bind:value={selectedSize}
              class="rounded-md border border-line bg-ink-2 px-2 py-1 font-mono text-xs text-fg outline-none focus:border-accent"
            >
              <optgroup label={i18n.t('size_group_1k')}>
                <option value="1024x1024">{i18n.t('size_square')}</option>
                <option value="1536x1024">{i18n.t('size_landscape')}</option>
                <option value="1024x1536">{i18n.t('size_portrait')}</option>
                <option value="1344x768">{i18n.t('size_16_9_1k')}</option>
                <option value="768x1344">{i18n.t('size_9_16_1k')}</option>
              </optgroup>
              <optgroup label={i18n.t('size_group_fhd_2k')}>
                <option value="1920x1080">{i18n.t('size_1080p_land')}</option>
                <option value="1080x1920">{i18n.t('size_1080p_port')}</option>
                <option value="2048x2048">{i18n.t('size_2k_square')}</option>
                <option value="2560x1440">{i18n.t('size_2k_land')}</option>
              </optgroup>
              <optgroup label={i18n.t('size_group_4k')}>
                <option value="3840x2160">{i18n.t('size_4k_land')}</option>
                <option value="4096x4096">{i18n.t('size_4k_square')}</option>
              </optgroup>
              <optgroup label={i18n.t('size_group_custom')}>
                <option value="auto">{i18n.t('size_auto')}</option>
                <option value="custom">{i18n.t('size_custom')}</option>
              </optgroup>
            </select>
            {#if selectedSize === 'custom'}
              <div class="flex items-center gap-1 font-mono text-xs">
                <input
                  type="number"
                  min="64"
                  max="4096"
                  step="8"
                  bind:value={customSizeW}
                  class="w-16 rounded-md border border-accent/50 bg-ink-0 px-1.5 py-0.5 text-center text-xs font-semibold text-fg outline-none focus:border-accent"
                />
                <span class="text-fg-muted">×</span>
                <input
                  type="number"
                  min="64"
                  max="4096"
                  step="8"
                  bind:value={customSizeH}
                  class="w-16 rounded-md border border-accent/50 bg-ink-0 px-1.5 py-0.5 text-center text-xs font-semibold text-fg outline-none focus:border-accent"
                />
              </div>
            {/if}
          </div>

          <!-- Prompt Craft Toggle Button -->
          <button
            type="button"
            onclick={() => (craftBarOpen = !craftBarOpen)}
            class="flex items-center gap-1 rounded-md border px-2 py-1 text-[11px] font-medium transition-colors {craftBarOpen
              ? 'border-accent/50 bg-accent/15 text-accent'
              : 'border-line bg-ink-2 text-fg-muted hover:text-fg'}"
          >
            <SlidersHorizontal size={12} />
            <span>{i18n.t('prompt_craft_toggle')}</span>
          </button>

          <!-- Batch Count: Quick 1/2/4 Buttons + Custom Number Input (1..16) -->
          <div class="ml-auto flex items-center gap-1">
            <span class="mr-0.5 font-mono text-[10px] text-fg-muted uppercase">
              {i18n.t('batch_label')}
            </span>
            {#each [1, 2, 4] as cnt (cnt)}
              <button
                type="button"
                onclick={() => (batchCount = cnt)}
                class="rounded-md border px-2 py-0.5 font-mono text-xs font-semibold transition-colors {batchCount ===
                cnt
                  ? 'border-accent bg-accent/20 text-accent'
                  : 'border-line bg-ink-2 text-fg-muted hover:text-fg'}"
              >
                ×{cnt}
              </button>
            {/each}
            <input
              type="number"
              min="1"
              max="16"
              bind:value={batchCount}
              onchange={() => {
                batchCount = Math.max(1, Math.min(16, Math.floor(Number(batchCount) || 1)));
              }}
              title={i18n.t('batch_custom_title')}
              class="w-12 rounded-md border border-line bg-ink-0 px-1.5 py-0.5 text-center font-mono text-xs font-semibold text-accent outline-none focus:border-accent"
            />
          </div>
        </div>

        <!-- Row 2 (Collapsible): Structured Prompt Craft Tags & Negative Guardrails -->
        {#if craftBarOpen}
          <div
            class="flex flex-col gap-1.5 rounded-xl border border-line bg-ink-0/65 px-3 py-2 text-xs"
          >
            <!-- Archetype-specific Perspective / Material Modifiers -->
            <div class="flex flex-wrap items-center gap-1.5">
              <span class="flex items-center gap-1 font-mono text-[10px] text-fg-muted uppercase">
                <Wand2 size={11} class="text-accent" />
                <span>{i18n.t('prompt_modifiers_label')}:</span>
              </span>
              {#each currentArchetypeObj.modifiers as mod (mod.id)}
                {@const active = activeModifierIds.includes(mod.id)}
                <button
                  type="button"
                  onclick={() => toggleModifierTag(mod.id)}
                  class="rounded-md border px-2 py-0.5 text-[11px] font-medium transition-all {active
                    ? 'border-accent bg-accent/20 text-accent font-semibold'
                    : 'border-line bg-ink-2/80 text-fg-muted hover:border-line-strong hover:text-fg'}"
                >
                  + {i18n.locale === 'en' ? mod.en : mod.zh}
                </button>
              {/each}
            </div>

            <!-- Negative Guardrails + Custom Avoid Input -->
            <div class="flex flex-wrap items-center gap-1.5 border-t border-line/60 pt-1.5">
              <span class="flex items-center gap-1 font-mono text-[10px] text-fg-muted uppercase">
                <ShieldAlert size={11} class="text-warn" />
                <span>{i18n.t('prompt_neg_label')}:</span>
              </span>
              {#each NEGATIVE_GUARDRAILS as guard (guard.id)}
                {@const active = activeNegativeIds.includes(guard.id)}
                <button
                  type="button"
                  onclick={() => toggleNegativeId(guard.id)}
                  class="rounded-md border px-2 py-0.5 text-[11px] font-medium transition-all {active
                    ? 'border-warn/50 bg-warn/15 text-warn font-semibold'
                    : 'border-line bg-ink-2/60 text-fg-subtle hover:text-fg-muted'}"
                >
                  {i18n.locale === 'en' ? guard.en : guard.zh}
                </button>
              {/each}
              <input
                type="text"
                bind:value={customNegativeDraft}
                placeholder={i18n.t('prompt_neg_custom_ph')}
                class="min-w-[180px] flex-1 rounded border border-line bg-ink-1 px-2 py-0.5 text-[11px] text-fg outline-none focus:border-warn placeholder:text-fg-subtle"
              />
            </div>
          </div>
        {/if}

        <!-- Live Batch Variable [A|B|C] Detection Banner -->
        {#if expandedPrompts.length > 1}
          <div
            class="flex items-center justify-between rounded-lg border border-info/40 bg-info/10 px-3 py-1 text-[11px] text-info"
          >
            <div class="flex items-center gap-1.5 font-semibold">
              <Layers size={12} />
              <span>
                {i18n.t('prompt_var_detected')}
                <strong class="font-mono text-xs underline">{expandedPrompts.length}</strong>
                {i18n.t('prompt_var_jobs')}
              </span>
            </div>
            <div class="max-w-md truncate font-mono text-[10px] opacity-85">
              {expandedPrompts.join(' · ')}
            </div>
          </div>
        {/if}

        <!-- Row 3: Sketch Slot + Prompt Textarea + Forge Button -->
        <div class="flex items-stretch gap-3">
          <!-- Sketch / Reference Image Slot -->
          <input
            bind:this={fileInputEl}
            type="file"
            accept="image/*"
            hidden
            onchange={(e) => {
              const f = (e.currentTarget as HTMLInputElement).files?.[0];
              if (f) handleSketchFile(f);
              (e.currentTarget as HTMLInputElement).value = '';
            }}
          />

          {#if jobsStore.sketchPreview}
            <div
              class="group relative h-20 w-20 shrink-0 overflow-hidden rounded-xl border border-accent bg-ink-0"
            >
              <img
                src={jobsStore.sketchPreview}
                alt="Sketch reference"
                draggable="false"
                class="h-full w-full object-cover"
              />
              <button
                type="button"
                onclick={() => (jobsStore.sketchPreview = null)}
                title={i18n.t('sketch_remove')}
                class="absolute top-1 right-1 rounded-full bg-ink-0/85 p-1 text-fg hover:bg-err hover:text-white"
              >
                <X size={12} />
              </button>
            </div>
          {:else}
            <button
              type="button"
              onclick={() => fileInputEl?.click()}
              ondragover={(e) => e.preventDefault()}
              ondrop={(e) => {
                e.preventDefault();
                const f = e.dataTransfer?.files?.[0];
                if (f) handleSketchFile(f);
              }}
              title={i18n.t('sketch_slot_hint')}
              class="flex h-20 w-20 shrink-0 flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-line-strong bg-ink-2/60 p-2 text-center text-fg-muted transition-colors hover:border-accent hover:text-accent"
            >
              <ImagePlus size={18} strokeWidth={1.75} />
              <span class="text-[10px] leading-tight font-medium">{i18n.t('sketch_slot_title')}</span>
            </button>
          {/if}

          <!-- Prompt Textarea -->
          <textarea
            bind:value={jobsStore.promptDraft}
            onkeydown={(e) => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                triggerGenerate();
              }
            }}
            placeholder={i18n.t('prompt_placeholder')}
            rows="2"
            class="flex-1 resize-none rounded-xl border border-line bg-ink-0/80 px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent placeholder:text-fg-subtle"
          ></textarea>

          <!-- Primary Forge Button -->
          <button
            type="button"
            onclick={triggerGenerate}
            class="flex shrink-0 flex-col items-center justify-center gap-1 rounded-xl bg-accent px-6 font-display text-xs font-extrabold tracking-wider text-accent-ink uppercase shadow-lg transition-all hover:bg-accent-hover active:scale-[0.98]"
          >
            <Sparkles size={18} strokeWidth={2.2} />
            <span>{i18n.t('btn_generate')}</span>
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- Right Collapsible Project Hub Drawer -->
  {#if drawerOpen}
    <aside
      class="flex w-80 shrink-0 flex-col overflow-y-auto border-l border-line bg-ink-1 p-4"
    >
      <div class="mb-3 flex items-center justify-between">
        <div class="flex items-center gap-2 font-display text-xs font-bold tracking-wider uppercase">
          <FolderKanban size={15} class="text-accent" />
          <span>{i18n.t('project_hub_title')}</span>
        </div>
        <button
          type="button"
          onclick={startNewProject}
          title={i18n.t('project_new_btn')}
          class="flex items-center gap-1 rounded-md border border-line bg-ink-2 px-2 py-1 text-[11px] font-semibold text-fg-muted hover:border-accent hover:text-fg"
        >
          <FolderPlus size={13} />
          <span>{i18n.t('project_new_btn')}</span>
        </button>
      </div>

      <!-- Inline New Project Input -->
      {#if creatingProject}
        <div class="mb-2 flex items-center gap-1.5 rounded-lg border border-accent bg-ink-0 p-1.5">
          <input
            type="text"
            bind:value={newProjectNameDraft}
            placeholder={i18n.t('project_new_prompt')}
            onkeydown={(e) => {
              if (e.key === 'Enter') confirmNewProject();
              else if (e.key === 'Escape') creatingProject = false;
            }}
            class="w-full bg-transparent px-2 text-xs text-fg outline-none"
          />
          <button
            type="button"
            onclick={confirmNewProject}
            class="rounded bg-accent p-1 text-accent-ink"
          >
            <Check size={12} />
          </button>
          <button
            type="button"
            onclick={() => (creatingProject = false)}
            class="rounded bg-ink-2 p-1 text-fg-muted hover:text-fg"
          >
            <X size={12} />
          </button>
        </div>
      {/if}

      <!-- Project List -->
      <div class="flex max-h-52 flex-col gap-1 overflow-y-auto pr-0.5">
        <button
          type="button"
          onclick={() => selectProject('')}
          class="flex items-center justify-between rounded-lg px-3 py-2 text-left text-xs font-medium transition-colors {!appStore
            .settings?.activeProject
            ? 'bg-accent/15 font-semibold text-accent'
            : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
        >
          <span>{i18n.t('project_uncategorized')}</span>
          <Layers size={13} />
        </button>

        {#each appStore.projects as proj (proj.name)}
          {@const isProjActive = appStore.settings?.activeProject === proj.name}
          {#if renamingProjectName === proj.name}
            <div class="flex items-center gap-1.5 rounded-lg border border-accent bg-ink-0 px-2 py-1.5">
              <input
                type="text"
                bind:value={renameProjectDraft}
                onkeydown={(e) => {
                  if (e.key === 'Enter') confirmRenameProject(proj.name);
                  else if (e.key === 'Escape') renamingProjectName = null;
                }}
                class="w-full bg-transparent text-xs text-fg outline-none"
              />
              <button
                type="button"
                onclick={() => confirmRenameProject(proj.name)}
                class="rounded bg-accent p-1 text-accent-ink"
              >
                <Check size={12} />
              </button>
              <button
                type="button"
                onclick={() => (renamingProjectName = null)}
                class="rounded bg-ink-2 p-1 text-fg-muted hover:text-fg"
              >
                <X size={12} />
              </button>
            </div>
          {:else}
            <div
              class="group flex items-center justify-between rounded-lg px-3 py-2 text-xs font-medium transition-colors {isProjActive
                ? 'bg-accent/15 font-semibold text-accent'
                : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
            >
              <button
                type="button"
                onclick={() => selectProject(proj.name)}
                class="min-w-0 flex-1 truncate text-left"
              >
                {proj.name}
              </button>

              <div class="ml-2 flex shrink-0 items-center gap-1.5">
                {#if proj.referenceImages?.length > 0}
                  <span
                    class="rounded bg-accent/20 px-1 py-0.5 font-mono text-[9px] text-accent"
                    title={i18n.t('project_ref_images_label')}
                  >
                    REF×{proj.referenceImages.length}
                  </span>
                {/if}
                <span class="rounded bg-ink-2 px-1.5 py-0.5 font-mono text-[10px] text-fg-muted">
                  {proj.assetCount}
                </span>
                {#if isProjActive}
                  <button
                    type="button"
                    onclick={(e) => startRenameProject(proj.name, e)}
                    title={i18n.t('project_rename_btn')}
                    class="rounded p-1 text-accent transition-colors hover:bg-accent/25"
                  >
                    <Pencil size={12} strokeWidth={2} />
                  </button>
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </div>

      <!-- Project Style Settings & Style Reference Anchors -->
      {#if appStore.settings?.activeProject}
        <div class="mt-4 flex flex-col gap-3 border-t border-line pt-4">
          <!-- Default Art Style Preset for this Project -->
          <div class="flex flex-col gap-1">
            <label for="projStylePreset" class="text-[11px] font-semibold text-fg-muted">
              {i18n.t('project_style_preset_label')}
            </label>
            <select
              id="projStylePreset"
              bind:value={selectedStyle}
              onchange={handleSaveProjectStyle}
              class="rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs text-fg outline-none focus:border-accent"
            >
              {#each STYLE_PRESETS as st (st.id)}
                <option value={st.id}>{i18n.t(st.labelKey)}</option>
              {/each}
            </select>
          </div>

          <!-- Project Style Reference Images (Up to 4 Anchors) -->
          <div class="flex flex-col gap-1.5">
            <div class="flex items-center justify-between">
              <span class="text-[11px] font-semibold text-fg-muted">
                {i18n.t('project_ref_images_label')}
              </span>
              {#if projectRefImages.length < 4}
                <button
                  type="button"
                  onclick={() => projRefInputEl?.click()}
                  class="flex items-center gap-1 rounded border border-line bg-ink-2 px-2 py-0.5 text-[10px] font-medium text-accent hover:border-accent"
                >
                  <ImagePlus size={11} />
                  <span>{i18n.t('project_ref_add_btn')}</span>
                </button>
              {/if}
            </div>

            <input
              bind:this={projRefInputEl}
              type="file"
              accept="image/*"
              hidden
              onchange={(e) => {
                const f = (e.currentTarget as HTMLInputElement).files?.[0];
                if (f) handleAddProjectRefFile(f);
                (e.currentTarget as HTMLInputElement).value = '';
              }}
            />

            <p class="text-[10px] leading-relaxed text-fg-subtle">
              {i18n.t('project_ref_images_hint')}
            </p>

            {#if projectRefImages.length > 0}
              <div class="grid grid-cols-2 gap-2 pt-1">
                {#each projectRefImages as refPath, idx (refPath + idx)}
                  <div
                    class="group relative aspect-square overflow-hidden rounded-lg border {idx ===
                    0
                      ? 'border-accent ring-1 ring-accent/40'
                      : 'border-line'} bg-ink-0"
                  >
                    <img
                      src={assetUrl(refPath)}
                      alt="Style Reference"
                      draggable="false"
                      class="checkerboard h-full w-full object-contain"
                    />
                    {#if idx === 0}
                      <span
                        class="absolute top-1 left-1 flex items-center gap-0.5 rounded bg-accent px-1.5 py-0.5 font-mono text-[9px] font-bold text-accent-ink shadow"
                      >
                        <Star size={9} />
                        <span>{i18n.t('project_ref_primary_badge')}</span>
                      </span>
                    {:else}
                      <button
                        type="button"
                        onclick={() => setPrimaryProjectRef(idx)}
                        title={i18n.t('project_ref_set_primary')}
                        class="absolute top-1 left-1 rounded bg-ink-0/85 p-1 text-fg-muted opacity-0 transition-opacity group-hover:opacity-100 hover:text-accent"
                      >
                        <Star size={11} />
                      </button>
                    {/if}
                    <button
                      type="button"
                      onclick={() => removeProjectRef(idx)}
                      class="absolute top-1 right-1 rounded bg-ink-0/85 p-1 text-fg-muted opacity-0 transition-opacity group-hover:opacity-100 hover:bg-err hover:text-white"
                    >
                      <X size={11} />
                    </button>
                  </div>
                {/each}
              </div>

              <!-- Auto-inject primary reference toggle -->
              <label class="mt-1 flex cursor-pointer items-center gap-2 text-[11px] text-fg-muted">
                <input
                  type="checkbox"
                  bind:checked={autoInjectStyleRef}
                  class="accent-accent"
                />
                <span>{i18n.t('project_ref_auto_inject')}</span>
              </label>
            {/if}
          </div>

          <!-- Project Style Prompt Lock -->
          <div class="flex flex-col gap-1.5">
            <label for="projStyleInput" class="text-[11px] font-semibold text-fg-muted">
              {i18n.t('project_style_lock_label')}
            </label>
            <textarea
              id="projStyleInput"
              bind:value={projectStyleDraft}
              rows="3"
              placeholder={i18n.t('project_style_lock_ph')}
              class="w-full resize-none rounded-lg border border-line bg-ink-0 p-2.5 text-xs text-fg outline-none focus:border-accent"
            ></textarea>
            <button
              type="button"
              onclick={handleSaveProjectStyle}
              class="flex items-center justify-center gap-1.5 rounded-lg border border-accent/40 bg-accent/15 py-1.5 text-xs font-semibold text-accent transition-colors hover:bg-accent hover:text-accent-ink"
            >
              <Check size={13} />
              <span>{i18n.t('project_save_style')}</span>
            </button>
          </div>
        </div>
      {/if}
    </aside>
  {/if}
</div>

<!-- Lightbox Modal -->
{#if lightboxAsset}
  <div
    role="dialog"
    tabindex="-1"
    onclick={() => (lightboxAsset = null)}
    onkeydown={(e) => e.key === 'Escape' && (lightboxAsset = null)}
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-8 backdrop-blur-md"
  >
    <div
      role="presentation"
      onclick={(e) => e.stopPropagation()}
      class="relative flex max-h-full max-w-4xl flex-col overflow-hidden rounded-2xl border border-line-strong bg-ink-1 shadow-2xl"
    >
      <div class="checkerboard flex max-h-[75vh] items-center justify-center overflow-hidden p-4">
        <img
          src={assetUrl(lightboxAsset.filePath)}
          alt={lightboxAsset.prompt}
          draggable="false"
          class="max-h-[70vh] w-auto object-contain"
        />
      </div>
      <div class="flex items-center justify-between gap-4 border-t border-line bg-ink-0 px-5 py-3">
        <div class="min-w-0 flex-1">
          <div class="truncate text-xs font-medium text-fg">
            {lightboxAsset.prompt || lightboxAsset.filePath}
          </div>
          <div class="mt-0.5 font-mono text-[11px] text-fg-muted">
            {lightboxAsset.model} · {lightboxAsset.width}×{lightboxAsset.height}
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <button
            type="button"
            onclick={() => {
              const target = lightboxAsset!;
              lightboxAsset = null;
              appStore.openInStudio(target);
            }}
            class="flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-extrabold text-accent-ink hover:brightness-110"
          >
            <SlidersHorizontal size={14} strokeWidth={2.2} />
            <span>{i18n.t('asset_send_to_studio')}</span>
          </button>
          <button
            type="button"
            onclick={() => api.revealInExplorer(lightboxAsset!.filePath)}
            class="flex items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3 py-1.5 text-xs font-medium text-fg hover:border-accent"
          >
            <FolderOpen size={14} />
            <span>{i18n.t('asset_open_folder')}</span>
          </button>
          <button
            type="button"
            onclick={() => (lightboxAsset = null)}
            class="rounded-lg bg-ink-2 p-1.5 text-fg-muted hover:text-fg"
          >
            <X size={16} />
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Custom Darkroom Right-Click Context Menu for Asset Cards (No Edge Browser Menu!) -->
{#if contextMenu}
  <div
    role="menu"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && (contextMenu = null)}
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
    class="fixed z-50 flex w-60 flex-col gap-0.5 rounded-xl border border-line-strong bg-ink-1/95 p-2 text-xs shadow-2xl backdrop-blur-xl"
  >
    <!-- Change Asset Category (Archetype) Section -->
    <div class="mb-1 rounded-lg border border-line bg-ink-0/70 p-2">
      <div class="mb-1.5 font-mono text-[10px] font-bold tracking-wider text-fg-muted uppercase">
        {i18n.t('ctx_change_archetype')}
      </div>
      <div class="grid grid-cols-2 gap-1">
        {#each ARCHETYPE_PRESETS as arch (arch.id)}
          {@const isCurArch = (contextMenu.asset.archetype || 'item_icon') === arch.id}
          <button
            type="button"
            onclick={() => {
              changeAssetArchetype(contextMenu!.asset, arch.id);
              contextMenu = null;
            }}
            class="flex items-center justify-between rounded px-2 py-1 text-left text-[11px] font-medium transition-colors {isCurArch
              ? 'bg-accent text-accent-ink font-bold'
              : 'bg-ink-2/70 text-fg-muted hover:bg-ink-2 hover:text-fg'}"
          >
            <span class="truncate">{i18n.t(arch.labelKey)}</span>
            {#if isCurArch}
              <Check size={11} strokeWidth={2.5} />
            {/if}
          </button>
        {/each}
      </div>
    </div>

    <button
      type="button"
      onclick={() => {
        const target = contextMenu!.asset;
        contextMenu = null;
        appStore.openInStudio(target);
      }}
      class="flex items-center gap-2 rounded-lg bg-accent/15 px-2.5 py-1.5 text-left font-bold text-accent hover:bg-accent hover:text-accent-ink"
    >
      <SlidersHorizontal size={13} />
      <span>{i18n.t('asset_send_to_studio')}</span>
    </button>
    <button
      type="button"
      onclick={() => {
        const target = contextMenu!.asset;
        contextMenu = null;
        appStore.openInCharacterForge(target, 'outfit');
      }}
      class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-semibold text-fg hover:bg-ink-2 hover:text-accent"
    >
      <Sparkles size={13} />
      <span>{i18n.t('asset_send_to_character')}</span>
    </button>
    <button
      type="button"
      onclick={() => {
        lightboxAsset = contextMenu!.asset;
        contextMenu = null;
      }}
      class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-medium text-fg hover:bg-ink-2 hover:text-accent"
    >
      <Maximize2 size={13} />
      <span>{i18n.t('ctx_view_large')}</span>
    </button>
    <button
      type="button"
      onclick={() => {
        pinAssetToProjectStyle(contextMenu!.asset);
        contextMenu = null;
      }}
      class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-medium text-fg hover:bg-ink-2 hover:text-accent"
    >
      <Pin size={13} />
      <span>{i18n.t('asset_pin_style_ref')}</span>
    </button>
    <button
      type="button"
      onclick={() => {
        useAssetAsSketch(contextMenu!.asset);
        contextMenu = null;
      }}
      class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-medium text-fg hover:bg-ink-2 hover:text-accent"
    >
      <ImagePlus size={13} />
      <span>{i18n.t('asset_use_as_sketch')}</span>
    </button>
    {#if contextMenu.asset.prompt}
      <button
        type="button"
        onclick={() => {
          jobsStore.promptDraft = contextMenu!.asset.prompt;
          appStore.toast('ok', i18n.t('toast_copied'));
          contextMenu = null;
        }}
        class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-medium text-fg hover:bg-ink-2"
      >
        <Copy size={13} />
        <span>{i18n.t('asset_copy_prompt')}</span>
      </button>
    {/if}
    <button
      type="button"
      onclick={() => {
        api.revealInExplorer(contextMenu!.asset.filePath);
        contextMenu = null;
      }}
      class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-medium text-fg hover:bg-ink-2"
    >
      <FolderOpen size={13} />
      <span>{i18n.t('asset_open_folder')}</span>
    </button>
    <div class="my-0.5 h-px bg-line"></div>
    <button
      type="button"
      onclick={() => {
        removeAsset(contextMenu!.asset);
        contextMenu = null;
      }}
      class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left font-medium text-err hover:bg-err hover:text-white"
    >
      <Trash2 size={13} />
      <span>{i18n.t('asset_delete')}</span>
    </button>
  </div>
{/if}
