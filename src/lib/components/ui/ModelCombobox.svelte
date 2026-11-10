<script lang="ts">
  import { ChevronDown, RefreshCw, Search, Sparkles, Layers, Check } from '@lucide/svelte';
  import { i18n } from '$lib/i18n/index.svelte';

  interface Props {
    value: string;
    models: string[];
    fetching?: boolean;
    onselect: (modelId: string) => void;
    onfetch?: () => void;
  }

  let { value, models, fetching = false, onselect, onfetch }: Props = $props();

  let open = $state(false);
  let searchQuery = $state('');
  let wrapEl = $state<HTMLDivElement | null>(null);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  const IMG_REGEX =
    /image|img|dall|flux|seedream|seededit|banana|imagen|stable|sdxl|sd3|midjourney|\bmj\b|kolors|cogview|wanx|hunyuan-image|ideogram|recraft|playground|pixart|photon|irag/i;

  let filteredModels = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    const list = models.length ? models : ['gpt-image-1', 'gemini-2.5-flash-image', 'dall-e-3'];
    return q ? list.filter((m) => m.toLowerCase().includes(q)) : list;
  });

  let imageGroup = $derived(filteredModels.filter((m) => IMG_REGEX.test(m)));
  let otherGroup = $derived(filteredModels.filter((m) => !IMG_REGEX.test(m)));

  function openMenu() {
    open = true;
    searchQuery = '';
    setTimeout(() => {
      searchInputEl?.focus();
    }, 20);
  }

  function closeMenu() {
    open = false;
  }

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation();
    if (open) closeMenu();
    else openMenu();
  }

  function choose(modelId: string) {
    const trimmed = modelId.trim();
    if (!trimmed) return;
    onselect(trimmed);
    closeMenu();
  }

  function handleWindowClick(e: MouseEvent) {
    if (open && wrapEl && !wrapEl.contains(e.target as Node)) {
      closeMenu();
    }
  }
</script>

<svelte:window onclick={handleWindowClick} />

<div class="relative min-w-[260px] flex-1" bind:this={wrapEl}>
  <div class="flex items-center gap-1.5">
    <div
      class="group relative flex flex-1 items-center rounded-md border border-line bg-ink-2 transition-colors focus-within:border-accent hover:border-line-strong"
    >
      <input
        type="text"
        value={value}
        oninput={(e) => onselect((e.currentTarget as HTMLInputElement).value)}
        onclick={openMenu}
        placeholder={i18n.t('model_select_placeholder')}
        spellcheck="false"
        autocomplete="off"
        class="w-full bg-transparent py-1.5 pr-8 pl-3 font-mono text-xs text-fg outline-none placeholder:text-fg-subtle"
      />
      <button
        type="button"
        onclick={toggleMenu}
        aria-label="Toggle model dropdown"
        class="absolute right-1.5 flex h-6 w-6 items-center justify-center rounded text-fg-muted transition-colors hover:bg-ink-3 hover:text-fg"
      >
        <ChevronDown
          size={14}
          strokeWidth={1.75}
          class="transition-transform duration-150 {open ? 'rotate-180 text-accent' : ''}"
        />
      </button>
    </div>

    {#if onfetch}
      <button
        type="button"
        disabled={fetching}
        onclick={onfetch}
        class="flex shrink-0 items-center gap-1.5 rounded-md border border-line bg-ink-2 px-2.5 py-1.5 text-xs font-medium text-fg-muted transition-colors hover:border-accent hover:text-fg disabled:opacity-45"
      >
        <RefreshCw size={13} strokeWidth={1.75} class={fetching ? 'animate-spin text-accent' : ''} />
        <span>{fetching ? i18n.t('btn_fetching') : i18n.t('btn_fetch_models')}</span>
      </button>
    {/if}
  </div>

  {#if open}
    <div
      class="absolute top-[calc(100%+6px)] left-0 z-50 flex max-h-80 w-full min-w-[340px] flex-col overflow-hidden rounded-lg border border-line-strong bg-ink-1 shadow-2xl"
    >
      <!-- Search filter header -->
      <div class="flex items-center gap-2 border-b border-line bg-ink-0/60 px-3 py-2">
        <Search size={14} strokeWidth={1.75} class="shrink-0 text-fg-muted" />
        <input
          bind:this={searchInputEl}
          bind:value={searchQuery}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              if (filteredModels.length > 0 && filteredModels[0]) {
                choose(filteredModels[0]);
              } else if (searchQuery.trim()) {
                choose(searchQuery.trim());
              }
            } else if (e.key === 'Escape') {
              closeMenu();
            }
          }}
          placeholder={i18n.t('model_search_placeholder')}
          spellcheck="false"
          autocomplete="off"
          class="w-full bg-transparent font-mono text-xs text-fg outline-none placeholder:text-fg-subtle"
        />
      </div>

      <!-- Grouped scrollable list -->
      <div class="flex flex-1 flex-col gap-0.5 overflow-y-auto p-1.5">
        {#if filteredModels.length === 0}
          <button
            type="button"
            onclick={() => choose(searchQuery)}
            class="px-3 py-4 text-left text-xs text-fg-muted hover:text-fg"
          >
            {i18n.t('model_empty_hint')}:
            <span class="ml-1 font-mono text-accent">{searchQuery}</span>
          </button>
        {:else}
          {#if imageGroup.length > 0}
            <div
              class="flex items-center gap-1.5 px-2.5 pt-1.5 pb-1 text-[10px] font-semibold tracking-wider text-fg-muted uppercase"
            >
              <Sparkles size={11} strokeWidth={1.75} class="text-accent" />
              <span>{i18n.t('model_group_image')} ({imageGroup.length})</span>
            </div>
            {#each imageGroup as m (m)}
              <button
                type="button"
                onclick={() => choose(m)}
                class="flex w-full items-center justify-between gap-2 rounded-md px-2.5 py-1.5 text-left font-mono text-xs transition-colors {m === value
                  ? 'bg-accent/15 font-semibold text-accent'
                  : 'text-fg hover:bg-ink-2'}"
              >
                <span class="truncate">{m}</span>
                <span class="flex shrink-0 items-center gap-1.5">
                  <span
                    class="rounded border border-accent/35 bg-accent/15 px-1.5 py-0.5 font-sans text-[10px] font-semibold text-accent"
                  >
                    IMAGE
                  </span>
                  {#if m === value}
                    <Check size={13} strokeWidth={2} class="text-accent" />
                  {/if}
                </span>
              </button>
            {/each}
          {/if}

          {#if otherGroup.length > 0}
            <div
              class="flex items-center gap-1.5 px-2.5 pt-2 pb-1 text-[10px] font-semibold tracking-wider text-fg-muted uppercase"
            >
              <Layers size={11} strokeWidth={1.75} class="text-fg-subtle" />
              <span>{i18n.t('model_group_all')} ({otherGroup.length})</span>
            </div>
            {#each otherGroup as m (m)}
              <button
                type="button"
                onclick={() => choose(m)}
                class="flex w-full items-center justify-between gap-2 rounded-md px-2.5 py-1.5 text-left font-mono text-xs transition-colors {m === value
                  ? 'bg-accent/15 font-semibold text-accent'
                  : 'text-fg hover:bg-ink-2'}"
              >
                <span class="truncate">{m}</span>
                {#if m === value}
                  <Check size={13} strokeWidth={2} class="shrink-0 text-accent" />
                {/if}
              </button>
            {/each}
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>
