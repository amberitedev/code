import { setKey, getKey, type StoredKey } from './key-store';
import { apiClient, healthCheck } from '@/helpers/api/client';
import type { ConnectionInfo, AuthResult } from '@/helpers/api/types';

const CONNECTION_STRING_PREFIX = 'ms_';

export function parseConnectionString(connectionString: string): ConnectionInfo | null {
  if (!connectionString.startsWith(CONNECTION_STRING_PREFIX)) {
    return null;
  }

  const parts = connectionString.slice(3).split(':');
  if (parts.length !== 3) {
    return null;
  }

  const [host, portStr, apiKey] = parts;
  const port = parseInt(portStr, 10);

  if (isNaN(port) || !host || !apiKey) {
    return null;
  }

  const keyType = apiKey.startsWith('owner_') ? 'owner' : 'member';

  return { host, port, apiKey, keyType };
}

export async function connect(connectionString: string): Promise<AuthResult> {
  const parsed = parseConnectionString(connectionString);
  if (!parsed) {
    return { success: false, key_type: 'member', permissions: [], modrinth_user_id: null, username: null };
  }

  const { host, port, apiKey, keyType } = parsed;

  const canReach = await healthCheck(`http://${host}:${port}`);
  if (!canReach) {
    return { success: false, key_type: keyType, permissions: [], modrinth_user_id: null, username: null };
  }

  const storedKey: StoredKey = { key: apiKey, type: keyType, host, port };
  setKey(storedKey);

  try {
    const result = await apiClient<AuthResult>('/api/auth/connect', {
      method: 'POST',
      body: { connection_string: connectionString },
    });

    return { ...result, success: true, key_type: keyType };
  } catch {
    return { success: false, key_type: keyType, permissions: [], modrinth_user_id: null, username: null };
  }
}

export function disconnect(): void {
  const key = getKey();
  if (key) {
    setKey({ ...key, key: '' });
  }
}

export function isConnected(): boolean {
  const key = getKey();
  return key !== null && key.key !== '';
}
