<script lang="ts">
  import { onMount } from 'svelte';
  import { CheckCircle2, AlertCircle, Info } from '@lucide/svelte';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import GenerateView from '$features/generate/GenerateView.svelte';
  import StudioView from '$features/studio/StudioView.svelte';
  import CharacterView from '$features/character/CharacterView.svelte';
  import SettingsView from '$features/settings/SettingsView.svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { loadLastActiveView, saveLastActiveView } from '$lib/stores/workspace';

  onMount(() => {
    const savedView = loadLastActiveView();
    if (savedView === 'generate' || savedView === 'studio' || savedView === 'character') {
      appStore.activeView = savedView;
    }
    appStore.init();
  });

  $effect(() => {
    if (appStore.activeView !== 'settings') {
      saveLastActiveView(appStore.activeView);
    }
  });

  function blockBrowserContextMenu(e: MouseEvent) {
    // Allow custom context menu handlers that already handled or stopped propagation,
    // but ALWAYS suppress the default Edge/WebView2 browser menu (Inspect, Share, Magnify image, etc.)
    e.preventDefault();
  }

  function blockBrowserAccelerators(e: KeyboardEvent) {
    const key = e.key.toLowerCase();
    // Block F3, F5, F7, F12 and Ctrl+R / Ctrl+P / Ctrl+U / Ctrl+G / Ctrl+F / Ctrl+Shift+I
    if (
      e.key === 'F3' ||
      e.key === 'F5' ||
      e.key === 'F7' ||
      e.key === 'F12' ||
      ((e.ctrlKey || e.metaKey) && ['r', 'p', 'u', 'g', 'f'].includes(key)) ||
      ((e.ctrlKey || e.metaKey) && e.shiftKey && ['i', 'j', 'c'].includes(key))
    ) {
      e.preventDefault();
    }
  }
</script>

<svelte:window
  oncontextmenu={blockBrowserContextMenu}
  onkeydown={blockBrowserAccelerators}
  ondragstart={(e) => {
    if ((e.target as HTMLElement)?.tagName === 'IMG') {
      e.preventDefault();
    }
  }}
/>

<div class="flex h-screen w-screen flex-col overflow-hidden bg-ink-0 text-fg">
  <TitleBar />

  <main class="flex flex-1 overflow-hidden">
    <div class={appStore.activeView === 'generate' ? 'flex h-full w-full overflow-hidden' : 'hidden'}>
      <GenerateView />
    </div>
    <div class={appStore.activeView === 'studio' ? 'flex h-full w-full overflow-hidden' : 'hidden'}>
      <StudioView />
    </div>
    <div class={appStore.activeView === 'character' ? 'flex h-full w-full overflow-hidden' : 'hidden'}>
      <CharacterView />
    </div>
    {#if appStore.activeView === 'settings'}
      <SettingsView />
    {/if}
  </main>

  <!-- Global Top-Center Prominent Toast Stack -->
  <div
    class="pointer-events-none fixed top-14 left-1/2 z-[100] flex -translate-x-1/2 flex-col items-center gap-2.5"
  >
    {#each appStore.toasts as item (item.id)}
      <div
        class="pointer-events-auto flex min-w-[280px] items-center justify-center gap-2.5 rounded-2xl border px-5 py-3 text-sm font-bold shadow-2xl backdrop-blur-xl transition-all {item.kind ===
        'ok'
          ? 'border-ok/50 bg-ink-1/95 text-ok shadow-ok/10'
          : item.kind === 'err'
            ? 'border-err/50 bg-ink-1/95 text-err shadow-err/10'
            : 'border-accent/50 bg-ink-1/95 text-accent shadow-accent/10'}"
      >
        {#if item.kind === 'ok'}
          <CheckCircle2 size={18} strokeWidth={2.2} class="shrink-0" />
        {:else if item.kind === 'err'}
          <AlertCircle size={18} strokeWidth={2.2} class="shrink-0" />
        {:else}
          <Info size={18} strokeWidth={2.2} class="shrink-0" />
        {/if}
        <span>{item.text}</span>
      </div>
    {/each}
  </div>
</div>
