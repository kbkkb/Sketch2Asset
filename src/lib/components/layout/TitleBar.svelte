<script lang="ts">
  import {
    Sun,
    Moon,
    Minus,
    Square,
    X,
    KeyRound,
    AlertCircle
  } from '@lucide/svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n } from '$lib/i18n/index.svelte';

  function minimizeWindow() {
    getCurrentWindow().minimize();
  }
  function toggleMaximizeWindow() {
    getCurrentWindow().toggleMaximize();
  }
  function closeWindow() {
    getCurrentWindow().close();
  }
</script>

<header
  data-tauri-drag-region
  class="flex h-12 shrink-0 items-center justify-between border-b border-line bg-ink-1/90 px-3 backdrop-blur-md select-none"
>
  <!-- Left: Official App Brand Emblem (matches .exe icon) & Custom Navigation Icons -->
  <div class="flex items-center gap-4">
    <div data-tauri-drag-region class="flex items-center gap-2.5 pl-1">
      <!-- Official Sketch2Asset Hex-Prism Brand Emblem SVG (matches icon.ico) -->
      <svg
        data-tauri-drag-region
        width="30"
        height="30"
        viewBox="0 0 64 64"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        class="shrink-0 drop-shadow-sm"
      >
        <rect
          x="3"
          y="3"
          width="58"
          height="58"
          rx="14"
          fill="#0B0C10"
          stroke="#C8FF3D"
          stroke-width="2.5"
        />
        <!-- Outer Forge Pentagon -->
        <polygon
          points="32,10 51,24 44,50 20,50 13,24"
          fill="#151924"
          stroke="#C8FF3D"
          stroke-width="3.2"
          stroke-linejoin="round"
        />
        <!-- Inner Cyan Prism Diamond -->
        <polygon
          points="32,19 45,32 32,45 19,32"
          fill="#00E5FF"
          fill-opacity="0.22"
          stroke="#00E5FF"
          stroke-width="2.6"
          stroke-linejoin="round"
        />
        <!-- Forge Core -->
        <circle cx="32" cy="32" r="4.2" fill="#C8FF3D" />
      </svg>

      <div data-tauri-drag-region class="leading-none">
        <div
          data-tauri-drag-region
          class="font-display text-xs font-extrabold tracking-wider text-fg"
        >
          {i18n.t('app_title')}
        </div>
        <div
          data-tauri-drag-region
          class="font-mono text-[9px] tracking-widest text-fg-muted uppercase"
        >
          {i18n.t('app_subtitle')}
        </div>
      </div>
    </div>

    <div class="h-4 w-px bg-line"></div>

    <!-- Navigation Tabs with Custom Redrawn Vector Icons -->
    <nav class="flex items-center gap-1.5">
      <!-- Tab 1: Asset Forge (生图工坊) -->
      <button
        type="button"
        onclick={() => (appStore.activeView = 'generate')}
        class="group flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all {appStore.activeView ===
        'generate'
          ? 'bg-accent text-accent-ink shadow-sm'
          : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
      >
        <svg width="15" height="15" viewBox="0 0 20 20" fill="none" class="shrink-0">
          <!-- Isometric Crystal Forge Cube + Spark -->
          <path
            d="M10 2L17 6V14L10 18L3 14V6L10 2Z"
            stroke="currentColor"
            stroke-width="1.75"
            stroke-linejoin="round"
          />
          <path
            d="M10 6.2L13.5 10L10 13.8L6.5 10L10 6.2Z"
            fill="currentColor"
            fill-opacity="0.25"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linejoin="round"
          />
          <circle cx="10" cy="10" r="1.3" fill="currentColor" />
        </svg>
        <span>{i18n.t('nav_generate')}</span>
      </button>

      <!-- Tab 2: Pro Studio (工作台 V2) -->
      <button
        type="button"
        onclick={() => (appStore.activeView = 'studio')}
        class="group flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all {appStore.activeView ===
        'studio'
          ? 'bg-accent text-accent-ink shadow-sm'
          : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
      >
        <svg width="15" height="15" viewBox="0 0 20 20" fill="none" class="shrink-0">
          <!-- Pixel Matrix + Matting Cutout Frame -->
          <rect
            x="2.5"
            y="2.5"
            width="15"
            height="15"
            rx="3"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-dasharray="3 2"
          />
          <rect x="5.5" y="5.5" width="4" height="4" rx="0.8" fill="currentColor" />
          <rect x="10.5" y="5.5" width="4" height="4" rx="0.8" fill="currentColor" fill-opacity="0.35" />
          <rect x="5.5" y="10.5" width="4" height="4" rx="0.8" fill="currentColor" fill-opacity="0.35" />
          <rect x="10.5" y="10.5" width="4" height="4" rx="0.8" fill="currentColor" />
        </svg>
        <span>{i18n.t('nav_studio')}</span>
      </button>

      <!-- Tab 3: Character Forge (角色工坊) -->
      <button
        type="button"
        onclick={() => (appStore.activeView = 'character')}
        class="group flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all {appStore.activeView ===
        'character'
          ? 'bg-accent text-accent-ink shadow-sm'
          : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
      >
        <svg width="15" height="15" viewBox="0 0 20 20" fill="none" class="shrink-0">
          <!-- Character Rigging & Turnaround Emblem -->
          <circle
            cx="10"
            cy="5.5"
            r="2.6"
            fill="currentColor"
            fill-opacity="0.22"
            stroke="currentColor"
            stroke-width="1.65"
          />
          <path
            d="M4.5 16.5C4.8 12.8 7 11 10 11C13 11 15.2 12.8 15.5 16.5"
            stroke="currentColor"
            stroke-width="1.65"
            stroke-linecap="round"
          />
          <path
            d="M2.5 9.5L5.5 10.5M17.5 9.5L14.5 10.5"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
        <span>{i18n.t('nav_character')}</span>
      </button>

      <!-- Tab 4: API & Settings (API 与设置) -->
      <button
        type="button"
        onclick={() => (appStore.activeView = 'settings')}
        class="flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all {appStore.activeView ===
        'settings'
          ? 'bg-accent text-accent-ink shadow-sm'
          : 'text-fg-muted hover:bg-ink-2 hover:text-fg'}"
      >
        <svg width="15" height="15" viewBox="0 0 20 20" fill="none" class="shrink-0">
          <!-- API Routing Node Hub Icon -->
          <path
            d="M4 6H10M14 6H16M4 14H6M10 14H16"
            stroke="currentColor"
            stroke-width="1.75"
            stroke-linecap="round"
          />
          <circle cx="12" cy="6" r="2.2" fill="currentColor" fill-opacity="0.25" stroke="currentColor" stroke-width="1.65" />
          <circle cx="8" cy="14" r="2.2" fill="currentColor" fill-opacity="0.25" stroke="currentColor" stroke-width="1.65" />
        </svg>
        <span>{i18n.t('nav_settings')}</span>
      </button>
    </nav>
  </div>

  <!-- Right: Active Provider Vault Status + Theme, Language & Native Window Controls -->
  <div class="flex items-center gap-1.5">
    {#if appStore.activeProfile}
      <button
        type="button"
        onclick={() => (appStore.activeView = 'settings')}
        title={appStore.activeProfile.hasKey ? i18n.t('key_configured') : i18n.t('key_missing')}
        class="mr-1 flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium transition-colors {appStore
          .activeProfile.hasKey
          ? 'border-ok/30 bg-ok/10 text-ok hover:border-ok/60'
          : 'border-err/40 bg-err/15 text-err hover:border-err'}"
      >
        {#if appStore.activeProfile.hasKey}
          <KeyRound size={12} strokeWidth={2} />
        {:else}
          <AlertCircle size={12} strokeWidth={2} />
        {/if}
        <span class="max-w-[110px] truncate">{appStore.activeProfile.name}</span>
      </button>
    {/if}

    <button
      type="button"
      onclick={() => appStore.toggleTheme()}
      aria-label="Toggle theme"
      class="flex h-8 w-8 items-center justify-center rounded-md text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg"
    >
      {#if appStore.settings?.theme === 'light'}
        <Moon size={15} strokeWidth={1.75} />
      {:else}
        <Sun size={15} strokeWidth={1.75} />
      {/if}
    </button>

    <div class="mx-1 h-4 w-px bg-line"></div>

    <button
      type="button"
      onclick={minimizeWindow}
      aria-label="Minimize window"
      class="flex h-8 w-8 items-center justify-center rounded-md text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg"
    >
      <Minus size={15} strokeWidth={1.75} />
    </button>
    <button
      type="button"
      onclick={toggleMaximizeWindow}
      aria-label="Maximize window"
      class="flex h-8 w-8 items-center justify-center rounded-md text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg"
    >
      <Square size={13} strokeWidth={1.75} />
    </button>
    <button
      type="button"
      onclick={closeWindow}
      aria-label="Close window"
      class="flex h-8 w-8 items-center justify-center rounded-md text-fg-muted transition-colors hover:bg-err hover:text-white"
    >
      <X size={15} strokeWidth={1.75} />
    </button>
  </div>
</header>
