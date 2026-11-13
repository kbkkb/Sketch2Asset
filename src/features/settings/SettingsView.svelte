<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import {
    Plus,
    Trash2,
    KeyRound,
    Check,
    RefreshCw,
    Download,
    Upload,
    ShieldCheck,
    Server,
    Eye,
    EyeOff,
    Copy,
    Search,
    ArrowRight,
    ArrowLeft,
    Sparkles,
    Star,
    FolderOpen,
    Loader2,
    Cpu,
    HardDrive,
    SlidersHorizontal,
    Sun,
    Moon,
    Globe,
    Info,
    ExternalLink
  } from '@lucide/svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n, SUPPORTED_LOCALES, type LocaleCode } from '$lib/i18n/index.svelte';
  import { api } from '$lib/api';
  import type {
    MattingModelInfo,
    AppUpdateInfo,
    AgentHubStatus,
    ComfyWorkflowConfig,
    ComfyWorkflowAnalysis
  } from '$lib/api/types';

  let keyDraft = $state('');
  let showKey = $state(false);
  let fetching = $state(false);
  let poolSearch = $state('');
  let onlyImageFilter = $state(false);
  let customModelInput = $state('');
  let importInputEl = $state<HTMLInputElement | null>(null);
  let comfyWorkflowInputEl = $state<HTMLInputElement | null>(null);
  let comfyAnalysis = $state<ComfyWorkflowAnalysis | null>(null);
  let analyzingComfy = $state(false);

  // Local AI Matting Models in Settings
  let mattingModels = $state<MattingModelInfo[]>([]);
  let loadingModelId = $state<string | null>(null);

  // AI Agent Hub (MCP & Skill) state
  let agentHub = $state<AgentHubStatus | null>(null);
  let busyAgentId = $state<string | null>(null);
  let activeSnippetTab = $state<'json' | 'toml' | 'cli' | 'skill'>('json');

  // Update check state
  let appVersion = $state('0.2.1');
  let updateInfo = $state<AppUpdateInfo | null>(null);
  let isCheckingUpdate = $state(false);

  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = '0.2.1';
    }
  });

  async function handleCheckUpdate() {
    isCheckingUpdate = true;
    try {
      const res = await api.checkAppUpdate();
      updateInfo = res;
      if (!res.hasUpdate) {
        appStore.toast(
          'ok',
          i18n.locale === 'en'
            ? `Up to date (v${res.currentVersion})`
            : `当前已是最新版本 (v${res.currentVersion})`
        );
      }
    } catch (e: any) {
      appStore.toast('err', String(e?.message || e));
    } finally {
      isCheckingUpdate = false;
    }
  }

  const IMG_REGEX =
    /image|img|dall|flux|seedream|seededit|banana|imagen|stable|sdxl|sd3|midjourney|\bmj\b|kolors|cogview|wanx|hunyuan-image|ideogram|recraft|playground|pixart|photon|irag/i;

  function isImageModel(m: string): boolean {
    return IMG_REGEX.test(m);
  }

  async function loadProfileKey() {
    const prof = appStore.activeProfile;
    if (!prof) return;
    try {
      keyDraft = await api.getApiKey(prof.id);
    } catch {
      keyDraft = '';
    }
    if (prof.apiMode === 'comfyui' && prof.comfyConfig?.rawWorkflowJson) {
      try {
        comfyAnalysis = await api.analyzeComfyuiWorkflow(
          prof.apiBase || 'http://127.0.0.1:8188',
          prof.comfyConfig.rawWorkflowJson
        );
      } catch {
        comfyAnalysis = null;
      }
    } else {
      comfyAnalysis = null;
    }
  }

  async function loadMattingModels() {
    try {
      mattingModels = await api.listMattingModels();
    } catch {
      mattingModels = [];
    }
  }

  async function loadAgentHub() {
    try {
      agentHub = await api.getAgentHubStatus();
    } catch {
      agentHub = null;
    }
  }

  async function handleInstallAgent(agentId: string) {
    if (busyAgentId) return;
    busyAgentId = agentId;
    try {
      agentHub = await api.installAgentIntegration(agentId);
      appStore.toast('ok', i18n.t('agent_toast_installed'));
    } catch (e: any) {
      appStore.toast('err', String(e?.message || e));
    } finally {
      busyAgentId = null;
    }
  }

  async function handleUninstallAgent(agentId: string) {
    if (busyAgentId) return;
    busyAgentId = agentId;
    try {
      agentHub = await api.uninstallAgentIntegration(agentId);
      appStore.toast('info', i18n.t('agent_toast_uninstalled'));
    } catch (e: any) {
      appStore.toast('err', String(e?.message || e));
    } finally {
      busyAgentId = null;
    }
  }

  async function handleExportAgentBundle() {
    try {
      const outDir = await api.exportAgentSkillBundle(true);
      appStore.toast('ok', `${i18n.t('agent_toast_exported')} ${outDir}`);
    } catch (e: any) {
      appStore.toast('err', String(e?.message || e));
    }
  }

  async function copySnippet(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      appStore.toast('ok', i18n.t('toast_copied'));
    } catch {
      appStore.toast('err', 'Copy failed');
    }
  }

  onMount(() => {
    loadProfileKey();
    loadMattingModels();
    loadAgentHub();
  });

  async function handleDownloadMatting(m: MattingModelInfo) {
    if (loadingModelId) return;
    loadingModelId = m.id;
    try {
      const displayName = i18n.mattingModelName(m.id, m.name);
      appStore.toast(
        'info',
        i18n.locale === 'en' ? `Downloading ${displayName}...` : `正在下载 ${displayName}...`
      );
      mattingModels = await api.downloadMattingModel(m.id);
      appStore.toast('ok', i18n.t('toast_model_installed'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Download failed' : '下载失败'));
    } finally {
      loadingModelId = null;
    }
  }

  async function handleDeleteMatting(m: MattingModelInfo) {
    if (loadingModelId) return;
    loadingModelId = m.id;
    try {
      mattingModels = await api.deleteMattingModel(m.id);
      appStore.toast('ok', i18n.t('toast_model_deleted'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? (i18n.locale === 'en' ? 'Uninstall failed' : '卸载失败'));
    } finally {
      loadingModelId = null;
    }
  }

  async function handleOpenModelsFolder() {
    try {
      await api.openModelsFolder();
    } catch (e: any) {
      appStore.toast(
        'err',
        e?.message ?? (i18n.locale === 'en' ? 'Failed to open directory' : '无法打开目录')
      );
    }
  }

  async function handleSelectProfile(id: string) {
    showKey = false;
    poolSearch = '';
    await appStore.switchProfile(id);
    await loadProfileKey();
  }

  async function handleAddProfile() {
    if (!appStore.settings) return;
    const newId = 'prof_' + Math.random().toString(36).slice(2, 9);
    const nextProfiles = [
      ...appStore.settings.profiles,
      {
        id: newId,
        name: `Provider ${appStore.settings.profiles.length + 1}`,
        apiBase: 'https://api.openai.com/v1',
        apiMode: 'images' as const,
        apiModel: '',
        models: [],
        fetchedModels: [],
        hasKey: false
      }
    ];
    await appStore.saveSettings({
      profiles: nextProfiles,
      activeProfileId: newId
    });
    await loadProfileKey();
  }

  async function handleDeleteProfile(id: string) {
    if (!appStore.settings || appStore.settings.profiles.length <= 1) return;
    const nextProfiles = appStore.settings.profiles.filter((p) => p.id !== id);
    const nextActive = nextProfiles[0]!.id;
    await api.setApiKey(id, '');
    await appStore.saveSettings({
      profiles: nextProfiles,
      activeProfileId: nextActive
    });
    await loadProfileKey();
  }

  function defaultComfyConfig(): ComfyWorkflowConfig {
    return {
      mode: 'builtin',
      workflowName: '',
      rawWorkflowJson: '',
      positiveNodeId: '',
      positiveField: '',
      negativeNodeId: '',
      negativeField: '',
      imageNodeId: '',
      secondaryImageNodeId: '',
      latentNodeId: '',
      seedNodeId: '',
      denoiseStrength: 0.72
    };
  }

  async function updateActiveProfileField(
    field: 'name' | 'apiBase' | 'apiMode' | 'apiModel',
    val: string
  ) {
    if (!appStore.settings || !appStore.activeProfile) return;
    const profiles = appStore.settings.profiles.map((p) => {
      if (p.id !== appStore.activeProfile!.id) return p;
      const next = { ...p, [field]: val } as typeof p;
      if (field === 'apiMode' && val === 'comfyui') {
        if (!next.apiBase || next.apiBase === 'https://api.openai.com/v1') {
          next.apiBase = 'http://127.0.0.1:8188';
        }
        if (!next.comfyConfig) {
          next.comfyConfig = defaultComfyConfig();
        }
        next.hasKey = true;
      }
      return next;
    });
    await appStore.saveSettings({ profiles });
    appStore.toast('ok', i18n.t('toast_profile_saved'));
  }

  async function updateComfyConfig(patch: Partial<ComfyWorkflowConfig>) {
    if (!appStore.settings || !appStore.activeProfile) return;
    const curProf = appStore.activeProfile;
    const baseCfg = curProf.comfyConfig ?? defaultComfyConfig();
    const nextCfg: ComfyWorkflowConfig = { ...baseCfg, ...patch };
    const profiles = appStore.settings.profiles.map((p) =>
      p.id === curProf.id ? { ...p, comfyConfig: nextCfg } : p
    );
    await appStore.saveSettings({ profiles });
  }

  async function handleImportComfyWorkflow(file: File) {
    if (!appStore.activeProfile) return;
    analyzingComfy = true;
    try {
      const isPng = file.name.toLowerCase().endsWith('.png') || file.type === 'image/png';
      const rawContent = isPng
        ? await new Promise<string>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(String(reader.result || ''));
            reader.onerror = () => reject(new Error('Failed to read PNG file'));
            reader.readAsDataURL(file);
          })
        : await file.text();

      const baseUrl = appStore.activeProfile.apiBase || 'http://127.0.0.1:8188';
      const analysis = await api.analyzeComfyuiWorkflow(baseUrl, rawContent);
      comfyAnalysis = analysis;

      const curProf = appStore.activeProfile;
      const baseCfg = curProf.comfyConfig ?? defaultComfyConfig();
      const nextCfg: ComfyWorkflowConfig = {
        ...baseCfg,
        mode: 'custom',
        workflowName: file.name,
        rawWorkflowJson: analysis.normalizedApiJson,
        positiveNodeId: analysis.suggestedPositiveNodeId,
        positiveField: analysis.suggestedPositiveField,
        negativeNodeId: analysis.suggestedNegativeNodeId,
        negativeField: analysis.suggestedNegativeField,
        imageNodeId: analysis.suggestedImageNodeId,
        secondaryImageNodeId: analysis.suggestedSecondaryImageNodeId,
        latentNodeId: analysis.suggestedLatentNodeId,
        seedNodeId: analysis.suggestedSeedNodeId
      };

      const wfModelTag = `workflow:${file.name.replace(/\.(json|png)$/i, '')}`;
      const nextModels = curProf.models.includes(wfModelTag)
        ? curProf.models
        : [wfModelTag, ...curProf.models];
      const nextApiModel = curProf.apiModel || wfModelTag;

      const profiles = appStore.settings!.profiles.map((p) =>
        p.id === curProf.id
          ? {
              ...p,
              comfyConfig: nextCfg,
              models: nextModels,
              apiModel: nextApiModel,
              hasKey: true
            }
          : p
      );
      await appStore.saveSettings({ profiles });
      appStore.toast('ok', `${i18n.t('comfy_toast_wf_loaded')} (${analysis.nodeCount} nodes)`);
    } catch (e: any) {
      appStore.toast('err', e?.details ? `${e.message} — ${e.details}` : String(e?.message || e));
    } finally {
      analyzingComfy = false;
    }
  }

  async function handleReanalyzeComfyWorkflow() {
    const prof = appStore.activeProfile;
    if (!prof?.comfyConfig?.rawWorkflowJson) return;
    analyzingComfy = true;
    try {
      const analysis = await api.analyzeComfyuiWorkflow(
        prof.apiBase || 'http://127.0.0.1:8188',
        prof.comfyConfig.rawWorkflowJson
      );
      comfyAnalysis = analysis;
      await updateComfyConfig({
        rawWorkflowJson: analysis.normalizedApiJson,
        positiveNodeId: prof.comfyConfig.positiveNodeId || analysis.suggestedPositiveNodeId,
        positiveField: prof.comfyConfig.positiveField || analysis.suggestedPositiveField,
        negativeNodeId: prof.comfyConfig.negativeNodeId || analysis.suggestedNegativeNodeId,
        negativeField: prof.comfyConfig.negativeField || analysis.suggestedNegativeField,
        imageNodeId: prof.comfyConfig.imageNodeId || analysis.suggestedImageNodeId,
        secondaryImageNodeId:
          prof.comfyConfig.secondaryImageNodeId || analysis.suggestedSecondaryImageNodeId,
        latentNodeId: prof.comfyConfig.latentNodeId || analysis.suggestedLatentNodeId,
        seedNodeId: prof.comfyConfig.seedNodeId || analysis.suggestedSeedNodeId
      });
      appStore.toast('ok', `${i18n.t('comfy_toast_wf_loaded')} (${analysis.nodeCount} nodes)`);
    } catch (e: any) {
      appStore.toast('err', String(e?.message || e));
    } finally {
      analyzingComfy = false;
    }
  }

  async function handleSaveKey() {
    const prof = appStore.activeProfile;
    if (!prof) return;
    try {
      const cleanKey = keyDraft.trim();
      const updated = await api.setApiKey(prof.id, cleanKey);
      appStore.applySettings(updated);
      keyDraft = cleanKey; // Keep key in input so user can still inspect or copy it
      appStore.toast('ok', i18n.t('toast_key_saved'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to save API Key');
    }
  }

  async function handleCopyKey() {
    if (!keyDraft.trim()) return;
    try {
      await navigator.clipboard.writeText(keyDraft.trim());
      appStore.toast('ok', i18n.t('toast_key_copied'));
    } catch {
      appStore.toast('err', 'Copy failed');
    }
  }

  async function handleFetchModels() {
    const prof = appStore.activeProfile;
    if (!prof) return;
    fetching = true;
    try {
      const list = await api.fetchProviderModels(
        prof.id,
        prof.apiBase,
        keyDraft.trim() || undefined
      );
      const updated = await api.getSettings();
      appStore.applySettings(updated);
      await loadProfileKey();
      appStore.toast('ok', `${i18n.t('toast_models_fetched')} (${list.length})`);
    } catch (e: any) {
      appStore.toast('err', e?.details ? `${e.message} — ${e.details}` : e?.message);
    } finally {
      fetching = false;
    }
  }

  // Dual-column Model Shuttle Computed & Actions
  let enabledModels = $derived(appStore.activeProfile?.models ?? []);

  let availablePoolModels = $derived.by(() => {
    const prof = appStore.activeProfile;
    if (!prof) return [];
    const enabledSet = new Set(prof.models ?? []);
    const rawPool = prof.fetchedModels?.length ? prof.fetchedModels : [];
    const q = poolSearch.trim().toLowerCase();

    return rawPool
      .filter((m) => !enabledSet.has(m))
      .filter((m) => (!onlyImageFilter ? true : isImageModel(m)))
      .filter((m) => (!q ? true : m.toLowerCase().includes(q)))
      .sort((a, b) => {
        const aImg = isImageModel(a) ? 0 : 1;
        const bImg = isImageModel(b) ? 0 : 1;
        if (aImg !== bImg) return aImg - bImg;
        return a.localeCompare(b);
      });
  });

  async function enableModel(modelId: string) {
    const clean = modelId.trim();
    if (!clean || !appStore.settings || !appStore.activeProfile) return;
    const curProf = appStore.activeProfile;
    const nextModels = curProf.models.includes(clean)
      ? curProf.models
      : [...curProf.models, clean];
    const nextDefault = curProf.apiModel || clean;
    const profiles = appStore.settings.profiles.map((p) =>
      p.id === curProf.id ? { ...p, models: nextModels, apiModel: nextDefault } : p
    );
    await appStore.saveSettings({ profiles });
  }

  async function enableAllFiltered() {
    if (!appStore.settings || !appStore.activeProfile || availablePoolModels.length === 0) return;
    const curProf = appStore.activeProfile;
    const merged = Array.from(new Set([...curProf.models, ...availablePoolModels]));
    const nextDefault = curProf.apiModel || merged[0] || '';
    const profiles = appStore.settings.profiles.map((p) =>
      p.id === curProf.id ? { ...p, models: merged, apiModel: nextDefault } : p
    );
    await appStore.saveSettings({ profiles });
    appStore.toast('ok', i18n.t('toast_profile_saved'));
  }

  async function disableModel(modelId: string) {
    if (!appStore.settings || !appStore.activeProfile) return;
    const curProf = appStore.activeProfile;
    const nextModels = curProf.models.filter((m) => m !== modelId);
    const nextDefault =
      curProf.apiModel === modelId ? (nextModels[0] ?? '') : curProf.apiModel;
    const profiles = appStore.settings.profiles.map((p) =>
      p.id === curProf.id
        ? { ...p, models: nextModels, apiModel: nextDefault }
        : p
    );
    await appStore.saveSettings({ profiles });
  }

  async function handleAddCustomModel() {
    const clean = customModelInput.trim();
    if (!clean) return;
    await enableModel(clean);
    customModelInput = '';
  }

  async function handleExportBundle() {
    try {
      const json = await api.exportConfigBundle();
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `sketch2asset_config_${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to export config');
    }
  }

  async function handleImportBundle(file: File) {
    try {
      const text = await file.text();
      const updated = await api.importConfigBundle(text);
      appStore.applySettings(updated);
      await loadProfileKey();
      appStore.toast('ok', i18n.t('toast_config_imported'));
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to import config');
    }
  }
</script>

<div class="stage-backdrop flex flex-1 justify-center overflow-y-auto p-6">
  <div class="flex w-full max-w-6xl flex-col gap-5">
    <!-- Top Header -->
    <div>
      <h2 class="font-display text-2xl font-extrabold tracking-tight text-fg">
        {i18n.t('settings_title')}
      </h2>
      <p class="mt-1 flex items-center gap-1.5 text-xs text-fg-muted">
        <ShieldCheck size={14} class="text-accent" />
        <span>{i18n.t('settings_subtitle')}</span>
      </p>
    </div>

    {#if appStore.settings && appStore.activeProfile}
      <div class="flex flex-col gap-5 lg:flex-row lg:items-start">
        <!-- LEFT NAVIGATION SIDEBAR (Settings Pagination Tabs) -->
        <aside
          class="flex w-full shrink-0 flex-col gap-2 rounded-2xl border border-line bg-ink-1 p-3 lg:w-64"
        >
          <!-- Tab 1: API Providers & Model Pool -->
          <button
            type="button"
            onclick={() => (appStore.settingsTab = 'providers')}
            class="flex items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-all {appStore.settingsTab ===
            'providers'
              ? 'border-accent bg-accent/15 text-fg shadow-sm'
              : 'border-transparent bg-transparent text-fg-muted hover:border-line hover:bg-ink-2/60 hover:text-fg'}"
          >
            <div
              class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg {appStore.settingsTab ===
              'providers'
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted'}"
            >
              <Server size={15} strokeWidth={2} />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center justify-between gap-1.5">
                <span class="truncate text-xs font-bold whitespace-nowrap">
                  {i18n.t('settings_tab_providers')}
                </span>
                <span
                  class="rounded bg-ink-0/80 px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent"
                >
                  {appStore.settings.profiles.length}
                </span>
              </div>
              <p class="mt-0.5 line-clamp-2 text-[11px] leading-snug text-fg-muted">
                {i18n.t('settings_tab_providers_sub')}
              </p>
            </div>
          </button>

          <!-- Tab 2: Local AI Matting Models -->
          <button
            type="button"
            onclick={() => (appStore.settingsTab = 'matting')}
            class="flex items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-all {appStore.settingsTab ===
            'matting'
              ? 'border-accent bg-accent/15 text-fg shadow-sm'
              : 'border-transparent bg-transparent text-fg-muted hover:border-line hover:bg-ink-2/60 hover:text-fg'}"
          >
            <div
              class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg {appStore.settingsTab ===
              'matting'
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted'}"
            >
              <Cpu size={15} strokeWidth={2} />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center justify-between gap-1.5">
                <span class="truncate text-xs font-bold whitespace-nowrap">
                  {i18n.t('settings_tab_matting')}
                </span>
                <span
                  class="rounded bg-ink-0/80 px-1.5 py-0.5 font-mono text-[10px] font-bold text-ok"
                >
                  {mattingModels.filter((m) => m.installed).length}/{mattingModels.length}
                </span>
              </div>
              <p class="mt-0.5 line-clamp-2 text-[11px] leading-snug text-fg-muted">
                {i18n.t('settings_tab_matting_sub')}
              </p>
            </div>
          </button>

          <!-- Tab 3: AI Agent Hub (MCP & Skill) -->
          <button
            type="button"
            onclick={() => {
              appStore.settingsTab = 'agents';
              loadAgentHub();
            }}
            class="flex items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-all {appStore.settingsTab ===
            'agents'
              ? 'border-accent bg-accent/15 text-fg shadow-sm'
              : 'border-transparent bg-transparent text-fg-muted hover:border-line hover:bg-ink-2/60 hover:text-fg'}"
          >
            <div
              class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg {appStore.settingsTab ===
              'agents'
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted'}"
            >
              <Sparkles size={15} strokeWidth={2} />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center justify-between gap-1.5">
                <span class="truncate text-xs font-bold whitespace-nowrap">
                  {i18n.t('settings_tab_agents')}
                </span>
                {#if agentHub}
                  <span
                    class="rounded bg-ink-0/80 px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent"
                  >
                    {agentHub.agents.filter((a) => a.mcpInstalled && a.skillInstalled).length}/{agentHub.agents.length}
                  </span>
                {/if}
              </div>
              <p class="mt-0.5 line-clamp-2 text-[11px] leading-snug text-fg-muted">
                {i18n.t('settings_tab_agents_sub')}
              </p>
            </div>
          </button>

          <!-- Tab 4: Appearance & Backup -->
          <button
            type="button"
            onclick={() => (appStore.settingsTab = 'system')}
            class="flex items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition-all {appStore.settingsTab ===
            'system'
              ? 'border-accent bg-accent/15 text-fg shadow-sm'
              : 'border-transparent bg-transparent text-fg-muted hover:border-line hover:bg-ink-2/60 hover:text-fg'}"
          >
            <div
              class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg {appStore.settingsTab ===
              'system'
                ? 'bg-accent text-accent-ink'
                : 'bg-ink-2 text-fg-muted'}"
            >
              <SlidersHorizontal size={15} strokeWidth={2} />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center justify-between gap-1.5">
                <span class="truncate text-xs font-bold whitespace-nowrap">
                  {i18n.t('settings_tab_system')}
                </span>
              </div>
              <p class="mt-0.5 line-clamp-2 text-[11px] leading-snug text-fg-muted">
                {i18n.t('settings_tab_system_sub')}
              </p>
            </div>
          </button>
        </aside>

        <!-- RIGHT PAGINATED CONTENT PANEL -->
        <div class="min-w-0 flex-1">
          {#if appStore.settingsTab === 'providers'}
            <div class="grid grid-cols-1 gap-5 lg:grid-cols-12">
              <!-- Left 4 Columns: Profile Selector List -->
              <div
                class="flex flex-col gap-3 rounded-2xl border border-line bg-ink-1 p-4 lg:col-span-4"
              >
                <div class="flex items-center justify-between">
                  <span class="font-mono text-xs font-bold tracking-wider text-fg-muted uppercase">
                    {i18n.t('profile_list_title')}
                  </span>
                  <button
                    type="button"
                    onclick={handleAddProfile}
                    class="flex shrink-0 items-center gap-1 rounded-lg bg-accent px-2.5 py-1 text-xs font-bold whitespace-nowrap text-accent-ink transition-colors hover:bg-accent-hover"
                  >
                    <Plus size={13} strokeWidth={2.2} />
                    <span>{i18n.t('profile_add_new')}</span>
                  </button>
                </div>

                <div class="flex flex-col gap-1.5">
                  {#each appStore.settings.profiles as prof (prof.id)}
                    <button
                      type="button"
                      onclick={() => handleSelectProfile(prof.id)}
                      class="flex items-center justify-between rounded-xl border px-3 py-2.5 text-left transition-all {prof.id ===
                      appStore.activeProfile.id
                        ? 'border-accent bg-accent/15 text-fg shadow-sm'
                        : 'border-line bg-ink-2/60 text-fg-muted hover:border-line-strong hover:text-fg'}"
                    >
                      <div class="min-w-0 flex-1">
                        <div class="truncate text-xs font-bold">{prof.name}</div>
                        <div class="mt-0.5 truncate font-mono text-[10px] text-fg-subtle">
                          {prof.apiBase || 'No URL'}
                        </div>
                      </div>
                      <span
                        class="ml-2 h-2 w-2 shrink-0 rounded-full {prof.hasKey
                          ? 'bg-ok'
                          : 'bg-err'}"
                      ></span>
                    </button>
                  {/each}
                </div>
              </div>

              <!-- Right 8 Columns: Active Profile Editor + Dual-Pane Model Shuttle -->
              <div
                class="flex flex-col gap-4 rounded-2xl border border-line bg-ink-1 p-5 lg:col-span-8"
              >
                <div class="flex items-center justify-between border-b border-line pb-3">
                  <div class="flex items-center gap-2">
                    <Server size={16} class="text-accent" />
                    <span class="font-display text-sm font-bold"
                      >{appStore.activeProfile.name}</span
                    >
                  </div>
                  {#if appStore.settings.profiles.length > 1}
                    <button
                      type="button"
                      onclick={() => handleDeleteProfile(appStore.activeProfile!.id)}
                      class="flex shrink-0 items-center gap-1 rounded-lg border border-err/40 bg-err/10 px-2.5 py-1 text-xs font-medium whitespace-nowrap text-err hover:bg-err hover:text-white"
                    >
                      <Trash2 size={13} />
                      <span>{i18n.t('profile_delete')}</span>
                    </button>
                  {/if}
                </div>

                <!-- Top 2-Column Grid for Name & Protocol Mode -->
                <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <!-- Profile Name -->
                  <div class="flex flex-col gap-1.5">
                    <label for="profName" class="text-xs font-semibold text-fg-muted">
                      {i18n.t('profile_name_label')}
                    </label>
                    <input
                      id="profName"
                      type="text"
                      value={appStore.activeProfile.name}
                      onchange={(e) =>
                        updateActiveProfileField(
                          'name',
                          (e.currentTarget as HTMLInputElement).value
                        )}
                      class="rounded-lg border border-line bg-ink-0 px-3.5 py-2 text-xs text-fg outline-none focus:border-accent"
                    />
                  </div>

                  <!-- Endpoint Protocol Mode -->
                  <div class="flex flex-col gap-1.5">
                    <label for="profMode" class="text-xs font-semibold text-fg-muted">
                      {i18n.t('profile_mode_label')}
                    </label>
                    <select
                      id="profMode"
                      value={appStore.activeProfile.apiMode}
                      onchange={(e) =>
                        updateActiveProfileField(
                          'apiMode',
                          (e.currentTarget as HTMLSelectElement).value
                        )}
                      class="rounded-lg border border-line bg-ink-0 px-3.5 py-2 text-xs text-fg outline-none focus:border-accent"
                    >
                      <option value="images">{i18n.t('profile_mode_images')}</option>
                      <option value="chat">{i18n.t('profile_mode_chat')}</option>
                      <option value="comfyui">{i18n.t('profile_mode_comfyui')}</option>
                    </select>
                  </div>
                </div>

                <!-- API Base URL -->
                <div class="flex flex-col gap-1.5">
                  <label for="profBase" class="text-xs font-semibold text-fg-muted">
                    {i18n.t('profile_base_label')}
                  </label>
                  <input
                    id="profBase"
                    type="text"
                    value={appStore.activeProfile.apiBase}
                    onchange={(e) =>
                      updateActiveProfileField(
                        'apiBase',
                        (e.currentTarget as HTMLInputElement).value
                      )}
                    placeholder={appStore.activeProfile.apiMode === 'comfyui'
                      ? 'http://127.0.0.1:8188'
                      : 'https://api.openai.com/v1'}
                    spellcheck="false"
                    class="rounded-lg border border-line bg-ink-0 px-3.5 py-2 font-mono text-xs text-fg outline-none focus:border-accent"
                  />
                </div>

                {#if appStore.activeProfile.apiMode === 'comfyui'}
                  {@const ccfg = appStore.activeProfile.comfyConfig ?? defaultComfyConfig()}
                  <div class="flex flex-col gap-3.5 rounded-xl border border-accent/40 bg-accent/5 p-4">
                    <div class="flex flex-wrap items-start justify-between gap-2">
                      <div>
                        <div class="flex items-center gap-2 text-xs font-bold text-fg">
                          <Cpu size={14} class="text-accent" />
                          <span>{i18n.t('comfy_panel_title')}</span>
                        </div>
                        <p class="mt-0.5 text-[11px] text-fg-muted">
                          {i18n.t('comfy_panel_sub')}
                        </p>
                      </div>
                    </div>

                    <!-- Mode Switcher: Built-in vs Custom Workflow -->
                    <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
                      <button
                        type="button"
                        onclick={() => updateComfyConfig({ mode: 'builtin' })}
                        class="flex flex-col gap-1 rounded-lg border px-3 py-2.5 text-left transition-all {ccfg.mode !==
                        'custom'
                          ? 'border-accent bg-accent/15 text-fg shadow-xs'
                          : 'border-line bg-ink-0/70 text-fg-muted hover:border-line-strong hover:text-fg'}"
                      >
                        <span class="text-xs font-bold">{i18n.t('comfy_mode_builtin')}</span>
                      </button>

                      <button
                        type="button"
                        onclick={() => updateComfyConfig({ mode: 'custom' })}
                        class="flex flex-col gap-1 rounded-lg border px-3 py-2.5 text-left transition-all {ccfg.mode ===
                        'custom'
                          ? 'border-accent bg-accent/15 text-fg shadow-xs'
                          : 'border-line bg-ink-0/70 text-fg-muted hover:border-line-strong hover:text-fg'}"
                      >
                        <span class="text-xs font-bold">{i18n.t('comfy_mode_custom')}</span>
                      </button>
                    </div>

                    <!-- Denoise Slider for Sketch / Reference Img2Img -->
                    <div class="flex items-center justify-between gap-4 rounded-lg border border-line bg-ink-0/70 px-3 py-2">
                      <span class="text-xs font-medium text-fg-muted">
                        {i18n.t('comfy_denoise_label')}
                      </span>
                      <div class="flex items-center gap-2.5">
                        <input
                          type="range"
                          min="0.15"
                          max="0.98"
                          step="0.01"
                          value={ccfg.denoiseStrength ?? 0.72}
                          onchange={(e) =>
                            updateComfyConfig({
                              denoiseStrength: parseFloat(
                                (e.currentTarget as HTMLInputElement).value
                              )
                            })}
                          class="h-1.5 w-32 accent-accent"
                        />
                        <span class="w-10 text-right font-mono text-xs font-bold text-accent">
                          {(ccfg.denoiseStrength ?? 0.72).toFixed(2)}
                        </span>
                      </div>
                    </div>

                    {#if ccfg.mode !== 'custom'}
                      <div class="rounded-lg border border-line bg-ink-0/60 px-3 py-2 text-[11px] leading-relaxed text-fg-muted">
                        {i18n.t('comfy_builtin_hint')}
                      </div>
                    {:else}
                      <!-- Custom Workflow Import & Visual Node Slot Mapper -->
                      <div class="flex flex-col gap-3 rounded-lg border border-line bg-ink-0/80 p-3.5">
                        <div class="flex flex-wrap items-center justify-between gap-2">
                          <div class="min-w-0 flex-1">
                            {#if ccfg.workflowName}
                              <div class="flex items-center gap-2">
                                <span class="truncate font-mono text-xs font-bold text-ok">
                                  {i18n.t('comfy_workflow_loaded')}{ccfg.workflowName}
                                </span>
                                {#if comfyAnalysis}
                                  <span class="rounded bg-ink-2 px-1.5 py-0.5 font-mono text-[10px] text-fg-muted">
                                    {comfyAnalysis.nodeCount} nodes · {comfyAnalysis.format}
                                  </span>
                                {/if}
                              </div>
                            {:else}
                              <div class="text-[11px] text-fg-muted">
                                {i18n.t('comfy_workflow_empty')}
                              </div>
                            {/if}
                          </div>

                          <div class="flex items-center gap-2">
                            <input
                              bind:this={comfyWorkflowInputEl}
                              type="file"
                              accept=".json,.png,application/json,image/png"
                              class="hidden"
                              onchange={(e) => {
                                const f = (e.currentTarget as HTMLInputElement).files?.[0];
                                if (f) handleImportComfyWorkflow(f);
                                (e.currentTarget as HTMLInputElement).value = '';
                              }}
                            />
                            <button
                              type="button"
                              disabled={analyzingComfy}
                              onclick={() => comfyWorkflowInputEl?.click()}
                              class="flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-bold text-accent-ink transition-colors hover:bg-accent-hover disabled:opacity-45"
                            >
                              <Upload size={13} />
                              <span>{i18n.t('comfy_import_btn')}</span>
                            </button>

                            {#if ccfg.rawWorkflowJson}
                              <button
                                type="button"
                                disabled={analyzingComfy}
                                onclick={handleReanalyzeComfyWorkflow}
                                class="flex items-center gap-1 rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 text-xs font-medium text-fg-muted transition-colors hover:border-accent hover:text-fg disabled:opacity-45"
                              >
                                <RefreshCw size={12} class={analyzingComfy ? 'animate-spin' : ''} />
                                <span>{i18n.t('comfy_reanalyze_btn')}</span>
                              </button>
                            {/if}
                          </div>
                        </div>

                        {#if comfyAnalysis}
                          <div class="grid grid-cols-1 gap-3 border-t border-line pt-3 sm:grid-cols-2">
                            <!-- 1. Positive Prompt Slot -->
                            <div class="flex flex-col gap-1">
                              <label for="comfyPosNode" class="text-[11px] font-semibold text-fg-muted">
                                {i18n.t('comfy_slot_pos_label')}
                              </label>
                              <select
                                id="comfyPosNode"
                                value={ccfg.positiveNodeId}
                                onchange={(e) => {
                                  const nid = (e.currentTarget as HTMLSelectElement).value;
                                  const found = comfyAnalysis?.textNodes.find((n) => n.nodeId === nid);
                                  updateComfyConfig({
                                    positiveNodeId: nid,
                                    positiveField: found?.field ?? 'text'
                                  });
                                }}
                                class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-[11px] text-fg outline-none focus:border-accent"
                              >
                                <option value="">{i18n.t('comfy_slot_auto')}</option>
                                {#each comfyAnalysis.textNodes as tn (tn.nodeId + ':' + tn.field)}
                                  <option value={tn.nodeId}>
                                    #{tn.nodeId} [{tn.classType}] {tn.title} — {tn.preview || '(empty)'}
                                  </option>
                                {/each}
                              </select>
                            </div>

                            <!-- 2. Negative Prompt Slot -->
                            <div class="flex flex-col gap-1">
                              <label for="comfyNegNode" class="text-[11px] font-semibold text-fg-muted">
                                {i18n.t('comfy_slot_neg_label')}
                              </label>
                              <select
                                id="comfyNegNode"
                                value={ccfg.negativeNodeId}
                                onchange={(e) => {
                                  const nid = (e.currentTarget as HTMLSelectElement).value;
                                  const found = comfyAnalysis?.textNodes.find((n) => n.nodeId === nid);
                                  updateComfyConfig({
                                    negativeNodeId: nid,
                                    negativeField: found?.field ?? 'text'
                                  });
                                }}
                                class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-[11px] text-fg outline-none focus:border-accent"
                              >
                                <option value="">{i18n.t('comfy_slot_auto')}</option>
                                {#each comfyAnalysis.textNodes as tn (tn.nodeId + ':' + tn.field)}
                                  <option value={tn.nodeId}>
                                    #{tn.nodeId} [{tn.classType}] {tn.title} — {tn.preview || '(empty)'}
                                  </option>
                                {/each}
                              </select>
                            </div>

                            <!-- 3. Primary Reference / Sketch Image Slot -->
                            <div class="flex flex-col gap-1">
                              <label for="comfyImgNode" class="text-[11px] font-semibold text-fg-muted">
                                {i18n.t('comfy_slot_img_label')}
                              </label>
                              <select
                                id="comfyImgNode"
                                value={ccfg.imageNodeId}
                                onchange={(e) =>
                                  updateComfyConfig({
                                    imageNodeId: (e.currentTarget as HTMLSelectElement).value
                                  })}
                                class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-[11px] text-fg outline-none focus:border-accent"
                              >
                                <option value="">{i18n.t('comfy_slot_auto')}</option>
                                {#each comfyAnalysis.imageNodes as imn (imn.nodeId)}
                                  <option value={imn.nodeId}>
                                    #{imn.nodeId} [{imn.classType}] {imn.title} ({imn.preview})
                                  </option>
                                {/each}
                              </select>
                            </div>

                            <!-- 4. Secondary Reference / Mask Slot -->
                            <div class="flex flex-col gap-1">
                              <label for="comfySecImgNode" class="text-[11px] font-semibold text-fg-muted">
                                {i18n.t('comfy_slot_sec_img_label')}
                              </label>
                              <select
                                id="comfySecImgNode"
                                value={ccfg.secondaryImageNodeId}
                                onchange={(e) =>
                                  updateComfyConfig({
                                    secondaryImageNodeId: (e.currentTarget as HTMLSelectElement).value
                                  })}
                                class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-[11px] text-fg outline-none focus:border-accent"
                              >
                                <option value="">{i18n.t('comfy_slot_auto')}</option>
                                {#each comfyAnalysis.imageNodes as imn (imn.nodeId)}
                                  <option value={imn.nodeId}>
                                    #{imn.nodeId} [{imn.classType}] {imn.title} ({imn.preview})
                                  </option>
                                {/each}
                              </select>
                            </div>

                            <!-- 5. Latent Size Slot -->
                            <div class="flex flex-col gap-1">
                              <label for="comfyLatentNode" class="text-[11px] font-semibold text-fg-muted">
                                {i18n.t('comfy_slot_latent_label')}
                              </label>
                              <select
                                id="comfyLatentNode"
                                value={ccfg.latentNodeId}
                                onchange={(e) =>
                                  updateComfyConfig({
                                    latentNodeId: (e.currentTarget as HTMLSelectElement).value
                                  })}
                                class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-[11px] text-fg outline-none focus:border-accent"
                              >
                                <option value="">{i18n.t('comfy_slot_auto')}</option>
                                {#each comfyAnalysis.latentNodes as ln (ln.nodeId)}
                                  <option value={ln.nodeId}>
                                    #{ln.nodeId} [{ln.classType}] {ln.title} ({ln.preview})
                                  </option>
                                {/each}
                              </select>
                            </div>

                            <!-- 6. Seed Slot -->
                            <div class="flex flex-col gap-1">
                              <label for="comfySeedNode" class="text-[11px] font-semibold text-fg-muted">
                                {i18n.t('comfy_slot_seed_label')}
                              </label>
                              <select
                                id="comfySeedNode"
                                value={ccfg.seedNodeId}
                                onchange={(e) =>
                                  updateComfyConfig({
                                    seedNodeId: (e.currentTarget as HTMLSelectElement).value
                                  })}
                                class="rounded-lg border border-line bg-ink-1 px-2.5 py-1.5 font-mono text-[11px] text-fg outline-none focus:border-accent"
                              >
                                <option value="">{i18n.t('comfy_slot_auto')}</option>
                                {#each comfyAnalysis.seedNodes as sn (sn.nodeId)}
                                  <option value={sn.nodeId}>
                                    #{sn.nodeId} [{sn.classType}] {sn.title} (seed={sn.preview})
                                  </option>
                                {/each}
                              </select>
                            </div>
                          </div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/if}

                <!-- API Key Vault (Password Input + Eye Toggle + One-Click Copy + Save) -->
                <div class="flex flex-col gap-1.5">
                  <div class="flex items-center justify-between">
                    <label for="profKey" class="text-xs font-semibold text-fg-muted">
                      {i18n.t('profile_key_label')}
                    </label>
                    <span
                      class="flex items-center gap-1 font-mono text-[11px] {appStore.activeProfile
                        .hasKey
                        ? 'text-ok'
                        : 'text-err'}"
                    >
                      <KeyRound size={12} />
                      <span>
                        {appStore.activeProfile.hasKey
                          ? i18n.t('key_configured')
                          : i18n.t('key_missing')}
                      </span>
                    </span>
                  </div>

                  <div class="flex items-center gap-2">
                    <div
                      class="relative flex flex-1 items-center rounded-lg border border-line bg-ink-0 focus-within:border-accent"
                    >
                      <input
                        id="profKey"
                        type={showKey ? 'text' : 'password'}
                        bind:value={keyDraft}
                        placeholder={i18n.t('profile_key_placeholder')}
                        spellcheck="false"
                        autocomplete="off"
                        class="w-full bg-transparent py-2 pr-20 pl-3.5 font-mono text-xs text-fg outline-none"
                      />
                      <div class="absolute right-1.5 flex items-center gap-1">
                        <button
                          type="button"
                          onclick={() => (showKey = !showKey)}
                          title={showKey ? i18n.t('profile_key_hide') : i18n.t('profile_key_show')}
                          class="rounded p-1.5 text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg"
                        >
                          {#if showKey}
                            <EyeOff size={14} />
                          {:else}
                            <Eye size={14} />
                          {/if}
                        </button>
                        <button
                          type="button"
                          onclick={handleCopyKey}
                          disabled={!keyDraft.trim()}
                          title={i18n.t('profile_key_copy')}
                          class="rounded p-1.5 text-fg-muted transition-colors hover:bg-ink-2 hover:text-accent disabled:opacity-35"
                        >
                          <Copy size={14} />
                        </button>
                      </div>
                    </div>

                    <button
                      type="button"
                      onclick={handleSaveKey}
                      disabled={!keyDraft.trim()}
                      class="flex shrink-0 items-center gap-1.5 rounded-lg bg-accent px-4 py-2 text-xs font-bold whitespace-nowrap text-accent-ink transition-colors hover:bg-accent-hover disabled:opacity-40"
                    >
                      <Check size={14} strokeWidth={2.2} />
                      <span>{i18n.t('profile_save_key')}</span>
                    </button>
                  </div>
                </div>

                <!-- Dual-Column Model Shuttle (Left Pool -> Right Enabled) -->
                <div class="mt-1 flex flex-col gap-3 rounded-xl border border-line bg-ink-0/60 p-4">
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <div>
                      <div class="text-xs font-bold text-fg">
                        {i18n.t('profile_default_model_label')}
                      </div>
                      <p class="mt-0.5 text-[11px] text-fg-muted">
                        {i18n.t('shuttle_subtitle')}
                      </p>
                    </div>
                    <button
                      type="button"
                      disabled={fetching}
                      onclick={handleFetchModels}
                      class="flex shrink-0 items-center gap-1.5 rounded-lg border border-accent/50 bg-accent/15 px-3.5 py-1.5 text-xs font-bold whitespace-nowrap text-accent transition-all hover:bg-accent hover:text-accent-ink disabled:opacity-45"
                    >
                      <RefreshCw
                        size={13}
                        strokeWidth={2}
                        class={fetching ? 'animate-spin' : ''}
                      />
                      <span
                        >{fetching ? i18n.t('btn_fetching') : i18n.t('btn_fetch_models')}</span
                      >
                    </button>
                  </div>

                  <!-- Custom Model Quick Add Row -->
                  <div class="flex items-center gap-2">
                    <input
                      type="text"
                      bind:value={customModelInput}
                      onkeydown={(e) => e.key === 'Enter' && handleAddCustomModel()}
                      placeholder={i18n.t('shuttle_custom_ph')}
                      spellcheck="false"
                      class="flex-1 rounded-lg border border-line bg-ink-1 px-3 py-1.5 font-mono text-xs text-fg outline-none focus:border-accent"
                    />
                    <button
                      type="button"
                      onclick={handleAddCustomModel}
                      disabled={!customModelInput.trim()}
                      class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-2 px-3 py-1.5 text-xs font-semibold whitespace-nowrap text-fg transition-colors hover:border-accent hover:text-accent disabled:opacity-40"
                    >
                      <Plus size={13} />
                      <span>{i18n.t('shuttle_custom_add')}</span>
                    </button>
                  </div>

                  <!-- Two-Pane Shuttle Grid -->
                  <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
                    <!-- LEFT PANE: Fetched Remote Pool -->
                    <div
                      class="flex h-72 flex-col overflow-hidden rounded-xl border border-line bg-ink-1"
                    >
                      <div
                        class="flex items-center justify-between border-b border-line bg-ink-2/60 px-3 py-2"
                      >
                        <span class="font-mono text-[11px] font-bold text-fg-muted uppercase">
                          {i18n.t('shuttle_left_title')} ({availablePoolModels.length})
                        </span>
                        <div class="flex items-center gap-1.5">
                          <button
                            type="button"
                            onclick={() => (onlyImageFilter = !onlyImageFilter)}
                            class="rounded border px-1.5 py-0.5 text-[10px] font-semibold whitespace-nowrap transition-colors {onlyImageFilter
                              ? 'border-accent bg-accent/20 text-accent'
                              : 'border-line bg-ink-0 text-fg-muted hover:text-fg'}"
                          >
                            {i18n.t('shuttle_only_image')}
                          </button>
                          {#if availablePoolModels.length > 0}
                            <button
                              type="button"
                              onclick={enableAllFiltered}
                              class="rounded border border-line bg-ink-0 px-1.5 py-0.5 text-[10px] font-semibold whitespace-nowrap text-accent hover:border-accent"
                            >
                              {i18n.t('shuttle_add_filtered')}
                            </button>
                          {/if}
                        </div>
                      </div>

                      <!-- Search Filter in Left Pool -->
                      <div
                        class="flex items-center gap-1.5 border-b border-line bg-ink-0/60 px-2.5 py-1.5"
                      >
                        <Search size={12} class="text-fg-subtle" />
                        <input
                          type="text"
                          bind:value={poolSearch}
                          placeholder={i18n.t('shuttle_search_ph')}
                          spellcheck="false"
                          class="w-full bg-transparent font-mono text-xs text-fg outline-none placeholder:text-fg-subtle"
                        />
                      </div>

                      <div class="flex-1 overflow-y-auto p-1.5">
                        {#if availablePoolModels.length === 0}
                          <div
                            class="flex h-full items-center justify-center p-4 text-center text-xs text-fg-subtle"
                          >
                            {i18n.t('shuttle_empty_left')}
                          </div>
                        {:else}
                          {#each availablePoolModels as m (m)}
                            <button
                              type="button"
                              onclick={() => enableModel(m)}
                              title={m}
                              class="group flex w-full items-center justify-between gap-2 rounded-lg px-2.5 py-1.5 text-left font-mono text-xs text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg"
                            >
                              <span class="truncate" title={m}>{m}</span>
                              <span class="flex shrink-0 items-center gap-1.5">
                                {#if isImageModel(m)}
                                  <span
                                    class="rounded border border-accent/35 bg-accent/15 px-1.5 py-0.5 font-sans text-[9px] font-semibold text-accent"
                                  >
                                    IMAGE
                                  </span>
                                {/if}
                                <ArrowRight
                                  size={13}
                                  class="text-fg-subtle transition-transform group-hover:translate-x-0.5 group-hover:text-accent"
                                />
                              </span>
                            </button>
                          {/each}
                        {/if}
                      </div>
                    </div>

                    <!-- RIGHT PANE: Enabled Models in Asset Forge -->
                    <div
                      class="flex h-72 flex-col overflow-hidden rounded-xl border border-accent/40 bg-ink-1"
                    >
                      <div
                        class="flex items-center justify-between border-b border-line bg-accent/10 px-3 py-2"
                      >
                        <span
                          class="flex items-center gap-1.5 font-mono text-[11px] font-bold text-accent uppercase"
                        >
                          <Sparkles size={12} />
                          <span>{i18n.t('shuttle_right_title')} ({enabledModels.length})</span>
                        </span>
                      </div>

                      <div class="flex-1 overflow-y-auto p-1.5">
                        {#if enabledModels.length === 0}
                          <div
                            class="flex h-full items-center justify-center p-4 text-center text-xs text-fg-subtle"
                          >
                            {i18n.t('shuttle_empty_right')}
                          </div>
                        {:else}
                          {#each enabledModels as m (m)}
                            {@const isDef = appStore.activeProfile.apiModel === m}
                            <div
                              class="group flex items-center justify-between gap-2 rounded-lg px-2.5 py-1.5 font-mono text-xs transition-colors {isDef
                                ? 'bg-accent/15 font-semibold text-accent'
                                : 'text-fg hover:bg-ink-2'}"
                              title={m}
                            >
                              <button
                                type="button"
                                onclick={() => updateActiveProfileField('apiModel', m)}
                                title={isDef
                                  ? `${m} (${i18n.t('shuttle_default_badge')})`
                                  : `${m} - ${i18n.t('shuttle_set_default')}`}
                                class="flex min-w-0 flex-1 items-center gap-2 text-left"
                              >
                                <Star
                                  size={12}
                                  class="shrink-0 {isDef
                                    ? 'fill-accent text-accent'
                                    : 'text-fg-subtle group-hover:text-accent'}"
                                />
                                <span class="truncate" title={m}>{m}</span>
                              </button>

                              <div class="flex shrink-0 items-center gap-1.5">
                                {#if isDef}
                                  <span
                                    class="rounded bg-accent px-1.5 py-0.5 font-sans text-[9px] font-bold text-accent-ink"
                                  >
                                    {i18n.t('shuttle_default_badge')}
                                  </span>
                                {/if}
                                <button
                                  type="button"
                                  onclick={() => disableModel(m)}
                                  class="rounded p-1 text-fg-subtle transition-colors hover:bg-err/20 hover:text-err"
                                >
                                  <ArrowLeft size={13} />
                                </button>
                              </div>
                            </div>
                          {/each}
                        {/if}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          {:else if appStore.settingsTab === 'matting'}
            <!-- Local AI Matting Models Management Card -->
            <div class="rounded-2xl border border-line bg-ink-1 p-6">
              <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
                <div>
                  <div class="flex items-center gap-2">
                    <Cpu size={18} class="text-accent" />
                    <h3 class="font-display text-sm font-bold tracking-wide text-fg">
                      {i18n.t('settings_matting_title')}
                    </h3>
                  </div>
                  <p class="mt-1 max-w-2xl text-xs text-fg-muted">
                    {i18n.t('settings_matting_sub')}
                  </p>
                </div>

                <button
                  type="button"
                  onclick={handleOpenModelsFolder}
                  class="flex shrink-0 items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3 py-1.5 text-xs font-semibold whitespace-nowrap text-fg hover:border-accent hover:text-accent"
                >
                  <FolderOpen size={13} />
                  <span>{i18n.t('settings_open_models_dir')}</span>
                </button>
              </div>

              <div class="grid grid-cols-1 gap-3.5 md:grid-cols-2 lg:grid-cols-3">
                {#each mattingModels as m (m.id)}
                  <div
                    class="flex flex-col justify-between rounded-xl border border-line bg-ink-0/60 p-4 transition-colors hover:border-line-strong"
                  >
                    <div>
                      <div class="flex items-start justify-between gap-2">
                        <div class="font-display text-xs font-bold text-fg">
                          {i18n.mattingModelName(m.id, m.name)}
                        </div>
                        {#if m.installed}
                          <span
                            class="shrink-0 rounded bg-ok/15 px-2 py-0.5 font-mono text-[10px] font-bold whitespace-nowrap text-ok"
                          >
                            {i18n.t('model_installed_badge')}
                          </span>
                        {:else}
                          <span
                            class="shrink-0 rounded bg-ink-2 px-2 py-0.5 font-mono text-[10px] font-medium whitespace-nowrap text-fg-subtle"
                          >
                            {i18n.t('model_not_installed_badge')}
                          </span>
                        {/if}
                      </div>

                      <div
                        class="mt-1.5 flex items-center gap-2 font-mono text-[11px] text-fg-muted"
                      >
                        <HardDrive size={12} class="shrink-0" />
                        <span>{m.sizeMb} MB</span>
                        <span class="text-fg-subtle">·</span>
                        <span class="truncate text-fg-subtle">{m.fileName}</span>
                      </div>

                      <p class="mt-2.5 text-[11px] leading-relaxed text-fg-muted">
                        {i18n.mattingModelDesc(m.id, m.description)}
                      </p>
                    </div>

                    <div
                      class="mt-4 flex items-center justify-end gap-2 border-t border-line/60 pt-3"
                    >
                      {#if m.installed}
                        <button
                          type="button"
                          onclick={() => handleDeleteMatting(m)}
                          disabled={loadingModelId === m.id}
                          class="flex shrink-0 items-center gap-1 rounded-lg border border-err/30 bg-err/10 px-2.5 py-1 text-xs font-semibold whitespace-nowrap text-err transition-colors hover:bg-err hover:text-white disabled:opacity-50"
                        >
                          {#if loadingModelId === m.id}
                            <Loader2 size={12} class="animate-spin" />
                          {:else}
                            <Trash2 size={12} />
                          {/if}
                          <span>{i18n.t('btn_uninstall_model')}</span>
                        </button>
                      {:else}
                        <button
                          type="button"
                          onclick={() => handleDownloadMatting(m)}
                          disabled={loadingModelId === m.id}
                          class="flex shrink-0 items-center gap-1 rounded-lg bg-accent px-3 py-1 text-xs font-bold whitespace-nowrap text-accent-ink transition-transform active:scale-95 disabled:opacity-50"
                        >
                          {#if loadingModelId === m.id}
                            <Loader2 size={12} class="animate-spin" />
                          {:else}
                            <Download size={12} strokeWidth={2.2} />
                          {/if}
                          <span>{i18n.t('btn_install_model')}</span>
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {:else if appStore.settingsTab === 'system'}
            <div class="flex flex-col gap-5">
              <!-- Appearance & Language Card -->
              <div class="flex flex-col gap-4 rounded-2xl border border-line bg-ink-1 p-6">
                <div class="flex items-center gap-2">
                  <SlidersHorizontal size={16} class="text-accent" />
                  <h3 class="font-display text-sm font-bold text-fg">
                    {i18n.t('settings_appearance_title')}
                  </h3>
                </div>

                <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <!-- Theme Switcher -->
                  <div class="flex flex-col gap-2 rounded-xl border border-line bg-ink-0/60 p-4">
                    <span class="text-xs font-semibold text-fg-muted">
                      {i18n.t('settings_theme_label')}
                    </span>
                    <div class="grid grid-cols-2 gap-2">
                      <button
                        type="button"
                        onclick={() => appStore.saveSettings({ theme: 'dark' })}
                        class="flex items-center justify-center gap-2 rounded-lg border px-3 py-2 text-xs font-semibold whitespace-nowrap transition-all {appStore.settings
                          .theme === 'dark'
                          ? 'border-accent bg-accent/15 text-accent'
                          : 'border-line bg-ink-1 text-fg-muted hover:text-fg'}"
                      >
                        <Moon size={14} />
                        <span>{i18n.t('settings_theme_dark')}</span>
                      </button>
                      <button
                        type="button"
                        onclick={() => appStore.saveSettings({ theme: 'light' })}
                        class="flex items-center justify-center gap-2 rounded-lg border px-3 py-2 text-xs font-semibold whitespace-nowrap transition-all {appStore.settings
                          .theme === 'light'
                          ? 'border-accent bg-accent/15 text-accent'
                          : 'border-line bg-ink-1 text-fg-muted hover:text-fg'}"
                      >
                        <Sun size={14} />
                        <span>{i18n.t('settings_theme_light')}</span>
                      </button>
                    </div>
                  </div>

                  <!-- Language Switcher (Extensible Dropdown) -->
                  <div class="flex flex-col gap-2 rounded-xl border border-line bg-ink-0/60 p-4">
                    <label for="langSelect" class="flex items-center gap-1.5 text-xs font-semibold text-fg-muted">
                      <Globe size={13} class="text-accent" />
                      <span>{i18n.t('settings_lang_label')}</span>
                    </label>
                    <select
                      id="langSelect"
                      value={appStore.settings.language}
                      onchange={(e) => {
                        const code = (e.currentTarget as HTMLSelectElement).value as LocaleCode;
                        i18n.setLocale(code);
                        appStore.saveSettings({ language: code });
                      }}
                      class="w-full rounded-lg border border-line bg-ink-1 px-3 py-2 text-xs font-semibold text-fg outline-none transition-colors hover:border-line-strong focus:border-accent"
                    >
                      {#each SUPPORTED_LOCALES as loc (loc.code)}
                        <option value={loc.code}>{loc.label}</option>
                      {/each}
                    </select>
                  </div>
                </div>
              </div>

              <!-- Backup & Migration Card -->
              <div
                class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-line bg-ink-1 p-6"
              >
                <div>
                  <div class="text-xs font-bold text-fg">
                    {i18n.t('bundle_card_title')}
                  </div>
                  <p class="mt-1 text-xs text-fg-muted">
                    {i18n.t('bundle_card_sub')}
                  </p>
                </div>
                <div class="flex items-center gap-2.5">
                  <input
                    bind:this={importInputEl}
                    type="file"
                    accept=".json,application/json"
                    hidden
                    onchange={(e) => {
                      const f = (e.currentTarget as HTMLInputElement).files?.[0];
                      if (f) handleImportBundle(f);
                      (e.currentTarget as HTMLInputElement).value = '';
                    }}
                  />
                  <button
                    type="button"
                    onclick={() => importInputEl?.click()}
                    class="flex shrink-0 items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3.5 py-2 text-xs font-semibold whitespace-nowrap text-fg hover:border-accent"
                  >
                    <Upload size={14} />
                    <span>{i18n.t('bundle_import_btn')}</span>
                  </button>
                  <button
                    type="button"
                    onclick={handleExportBundle}
                    class="flex shrink-0 items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3.5 py-2 text-xs font-semibold whitespace-nowrap text-fg hover:border-accent"
                  >
                    <Download size={14} />
                    <span>{i18n.t('bundle_export_btn')}</span>
                  </button>
                </div>
              </div>

              <!-- About & App Update Card -->
              <div
                class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-line bg-ink-1 p-6"
              >
                <div class="flex flex-col gap-1">
                  <div class="flex items-center gap-2 text-xs font-bold text-fg">
                    <Info size={14} class="text-accent" />
                    <span>{i18n.t('about_app_title')}</span>
                    <span class="rounded-md bg-ink-2 px-2 py-0.5 font-mono text-[11px] font-bold text-accent">
                      v{appVersion}
                    </span>
                    {#if updateInfo}
                      {#if updateInfo.hasUpdate}
                        <span class="rounded-md bg-ok/20 px-2 py-0.5 text-[11px] font-bold text-ok">
                          {i18n.t('about_has_update')}: {updateInfo.latestVersion}
                        </span>
                      {:else}
                        <span class="rounded-md bg-ink-2 px-2 py-0.5 text-[11px] font-semibold text-fg-muted">
                          {i18n.t('about_is_latest')}
                        </span>
                      {/if}
                    {/if}
                  </div>
                  <p class="text-xs text-fg-muted">
                    {i18n.t('about_thanks_linuxdo')}
                  </p>
                </div>

                <div class="flex items-center gap-2.5">
                  {#if updateInfo?.hasUpdate}
                    <a
                      href={updateInfo.releaseUrl}
                      target="_blank"
                      rel="noreferrer"
                      class="flex shrink-0 items-center gap-1.5 rounded-lg bg-accent px-3.5 py-2 text-xs font-bold whitespace-nowrap text-ink-0 transition-opacity hover:opacity-90"
                    >
                      <ExternalLink size={14} />
                      <span>{i18n.t('about_view_release')}</span>
                    </a>
                  {/if}
                  <button
                    type="button"
                    disabled={isCheckingUpdate}
                    onclick={handleCheckUpdate}
                    class="flex shrink-0 items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3.5 py-2 text-xs font-semibold whitespace-nowrap text-fg hover:border-accent disabled:opacity-50"
                  >
                    {#if isCheckingUpdate}
                      <Loader2 size={14} class="animate-spin text-accent" />
                      <span>{i18n.t('about_checking')}</span>
                    {:else}
                      <RefreshCw size={14} />
                      <span>{i18n.t('about_check_update_btn')}</span>
                    {/if}
                  </button>
                </div>
              </div>
            </div>
          {:else if appStore.settingsTab === 'agents'}
            <div class="flex flex-col gap-5">
              <!-- Top Overview Banner -->
              <div
                class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-line bg-ink-1 p-5"
              >
                <div class="max-w-2xl">
                  <div class="flex items-center gap-2">
                    <Sparkles size={16} class="text-accent" />
                    <h3 class="text-sm font-bold text-fg">
                      {i18n.t('agent_hub_title')}
                    </h3>
                  </div>
                  <p class="mt-1 text-xs leading-relaxed text-fg-muted">
                    {i18n.t('agent_hub_sub')}
                  </p>
                </div>

                <div class="flex items-center gap-2">
                  <button
                    type="button"
                    disabled={busyAgentId !== null}
                    onclick={() => handleInstallAgent('all')}
                    class="flex shrink-0 items-center gap-1.5 rounded-xl bg-accent px-4 py-2.5 text-xs font-bold whitespace-nowrap text-accent-ink shadow-sm transition-opacity hover:opacity-90 disabled:opacity-50"
                  >
                    {#if busyAgentId === 'all'}
                      <Loader2 size={14} class="animate-spin" />
                    {:else}
                      <Sparkles size={14} />
                    {/if}
                    <span>{i18n.t('agent_install_all_btn')}</span>
                  </button>
                </div>
              </div>

              <!-- Section 1: One-Click Install for Popular Agents -->
              <div class="flex flex-col gap-3 rounded-2xl border border-line bg-ink-1 p-5">
                <div class="flex items-center justify-between">
                  <span class="text-xs font-bold text-fg">
                    {i18n.t('agent_oneclick_section')}
                  </span>
                  <button
                    type="button"
                    onclick={loadAgentHub}
                    class="flex items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-[11px] font-semibold text-fg-muted hover:border-accent hover:text-fg"
                  >
                    <RefreshCw size={12} />
                    <span>{i18n.t('studio_refresh')}</span>
                  </button>
                </div>

                {#if agentHub}
                  <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
                    {#each agentHub.agents as agent (agent.id)}
                      {@const isReady = agent.mcpInstalled && agent.skillInstalled}
                      {@const isPartial = !isReady && (agent.mcpInstalled || agent.skillInstalled)}
                      <div
                        class="flex flex-col justify-between gap-3 rounded-xl border p-4 transition-all {isReady
                          ? 'border-ok/40 bg-ok/5'
                          : 'border-line bg-ink-0/60'}"
                      >
                        <div class="flex flex-col gap-1.5">
                          <div class="flex flex-wrap items-center justify-between gap-2">
                            <div class="flex items-center gap-2">
                              <span class="text-xs font-bold text-fg">{agent.name}</span>
                              <span
                                class="rounded bg-ink-2 px-1.5 py-0.5 text-[10px] font-medium text-fg-muted"
                              >
                                {agent.vendor}
                              </span>
                            </div>

                            <div class="flex items-center gap-1.5">
                              {#if agent.detected}
                                <span
                                  class="rounded bg-accent/15 px-1.5 py-0.5 text-[10px] font-semibold text-accent"
                                >
                                  {i18n.t('agent_detected_badge')}
                                </span>
                              {:else}
                                <span
                                  class="rounded bg-ink-2 px-1.5 py-0.5 text-[10px] text-fg-muted"
                                >
                                  {i18n.t('agent_not_detected_badge')}
                                </span>
                              {/if}

                              {#if isReady}
                                <span
                                  class="flex items-center gap-1 rounded bg-ok/20 px-2 py-0.5 text-[10px] font-bold text-ok"
                                >
                                  <Check size={11} />
                                  {i18n.t('agent_status_ready')}
                                </span>
                              {:else if isPartial}
                                <span
                                  class="rounded bg-amber-500/20 px-2 py-0.5 text-[10px] font-bold text-amber-400"
                                >
                                  {i18n.t('agent_status_partial')}
                                </span>
                              {:else}
                                <span
                                  class="rounded bg-ink-2 px-2 py-0.5 text-[10px] text-fg-muted"
                                >
                                  {i18n.t('agent_status_uninstalled')}
                                </span>
                              {/if}
                            </div>
                          </div>

                          <div
                            class="mt-1 flex flex-col gap-1 rounded-lg bg-ink-1/90 p-2 font-mono text-[10px] text-fg-muted"
                          >
                            <div class="truncate" title={agent.mcpConfigPath}>
                              <span class="font-bold text-fg">MCP:</span>
                              {agent.mcpConfigPath}
                            </div>
                            <div class="truncate" title={agent.skillPath}>
                              <span class="font-bold text-fg">Skill:</span>
                              {agent.skillPath}
                            </div>
                          </div>
                        </div>

                        <div class="flex items-center justify-end gap-2 pt-1">
                          {#if agent.mcpInstalled || agent.skillInstalled}
                            <button
                              type="button"
                              disabled={busyAgentId !== null}
                              onclick={() => handleUninstallAgent(agent.id)}
                              class="flex items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1.5 text-[11px] font-semibold text-fg-muted hover:border-red-500/50 hover:text-red-400 disabled:opacity-50"
                            >
                              <Trash2 size={12} />
                              <span>{i18n.t('agent_btn_uninstall')}</span>
                            </button>
                          {/if}
                          <button
                            type="button"
                            disabled={busyAgentId !== null}
                            onclick={() => handleInstallAgent(agent.id)}
                            class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[11px] font-bold transition-all disabled:opacity-50 {isReady
                              ? 'border border-line bg-ink-2 text-fg hover:border-accent'
                              : 'bg-accent text-accent-ink hover:opacity-90'}"
                          >
                            {#if busyAgentId === agent.id}
                              <Loader2 size={12} class="animate-spin" />
                            {:else}
                              <Download size={12} />
                            {/if}
                            <span>
                              {isReady
                                ? i18n.t('agent_btn_reinstall')
                                : i18n.t('agent_btn_install')}
                            </span>
                          </button>
                        </div>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>

              <!-- Section 2: Manual Installation & Export for Any Other Agent -->
              {#if agentHub}
                <div class="flex flex-col gap-4 rounded-2xl border border-line bg-ink-1 p-5">
                  <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <div class="text-xs font-bold text-fg">
                        {i18n.t('agent_manual_section')}
                      </div>
                      <p class="mt-0.5 text-xs text-fg-muted">
                        {i18n.t('agent_manual_sub')}
                      </p>
                    </div>

                    <button
                      type="button"
                      onclick={handleExportAgentBundle}
                      class="flex shrink-0 items-center gap-1.5 rounded-xl border border-accent/50 bg-accent/10 px-3.5 py-2 text-xs font-bold whitespace-nowrap text-accent hover:bg-accent/20"
                    >
                      <FolderOpen size={14} />
                      <span>{i18n.t('agent_export_bundle_btn')}</span>
                    </button>
                  </div>

                  <!-- Local Executable Path Bar -->
                  <div
                    class="flex items-center justify-between gap-2 rounded-xl border border-line bg-ink-0 px-3.5 py-2.5"
                  >
                    <div class="min-w-0 flex-1">
                      <div class="text-[10px] font-semibold text-fg-muted">
                        {i18n.t('agent_bin_path_label')}
                      </div>
                      <div class="truncate font-mono text-xs text-fg">
                        {agentHub.mcpExePath}
                      </div>
                    </div>
                    <button
                      type="button"
                      onclick={() => copySnippet(agentHub?.mcpExePath ?? '')}
                      class="flex shrink-0 items-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1.5 text-xs font-semibold text-fg hover:border-accent"
                    >
                      <Copy size={13} />
                    </button>
                  </div>

                  <!-- Snippet Switcher & Copy Bar -->
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <div class="flex flex-wrap items-center gap-1.5">
                      <button
                        type="button"
                        onclick={() => (activeSnippetTab = 'json')}
                        class="rounded-lg px-3 py-1.5 text-xs font-bold transition-all {activeSnippetTab ===
                        'json'
                          ? 'bg-accent text-accent-ink'
                          : 'bg-ink-2 text-fg-muted hover:text-fg'}"
                      >
                        MCP JSON
                      </button>
                      <button
                        type="button"
                        onclick={() => (activeSnippetTab = 'toml')}
                        class="rounded-lg px-3 py-1.5 text-xs font-bold transition-all {activeSnippetTab ===
                        'toml'
                          ? 'bg-accent text-accent-ink'
                          : 'bg-ink-2 text-fg-muted hover:text-fg'}"
                      >
                        Codex TOML
                      </button>
                      <button
                        type="button"
                        onclick={() => (activeSnippetTab = 'cli')}
                        class="rounded-lg px-3 py-1.5 text-xs font-bold transition-all {activeSnippetTab ===
                        'cli'
                          ? 'bg-accent text-accent-ink'
                          : 'bg-ink-2 text-fg-muted hover:text-fg'}"
                      >
                        CLI Command
                      </button>
                      <button
                        type="button"
                        onclick={() => (activeSnippetTab = 'skill')}
                        class="rounded-lg px-3 py-1.5 text-xs font-bold transition-all {activeSnippetTab ===
                        'skill'
                          ? 'bg-accent text-accent-ink'
                          : 'bg-ink-2 text-fg-muted hover:text-fg'}"
                      >
                        SKILL.md
                      </button>
                    </div>

                    <button
                      type="button"
                      onclick={() => {
                        if (!agentHub) return;
                        const txt =
                          activeSnippetTab === 'json'
                            ? agentHub.mcpJsonSnippet
                            : activeSnippetTab === 'toml'
                              ? agentHub.codexTomlSnippet
                              : activeSnippetTab === 'cli'
                                ? agentHub.cliExampleSnippet
                                : agentHub.skillMarkdown;
                        copySnippet(txt);
                      }}
                      class="flex items-center gap-1.5 rounded-lg border border-line bg-ink-2 px-3 py-1.5 text-xs font-bold text-fg hover:border-accent"
                    >
                      <Copy size={13} />
                      <span>
                        {activeSnippetTab === 'json'
                          ? i18n.t('agent_copy_mcp_json')
                          : activeSnippetTab === 'toml'
                            ? i18n.t('agent_copy_codex_toml')
                            : activeSnippetTab === 'cli'
                              ? i18n.t('agent_copy_cli_cmd')
                              : i18n.t('agent_copy_skill_md')}
                      </span>
                    </button>
                  </div>

                  <pre
                    class="max-h-60 overflow-auto rounded-xl border border-line bg-ink-0 p-3.5 font-mono text-[11px] leading-relaxed text-fg-muted">{activeSnippetTab ===
                    'json'
                      ? agentHub.mcpJsonSnippet
                      : activeSnippetTab === 'toml'
                        ? agentHub.codexTomlSnippet
                        : activeSnippetTab === 'cli'
                          ? agentHub.cliExampleSnippet
                          : agentHub.skillMarkdown}</pre>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
