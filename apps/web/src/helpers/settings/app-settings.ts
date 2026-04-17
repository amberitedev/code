export type Theme = 'light' | 'dark' | 'system';
export type NotificationPref = 'all' | 'important' | 'none';

export interface AppSettings {
  coreUrl: string;
  theme: Theme;
  notifications: NotificationPref;
  consoleFontSize: number;
  consoleTimestamp: boolean;
}

const STORAGE_KEY = 'LODESTONE_APP_SETTINGS';

const DEFAULT_SETTINGS: AppSettings = {
  coreUrl: 'http://localhost:16662',
  theme: 'system',
  notifications: 'all',
  consoleFontSize: 14,
  consoleTimestamp: true,
};

export function getSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(settings: Partial<AppSettings>): AppSettings {
  const current = getSettings();
  const updated = { ...current, ...settings };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(updated));
  return updated;
}

export function clearSettings(): void {
  localStorage.removeItem(STORAGE_KEY);
}

export function resetSettings(): AppSettings {
  clearSettings();
  return { ...DEFAULT_SETTINGS };
}
