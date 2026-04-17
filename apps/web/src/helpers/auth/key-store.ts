const STORAGE_KEY = 'LODESTONE_API_KEY';

export interface StoredKey {
  key: string;
  type: 'owner' | 'member';
  host: string;
  port: number;
}

export function getKey(): StoredKey | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as StoredKey;
  } catch {
    return null;
  }
}

export function setKey(key: StoredKey): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(key));
}

export function clearKey(): void {
  localStorage.removeItem(STORAGE_KEY);
}

export function hasKey(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== null;
}

export function getApiKeyHeader(): string | null {
  const stored = getKey();
  return stored ? stored.key : null;
}
