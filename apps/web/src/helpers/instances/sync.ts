import { ref } from 'vue';
import type { SyncStatus, ModDownload, SyncError } from '@/helpers/api/types';

const MODRINTH_CDN_URL = 'https://cdn.modrinth.com';

export interface LocalModHash {
  path: string;
  hash: string;
}

export function createSyncTracker(instanceId: string) {
  const status = ref<SyncStatus>({
    instance_id: instanceId,
    total_mods: 0,
    synced_mods: 0,
    missing_mods: [],
    errors: [],
  });

  const isSyncing = ref(false);

  async function getLocalModHashes(): Promise<LocalModHash[]> {
    try {
      return await window.indexedDB.databases();
    } catch {
      return [];
    }
  }

  async function checkCache(hash: string): Promise<boolean> {
    try {
      const caches = await caches.open('modrinth-mods-v1');
      const response = await caches.match(`${MODRINTH_CDN_URL}/files/${hash}`);
      return response !== undefined;
    } catch {
      return false;
    }
  }

  async function cacheMod(fileId: string, hash: string, data: ArrayBuffer): Promise<void> {
    try {
      const caches = await caches.open('modrinth-mods-v1');
      const response = new Response(data, {
        headers: {
          'Content-Type': 'application/java-archive',
          'X-Mod-Hash': hash,
          'X-File-Id': fileId,
        },
      });
      await caches.put(`${MODRINTH_CDN_URL}/files/${hash}`, response);
    } catch (error) {
      console.error('Failed to cache mod:', error);
    }
  }

  async function downloadMod(mod: ModDownload): Promise<boolean> {
    try {
      const modIndex = status.value.missing_mods.findIndex(
        (m) => m.file_id === mod.file_id
      );
      if (modIndex !== -1) {
        status.value.missing_mods[modIndex].status = 'downloading';
      }

      const response = await fetch(
        `https://api.modrinth.com/v2/project/${mod.project_id}/version/${mod.version_id}/file`
      );

      if (!response.ok) {
        throw new Error(`Failed to download mod: ${response.statusText}`);
      }

      const blob = await response.arrayBuffer();
      await cacheMod(mod.file_id, mod.hash, blob);

      if (modIndex !== -1) {
        status.value.missing_mods[modIndex].status = 'completed';
        status.value.synced_mods++;
      }

      return true;
    } catch (error) {
      const modIndex = status.value.missing_mods.findIndex(
        (m) => m.file_id === mod.file_id
      );
      if (modIndex !== -1) {
        status.value.missing_mods[modIndex].status = 'failed';
        status.value.errors.push({
          project_id: mod.project_id,
          file_id: mod.file_id,
          error: error instanceof Error ? error.message : 'Unknown error',
        });
      }
      return false;
    }
  }

  async function sync(localHashes: LocalModHash[], modpackMods: ModDownload[]): Promise<void> {
    isSyncing.value = true;
    status.value = {
      instance_id: instanceId,
      total_mods: modpackMods.length,
      synced_mods: 0,
      missing_mods: [],
      errors: [],
    };

    for (const mod of modpackMods) {
      const localMod = localHashes.find((h) => h.path === mod.path);
      if (!localMod || localMod.hash !== mod.hash) {
        const cached = await checkCache(mod.hash);
        if (!cached) {
          status.value.missing_mods.push({ ...mod, status: 'pending' });
        } else {
          status.value.synced_mods++;
        }
      } else {
        status.value.synced_mods++;
      }
    }

    for (const mod of status.value.missing_mods) {
      if (mod.status === 'pending') {
        await downloadMod(mod);
      }
    }

    isSyncing.value = false;
  }

  async function downloadMissing(): Promise<void> {
    for (const mod of status.value.missing_mods) {
      if (mod.status === 'pending' || mod.status === 'failed') {
        await downloadMod(mod);
      }
    }
  }

  return {
    status,
    isSyncing,
    sync,
    downloadMissing,
  };
}
