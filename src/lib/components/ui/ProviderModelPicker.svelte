<script lang="ts">
  import {
    ChevronUp,
    Search,
    Server,
    Check,
    Settings,
    Sparkles
  } from '@lucide/svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n } from '$lib/i18n/index.svelte';
  import type { ProviderProfile } from '$lib/api/types';

  let open = $state(false);
  let searchQuery = $state('');
  let wrapEl = $state<HTMLDivElement | null>(null);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  const IMG_REGEX =
    /image|img|dall|flux|seedream|seededit|banana|imagen|stable|sdxl|sd3|midjourney|\bmj\b|kolors|cogview|wanx|hunyuan-image|ideogram|recraft|playground|pixart|photon|irag/i;

  function isImageModel(m: string): boolean {
    return IMG_REGEX.test(m);
  }

  function sortedModelsForProfile(prof: ProviderProfile, query: string): string[] {
    const raw = prof.models ?? [];
    const set = new Set<string>();
    for (const m of raw) {
      if (m) set.add(m);
    }
    if (prof.apiModel && set.size > 0) {
      set.add(prof.apiModel);
    }
    const all = Array.from(set);
    const q = query.trim().toLowerCase();
    const filtered = q
      ? all.filter(
          (m) =>
            m.toLowerCase().includes(q) || prof.name.toLowerCase().includes(q)
        )
      : all;

    // Sort image-capable models first
    return filtered.sort((a, b) => {
      const aImg = isImageModel(a) ? 0 : 1;
      const bImg = isImageModel(b) ? 0 : 1;
      if (aImg !== bImg) return aImg - bImg;
      return a.localeCompare(b);
    });
  }

  let groupedProviders = $derived.by(() => {
    const profiles = appStore.settings?.profiles ?? [];
    const q = searchQuery.trim();
    return profiles
      .map((prof) => ({
        profile: prof,
        models: sortedModelsForProfile(prof, q)
      }))
      .filter((g) => g.models.length > 0 || !q);
  });

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation();
    open = !open;
    if (open) {
      searchQuery = '';
      setTimeout(() => searchInputEl?.focus(), 20);
    }
  }

  async function selectModel(profileId: string, modelId: string) {
    const clean = modelId.trim();
    if (!clean) return;
    await appStore.selectProviderAndModel(profileId, clean);
    open = false;
  }

  function handleWindowClick(e: MouseEvent) {
    if (open && wrapEl && !wrapEl.contains(e.target as Node)) {
      open = false;
    }
  }
</script>

<svelte:window onclick={handleWindowClick} />

{#if appStore.settings && appStore.activeProfile}
  <div class="relative" bind:this={wrapEl}>
    <!-- Single Unified Trigger Button -->
    <button
      type="button"
      onclick={toggleMenu}
      title={`${appStore.activeProfile.name} / ${appStore.activeProfile.apiModel || i18n.t('model_none_selected')}`}
      class="flex items-center gap-2 rounded-lg border border-line bg-ink-2 px-2.5 py-1 text-xs transition-colors hover:border-accent focus:border-accent"
    >
      <span
        class="h-2 w-2 shrink-0 rounded-full {appStore.activeProfile.hasKey
          ? 'bg-ok'
          : 'bg-err'}"
      ></span>
      <span class="font-semibold text-fg">{appStore.activeProfile.name}</span>
      <span class="text-fg-subtle">/</span>
      <span
        class="max-w-[180px] truncate font-mono text-xs {appStore.activeProfile.apiModel
          ? 'text-accent'
          : 'text-fg-muted'}"
        title={appStore.activeProfile.apiModel || ''}
      >
        {appStore.activeProfile.apiModel || i18n.t('model_none_selected')}
      </span>
      <ChevronUp
        size={13}
        strokeWidth={2}
        class="ml-0.5 text-fg-muted transition-transform duration-150 {open
          ? 'rotate-180 text-accent'
          : ''}"
      />
    </button>

    <!-- Upward Popover Menu Grouped by Provider -->
    {#if open}
      <div
        class="absolute bottom-[calc(100%+8px)] left-0 z-50 flex max-h-96 w-96 flex-col overflow-hidden rounded-xl border border-line-strong bg-ink-1 shadow-2xl"
      >
        <!-- Search Filter Header -->
        <div class="flex items-center gap-2 border-b border-line bg-ink-0/80 px-3 py-2.5">
          <Search size={14} strokeWidth={1.75} class="shrink-0 text-fg-muted" />
          <input
            bind:this={searchInputEl}
            bind:value={searchQuery}
            onkeydown={(e) => {
              if (e.key === 'Enter' && searchQuery.trim() && appStore.activeProfile) {
                e.preventDefault();
                selectModel(appStore.activeProfile.id, searchQuery.trim());
              } else if (e.key === 'Escape') {
                open = false;
              }
            }}
            placeholder={i18n.t('model_search_placeholder')}
            spellcheck="false"
            autocomplete="off"
            class="w-full bg-transparent font-mono text-xs text-fg outline-none placeholder:text-fg-subtle"
          />
        </div>

        <!-- Provider-Grouped Model List -->
        <div class="flex flex-1 flex-col gap-2 overflow-y-auto p-2">
          {#if groupedProviders.length === 0 && searchQuery.trim()}
            <button
              type="button"
              onclick={() =>
                appStore.activeProfile &&
                selectModel(appStore.activeProfile.id, searchQuery.trim())}
              class="rounded-lg px-3 py-3 text-left text-xs text-fg-muted hover:bg-ink-2 hover:text-fg"
            >
              {i18n.t('model_empty_hint')}:
              <span class="ml-1 font-mono text-accent">{searchQuery.trim()}</span>
            </button>
          {:else}
            {#each groupedProviders as group (group.profile.id)}
              <div class="rounded-lg border border-line/70 bg-ink-0/40 p-1.5">
                <!-- Provider Group Header -->
                <div class="flex items-center justify-between px-2 py-1">
                  <div class="flex items-center gap-1.5">
                    <Server size={12} class="text-accent" />
                    <span class="text-xs font-bold text-fg">{group.profile.name}</span>
                    <span
                      class="rounded bg-ink-2 px-1.5 py-0.5 font-mono text-[9px] text-fg-muted"
                    >
                      {group.profile.apiMode === 'chat' ? '/chat' : '/images'}
                    </span>
                  </div>
                  <span
                    class="flex items-center gap-1 text-[10px] font-medium {group.profile.hasKey
                      ? 'text-ok'
                      : 'text-err'}"
                  >
                    <span
                      class="h-1.5 w-1.5 rounded-full {group.profile.hasKey
                        ? 'bg-ok'
                        : 'bg-err'}"
                    ></span>
                    <span>
                      {group.profile.hasKey
                        ? i18n.t('key_configured')
                        : i18n.t('key_missing')}
                    </span>
                  </span>
                </div>

                <!-- Models inside this Provider -->
                <div class="mt-1 flex flex-col gap-0.5">
                  {#if group.models.length === 0}
                    <div class="px-2.5 py-2 text-[11px] text-fg-subtle">
                      {i18n.t('model_none_enabled_hint')}
                    </div>
                  {:else}
                    {#each group.models as m (m)}
                      {@const isActive =
                        appStore.activeProfile.id === group.profile.id &&
                        appStore.activeProfile.apiModel === m}
                      <button
                        type="button"
                        onclick={() => selectModel(group.profile.id, m)}
                        title={m}
                        class="flex w-full items-center justify-between gap-2 rounded-md px-2.5 py-1.5 text-left font-mono text-xs transition-colors {isActive
                          ? 'bg-accent/15 font-semibold text-accent'
                          : 'text-fg hover:bg-ink-2'}"
                      >
                        <span class="truncate" title={m}>{m}</span>
                        <span class="flex shrink-0 items-center gap-1.5">
                          {#if isImageModel(m)}
                            <span
                              class="flex items-center gap-0.5 rounded border border-accent/35 bg-accent/15 px-1.5 py-0.5 font-sans text-[9px] font-semibold text-accent"
                            >
                              <Sparkles size={9} />
                              <span>IMAGE</span>
                            </span>
                          {/if}
                          {#if isActive}
                            <Check size={13} strokeWidth={2.2} class="text-accent" />
                          {/if}
                        </span>
                      </button>
                    {/each}
                  {/if}
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <!-- Footer Link to Settings for Fetching / Managing Providers -->
        <button
          type="button"
          onclick={() => {
            open = false;
            appStore.activeView = 'settings';
          }}
          class="flex items-center justify-center gap-1.5 border-t border-line bg-ink-0/90 px-3 py-2 text-[11px] font-semibold text-fg-muted transition-colors hover:bg-ink-2 hover:text-accent"
        >
          <Settings size={12} />
          <span>{i18n.t('model_manage_in_settings')}</span>
        </button>
      </div>
    {/if}
  </div>
{/if}
