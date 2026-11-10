const STORAGE_KEYS = {
  activeView: 's2a_active_view_v2',
  generate: 's2a_workspace_generate_v2',
  studio: 's2a_workspace_studio_v2',
  character: 's2a_workspace_character_v2'
} as const;

export function loadPersistedState<T>(key: keyof typeof STORAGE_KEYS, fallback: T): T {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS[key]);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === 'object') {
      return { ...fallback, ...parsed };
    }
    return fallback;
  } catch {
    return fallback;
  }
}

export function savePersistedState<T>(key: keyof typeof STORAGE_KEYS, value: T): void {
  try {
    localStorage.setItem(STORAGE_KEYS[key], JSON.stringify(value));
  } catch {
    // ignore storage quota errors
  }
}

export function loadLastActiveView(): 'generate' | 'studio' | 'character' {
  try {
    const v = localStorage.getItem(STORAGE_KEYS.activeView);
    if (v === 'studio' || v === 'character' || v === 'generate') {
      return v;
    }
  } catch {
    // ignore
  }
  return 'generate';
}

export function saveLastActiveView(view: string): void {
  try {
    if (view === 'generate' || view === 'studio' || view === 'character') {
      localStorage.setItem(STORAGE_KEYS.activeView, view);
    }
  } catch {
    // ignore
  }
}
