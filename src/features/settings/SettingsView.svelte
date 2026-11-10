<script lang="ts">
  import { onMount } from 'svelte';
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
    Globe
  } from '@lucide/svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n, SUPPORTED_LOCALES, type LocaleCode } from '$lib/i18n/index.svelte';
  import { api } from '$lib/api';
  import type { MattingModelInfo } from '$lib/api/types';

  let keyDraft = $state('');
  let showKey = $state(false);
  let fetching = $state(false);
  let poolSearch = $state('');
  let onlyImageFilter = $state(false);
  let customModelInput = $state('');
  let importInputEl = $state<HTMLInputElement | null>(null);

  // Local AI Matting Models in Settings
  let mattingModels = $state<MattingModelInfo[]>([]);
  let loadingModelId = $state<string | null>(null);

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
  }

  async function loadMattingModels() {
    try {
      mattingModels = await api.listMattingModels();
    } catch {
      mattingModels = [];
    }
  }

  onMount(() => {
    loadProfileKey();
    loadMattingModels();
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

  async function updateActiveProfileField(
    field: 'name' | 'apiBase' | 'apiMode' | 'apiModel',
    val: string
  ) {
    if (!appStore.settings || !appStore.activeProfile) return;
    const profiles = appStore.settings.profiles.map((p) =>
      p.id === appStore.activeProfile!.id ? { ...p, [field]: val } : p
    );
    await appStore.saveSettings({ profiles });
    appStore.toast('ok', i18n.t('toast_profile_saved'));
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

          <!-- Tab 3: Appearance & Backup -->
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
                    placeholder="https://api.openai.com/v1"
                    spellcheck="false"
                    class="rounded-lg border border-line bg-ink-0 px-3.5 py-2 font-mono text-xs text-fg outline-none focus:border-accent"
                  />
                </div>

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
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
