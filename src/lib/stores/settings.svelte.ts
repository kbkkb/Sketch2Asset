import { getVersion } from '@tauri-apps/api/app';
import { api } from '$lib/api';
import type {
  AppSettings,
  AppUpdateInfo,
  AssetItem,
  ProjectInfo,
  ProviderProfile
} from '$lib/api/types';
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
  settingsTab = $state<'providers' | 'matting' | 'agents' | 'system'>('providers');
  studioPendingAsset = $state<AssetItem | null>(null);
  characterPendingAsset = $state<AssetItem | null>(null);
  characterSubMode = $state<'turnaround' | 'outfit' | 'spritesheet' | 'expression'>('turnaround');
  toasts = $state<ToastMessage[]>([]);
  assetsRevision = $state(0);
  appVersion = $state<string>('');
  updateInfo = $state<AppUpdateInfo | null>(null);
  isCheckingUpdate = $state(false);
  closePromptOpen = $state(false);

  notifyAssetsChanged() {
    this.assetsRevision++;
  }

  openInStudio(asset: AssetItem) {
    this.studioPendingAsset = asset;
    this.activeView = 'studio';
  }

  openInCharacterForge(
    asset: AssetItem,
    mode: 'turnaround' | 'outfit' | 'spritesheet' | 'expression' = 'outfit'
  ) {
    this.characterPendingAsset = asset;
    this.characterSubMode = mode;
    this.activeView = 'character';
  }

  openSettings(tab: 'providers' | 'matting' | 'agents' | 'system' = 'providers') {
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
      const [s, ver] = await Promise.all([
        api.getSettings(),
        getVersion().catch(() => '0.2.1')
      ]);
      this.appVersion = ver || '0.2.1';
      this.applySettings(s);
      this.projects = await api.listProjects();
      void this.refreshUpdateCheck(true);
    } catch (e: any) {
      this.toast('err', e?.message ?? 'Failed to initialize settings');
    }
  }

  async refreshUpdateCheck(silent = false): Promise<AppUpdateInfo | null> {
    if (this.isCheckingUpdate) return this.updateInfo;
    this.isCheckingUpdate = true;
    try {
      const res = await api.checkAppUpdate();
      this.updateInfo = res;
      if (!this.appVersion && res.currentVersion) {
        this.appVersion = res.currentVersion;
      }
      if (res.hasUpdate) {
        this.toast(
          'info',
          i18n.locale === 'en'
            ? `New version ${res.latestVersion} available — click top bar or Settings to update`
            : `发现新版本 ${res.latestVersion}，可在设置或顶栏点击更新`
        );
      } else if (!silent) {
        const ver = res.currentVersion || this.appVersion;
        this.toast(
          'ok',
          ver ? `${i18n.t('about_is_latest')} (v${ver})` : i18n.t('about_is_latest')
        );
      }
      return res;
    } catch (e: any) {
      if (!silent) {
        this.toast('err', String(e?.message || e));
      }
      return null;
    } finally {
      this.isCheckingUpdate = false;
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

  async requestClose() {
    const mode = this.settings?.closeBehavior || 'ask';
    if (mode === 'tray') {
      await api.performWindowClose('tray');
      return;
    }
    if (mode === 'exit') {
      await api.performWindowClose('exit');
      return;
    }
    this.closePromptOpen = true;
  }

  async confirmWindowClose(action: 'tray' | 'exit', remember: boolean) {
    this.closePromptOpen = false;
    if (remember && this.settings) {
      try {
        await this.saveSettings({ closeBehavior: action });
      } catch {
        // Proceed with close even if settings save throws
      }
    }
    await api.performWindowClose(action);
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
