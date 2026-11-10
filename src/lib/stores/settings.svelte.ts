import { api } from '$lib/api';
import type { AppSettings, AssetItem, ProjectInfo, ProviderProfile } from '$lib/api/types';
import { i18n } from '$lib/i18n/index.svelte';

export interface ToastMessage {
  id: string;
  kind: 'ok' | 'err' | 'info';
  text: string;
}

class AppStore {
  settings = $state<AppSettings | null>(null);
  projects = $state<ProjectInfo[]>([]);
  activeView = $state<'generate' | 'studio' | 'character' | 'settings'>('generate');
  settingsTab = $state<'providers' | 'matting' | 'system'>('providers');
  studioPendingAsset = $state<AssetItem | null>(null);
  characterPendingAsset = $state<AssetItem | null>(null);
  characterSubMode = $state<'turnaround' | 'outfit' | 'spritesheet'>('turnaround');
  toasts = $state<ToastMessage[]>([]);
  assetsRevision = $state(0);

  notifyAssetsChanged() {
    this.assetsRevision++;
  }

  openInStudio(asset: AssetItem) {
    this.studioPendingAsset = asset;
    this.activeView = 'studio';
  }

  openInCharacterForge(
    asset: AssetItem,
    mode: 'turnaround' | 'outfit' | 'spritesheet' = 'outfit'
  ) {
    this.characterPendingAsset = asset;
    this.characterSubMode = mode;
    this.activeView = 'character';
  }

  openSettings(tab: 'providers' | 'matting' | 'system' = 'providers') {
    this.settingsTab = tab;
    this.activeView = 'settings';
  }

  get activeProfile(): ProviderProfile | undefined {
    if (!this.settings) return undefined;
    return (
      this.settings.profiles.find((p) => p.id === this.settings!.activeProfileId) ??
      this.settings.profiles[0]
    );
  }

  get activeProjectInfo(): ProjectInfo | undefined {
    if (!this.settings?.activeProject) return undefined;
    return this.projects.find((p) => p.name === this.settings!.activeProject);
  }

  async init() {
    try {
      const s = await api.getSettings();
      this.applySettings(s);
      this.projects = await api.listProjects();
    } catch (e: any) {
      this.toast('err', e?.message ?? 'Failed to initialize settings');
    }
  }

  applySettings(s: AppSettings) {
    this.settings = s;
    i18n.locale = s.language === 'en' ? 'en' : 'zh-CN';
    document.documentElement.setAttribute('data-theme', s.theme || 'dark');
    document.documentElement.lang = i18n.locale;
  }

  async saveSettings(patch?: Partial<AppSettings>) {
    if (!this.settings) return;
    const next: AppSettings = { ...this.settings, ...patch };
    this.applySettings(next);
    const saved = await api.saveSettings(next);
    this.applySettings(saved);
  }

  async switchProfile(profileId: string) {
    await this.saveSettings({ activeProfileId: profileId });
  }

  async selectProviderAndModel(profileId: string, modelId: string) {
    if (!this.settings) return;
    const profiles = this.settings.profiles.map((p) =>
      p.id === profileId ? { ...p, apiModel: modelId } : p
    );
    await this.saveSettings({
      activeProfileId: profileId,
      profiles
    });
  }

  async setActiveModel(modelId: string) {
    if (!this.settings || !this.activeProfile) return;
    const profiles = this.settings.profiles.map((p) =>
      p.id === this.activeProfile!.id ? { ...p, apiModel: modelId } : p
    );
    await this.saveSettings({ profiles });
  }

  async toggleTheme() {
    if (!this.settings) return;
    const nextTheme = this.settings.theme === 'light' ? 'dark' : 'light';
    await this.saveSettings({ theme: nextTheme });
  }

  async toggleLanguage() {
    if (!this.settings) return;
    const nextLang = this.settings.language === 'en' ? 'zh-CN' : 'en';
    await this.saveSettings({ language: nextLang });
  }

  async refreshProjects() {
    this.projects = await api.listProjects();
  }

  toast(kind: 'ok' | 'err' | 'info', text: string) {
    const id = Math.random().toString(36).slice(2);
    this.toasts = [...this.toasts, { id, kind, text }];
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    }, 3600);
  }
}

export const appStore = new AppStore();
