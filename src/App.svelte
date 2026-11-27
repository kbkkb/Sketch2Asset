<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import {
    CheckCircle2,
    AlertCircle,
    Info,
    PanelBottomClose,
    Power,
    Loader2,
    Square,
    X
  } from '@lucide/svelte';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import GenerateView from '$features/generate/GenerateView.svelte';
  import StudioView from '$features/studio/StudioView.svelte';
  import CharacterView from '$features/character/CharacterView.svelte';
  import DecomposeView from '$features/decompose/DecomposeView.svelte';
  import SettingsView from '$features/settings/SettingsView.svelte';
  import { appStore } from '$lib/stores/settings.svelte';
  import { api } from '$lib/api';
  import { i18n } from '$lib/i18n/index.svelte';
  import { loadLastActiveView, saveLastActiveView } from '$lib/stores/workspace';

  let rememberCloseChoice = $state(false);
  let expandedToastError = $state<string | null>(null);

  onMount(() => {
    const savedView = loadLastActiveView();
    if (
      savedView === 'generate' ||
      savedView === 'studio' ||
      savedView === 'character' ||
      savedView === 'decompose'
    ) {
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

    let unlistenMcp: (() => void) | null = null;
    void listen<any>('mcp-ui-event', async (ev) => {
      const p = ev.payload ?? {};
      const isEn = i18n.locale === 'en';
      switch (p.kind) {
        case 'settings_synced':
          await appStore.reloadSettingsFromBackend();
          appStore.notifyAssetsChanged();
          if (p.toast) appStore.toast('info', p.toast);
          break;
        case 'generate_started':
          appStore.activeView = 'generate';
          appStore.activeMcpBanner = {
            taskId: p.taskId,
            view: 'generate',
            label: isEn ? 'AI Agent generating asset...' : 'AI Agent 正在生图...'
          };
          break;
        case 'character_started':
          appStore.activeView = 'character';
          appStore.activeMcpBanner = {
            taskId: p.taskId,
            view: 'character',
            label: isEn ? 'AI Agent generating character...' : 'AI Agent 正在角色工坊生成...'
          };
          break;
        case 'studio_started':
          appStore.activeView = 'studio';
          appStore.activeMcpBanner = {
            taskId: p.taskId,
            view: 'studio',
            label:
              p.op === 'matting'
                ? isEn
                  ? 'AI Agent removing background...'
                  : 'AI Agent 正在智能抠图...'
                : isEn
                  ? 'AI Agent editing region...'
                  : 'AI Agent 正在局部重绘...'
          };
          break;
        case 'decompose_started':
          appStore.activeView = 'decompose';
          appStore.activeMcpBanner = {
            taskId: p.taskId,
            view: 'decompose',
            label: isEn ? 'AI Agent decomposing 23 layers...' : 'AI Agent 正在 23 层智能拆图...'
          };
          break;
        case 'pose_updated':
          appStore.activeView = 'character';
          break;
        case 'generate_completed':
        case 'character_completed':
        case 'studio_completed':
        case 'decompose_completed':
          if (appStore.activeMcpBanner?.taskId === p.taskId) {
            appStore.activeMcpBanner = null;
          }
          appStore.notifyAssetsChanged();
          await appStore.refreshProjects();
          break;
        case 'generate_cancelled':
        case 'generate_failed':
        case 'character_cancelled':
        case 'character_failed':
        case 'studio_cancelled':
        case 'studio_failed':
        case 'decompose_cancelled':
        case 'decompose_failed':
          if (appStore.activeMcpBanner?.taskId === p.taskId) {
            appStore.activeMcpBanner = null;
          }
          break;
      }
    }).then((fn) => {
      unlistenMcp = fn;
    });

    return () => {
      clearInterval(revTimer);
      unlistenClose?.();
      unlistenMcp?.();
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
    <div class={appStore.activeView === 'decompose' ? 'flex h-full w-full overflow-hidden' : 'hidden'}>
      <DecomposeView />
    </div>
    {#if appStore.activeView === 'settings'}
      <SettingsView />
    {/if}
  </main>

  <!-- Floating Global MCP Live Activity Pill -->
  {#if appStore.activeMcpBanner}
    {@const banner = appStore.activeMcpBanner}
    <div
      class="fixed top-12 right-4 z-[95] flex items-center gap-2.5 rounded-full border border-accent/50 bg-ink-1/95 px-3 py-1.5 text-xs shadow-2xl backdrop-blur-xl"
    >
      <button
        type="button"
        onclick={() => (appStore.activeView = banner.view)}
        class="flex items-center gap-1.5 font-semibold text-accent hover:underline"
      >
        <Loader2 size={13} class="animate-spin" />
        <span class="rounded bg-accent/15 px-1.5 py-0.5 font-mono text-[10px] font-bold">MCP</span>
        <span>{banner.label}</span>
      </button>
      <button
        type="button"
        onclick={() => api.cancelMcpTask(banner.taskId)}
        class="flex items-center gap-1 rounded-full border border-err/60 bg-err/15 px-2.5 py-0.5 text-[11px] font-bold text-err transition-colors hover:bg-err hover:text-white"
        title={i18n.locale === 'en' ? 'Stop MCP Task' : '手动中止当前 AI Agent 任务'}
      >
        <Square size={10} fill="currentColor" />
        <span>{i18n.locale === 'en' ? 'Stop' : '中止'}</span>
      </button>
    </div>
  {/if}

  <!-- Global Top-Center Prominent Toast Stack -->
  <div
    class="pointer-events-none fixed top-14 left-1/2 z-[100] flex max-w-[min(92vw,720px)] -translate-x-1/2 flex-col items-center gap-2.5"
  >
    {#each appStore.toasts as item (item.id)}
      <div
        class="pointer-events-auto flex min-w-[280px] max-w-full items-center justify-center gap-2.5 rounded-2xl border px-5 py-3 text-sm font-bold shadow-2xl backdrop-blur-xl transition-all {item.kind ===
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
        <span class="line-clamp-2 break-all">{item.text}</span>
        {#if item.kind === 'err' && item.text.length > 70}
          <button
            type="button"
            onclick={() => (expandedToastError = item.text)}
            class="shrink-0 rounded-lg border border-err/40 bg-err/15 px-2 py-0.5 text-xs font-bold text-err transition-colors hover:bg-err hover:text-white"
          >
            {i18n.locale === 'en' ? 'Details' : '详情'}
          </button>
        {/if}
      </div>
    {/each}
  </div>

  {#if expandedToastError}
    <div
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={() => (expandedToastError = null)}
      onkeydown={(e) => e.key === 'Escape' && (expandedToastError = null)}
      class="fixed inset-0 z-[115] flex items-center justify-center bg-black/70 p-6 backdrop-blur-sm"
    >
      <div
        role="presentation"
        onclick={(e) => e.stopPropagation()}
        class="flex max-h-[82vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-err/50 bg-ink-1 shadow-2xl"
      >
        <div class="flex items-center justify-between border-b border-line bg-err/10 px-5 py-3">
          <div class="flex items-center gap-2 text-sm font-extrabold text-err">
            <AlertCircle size={17} />
            <span>{i18n.locale === 'en' ? 'Full Error Log' : '完整错误详情'}</span>
          </div>
          <button
            type="button"
            onclick={() => (expandedToastError = null)}
            class="rounded-lg p-1 text-fg-muted hover:bg-ink-2 hover:text-fg"
          >
            <X size={16} />
          </button>
        </div>
        <pre
          class="flex-1 overflow-auto p-5 font-mono text-xs leading-relaxed select-text break-all whitespace-pre-wrap text-fg"
        >{expandedToastError}</pre>
        <div class="flex items-center justify-end gap-2 border-t border-line bg-ink-0 px-5 py-3">
          <button
            type="button"
            onclick={async () => {
              try {
                await navigator.clipboard.writeText(expandedToastError || '');
                appStore.toast('ok', i18n.t('toast_copied'));
              } catch {}
            }}
            class="rounded-xl bg-accent px-4 py-1.5 text-xs font-extrabold text-accent-ink hover:brightness-105"
          >
            {i18n.locale === 'en' ? 'Copy' : '复制完整内容'}
          </button>
        </div>
      </div>
    </div>
  {/if}

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
