import { apiClient } from '@/helpers/api/client';
import type { ModpackConfig } from '@/helpers/api/types';

export async function getModpackConfig(instanceId: string): Promise<ModpackConfig | null> {
  try {
    return await apiClient<ModpackConfig>(`/api/instances/${instanceId}/modpack`, {
      method: 'GET',
    });
  } catch {
    return null;
  }
}

export interface UpdateModpackPayload {
  project_id: string;
  version_id: string;
  overrides?: Record<string, string>;
  linked_files?: ModpackConfig['linked_files'];
}

export async function updateModpackConfig(
  instanceId: string,
  config: UpdateModpackPayload
): Promise<ModpackConfig> {
  return apiClient<ModpackConfig>(`/api/instances/${instanceId}/modpack`, {
    method: 'PUT',
    body: config,
  });
}
