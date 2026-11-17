<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import {
    CheckCircle2,
    AlertCircle,
    Info,
    PanelBottomClose,
    Power,
    X
  } from '@lucide/svelte';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import GenerateView from '$features/generate/GenerateView.svelte';
  import StudioView from '$features/studio/StudioView.svelte';
  import CharacterView from '$features/character/CharacterView.svelte';
  import SettingsView from '$features/settings/SettingsView.svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { api } from '$lib/api';
  import { i18n } from '$lib/i18n/index.svelte';
  import { loadLastActiveView, saveLastActiveView } from '$lib/stores/workspace';

  let rememberCloseChoice = $state(false);

  onMount(() => {
    const savedView = loadLastActiveView();
    if (savedView === 'generate' || savedView === 'studio' || savedView === 'character') {
      appStore.activeView = savedView;
    }
    appStore.init();

    let lastRev = '';
    let revPolling = false;
    const pollLibraryRevision = async () => {
      if (document.visibilityState !== 'visible' || revPolling) return;
      revPolling = true;
      try {
        const rev = await api.getLibraryRevision();
        if (!rev) return;
        if (!lastRev) {
          lastRev = rev;
          return;
        }
        if (rev !== lastRev) {
          lastRev = rev;
          await appStore.refreshProjects();
          appStore.notifyAssetsChanged();
        }
      } catch {
        // Ignore transient polling errors
      } finally {
        revPolling = false;
      }
    };

    void pollLibraryRevision();
    const revTimer = setInterval(pollLibraryRevision, 2500);

    let unlistenClose: (() => void) | null = null;
    void listen('app-close-requested', () => {
      rememberCloseChoice = false;
      appStore.closePromptOpen = true;
    }).then((fn) => {
      unlistenClose = fn;
    });

    return () => {
      clearInterval(revTimer);
      unlistenClose?.();
    };
  });

  $effect(() => {
    if (appStore.activeView !== 'settings') {
      saveLastActiveView(appStore.activeView);
    }
  });

  $effect(() => {
    if (appStore.closePromptOpen) {
      rememberCloseChoice = false;
    }
  });

  function blockBrowserContextMenu(e: MouseEvent) {
    // Allow custom context menu handlers that already handled or stopped propagation,
    // but ALWAYS suppress the default Edge/WebView2 browser menu (Inspect, Share, Magnify image, etc.)
    e.preventDefault();
  }

  function blockBrowserAccelerators(e: KeyboardEvent) {
    if (e.key === 'Escape' && appStore.closePromptOpen) {
      appStore.closePromptOpen = false;
      return;
    }
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

  <!-- Close Confirmation Modal (Minimize to System Tray vs Exit Application) -->
  {#if appStore.closePromptOpen}
    <div
      role="dialog"
      aria-modal="true"
      class="fixed inset-0 z-[110] flex items-center justify-center p-4"
    >
      <button
        type="button"
        aria-label={i18n.t('close_dialog_cancel')}
        onclick={() => (appStore.closePromptOpen = false)}
        class="fixed inset-0 bg-black/65 backdrop-blur-sm"
      ></button>
      <div
        class="relative z-10 flex w-full max-w-md flex-col gap-4 rounded-2xl border border-line-strong bg-ink-1 p-5 shadow-2xl"
      >
        <div class="flex items-start justify-between gap-3">
          <div class="flex items-center gap-2.5">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-accent/15 text-accent">
              <PanelBottomClose size={18} />
            </div>
            <div>
              <h3 class="font-display text-sm font-extrabold text-fg">
                {i18n.t('close_dialog_title')}
              </h3>
              <p class="mt-0.5 text-xs leading-relaxed text-fg-muted">
                {i18n.t('close_dialog_desc')}
              </p>
            </div>
          </div>
          <button
            type="button"
            onclick={() => (appStore.closePromptOpen = false)}
            aria-label="Close dialog"
            class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-fg-muted hover:bg-ink-2 hover:text-fg"
          >
            <X size={15} />
          </button>
        </div>

        <label class="flex cursor-pointer items-center gap-2 rounded-xl border border-line bg-ink-0/60 px-3 py-2.5 text-xs text-fg-muted transition-colors hover:border-line-strong hover:text-fg">
          <input
            type="checkbox"
            bind:checked={rememberCloseChoice}
            class="h-3.5 w-3.5 accent-accent"
          />
          <span>{i18n.t('close_dialog_remember')}</span>
        </label>

        <div class="flex items-center justify-end gap-2.5 pt-1">
          <button
            type="button"
            onclick={() => (appStore.closePromptOpen = false)}
            class="rounded-xl border border-line bg-ink-2 px-3.5 py-2 text-xs font-semibold text-fg-muted transition-colors hover:text-fg"
          >
            {i18n.t('close_dialog_cancel')}
          </button>
          <button
            type="button"
            onclick={() => appStore.confirmWindowClose('exit', rememberCloseChoice)}
            class="flex items-center gap-1.5 rounded-xl border border-err/40 bg-err/15 px-3.5 py-2 text-xs font-bold text-err transition-colors hover:bg-err hover:text-white"
          >
            <Power size={13} />
            <span>{i18n.t('close_dialog_exit_btn')}</span>
          </button>
          <button
            type="button"
            onclick={() => appStore.confirmWindowClose('tray', rememberCloseChoice)}
            class="flex items-center gap-1.5 rounded-xl bg-accent px-4 py-2 text-xs font-extrabold text-accent-ink shadow-sm transition-all hover:brightness-105"
          >
            <PanelBottomClose size={13} />
            <span>{i18n.t('close_dialog_tray_btn')}</span>
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
