import { apiClient } from '@/helpers/api/client';
import type { Instance } from '@/helpers/api/types';

export interface CreateInstancePayload {
  name: string;
  game_type: Instance['game_type'];
  version: string;
  java_version?: string | null;
  port?: number | null;
}

export async function listInstances(): Promise<Instance[]> {
  return apiClient<Instance[]>('/api/instances', { method: 'GET' });
}

export async function getInstance(id: string): Promise<Instance> {
  return apiClient<Instance>(`/api/instances/${id}`, { method: 'GET' });
}

export async function createInstance(payload: CreateInstancePayload): Promise<Instance> {
  return apiClient<Instance>('/api/instances', {
    method: 'POST',
    body: payload,
  });
}

export async function deleteInstance(id: string): Promise<void> {
  return apiClient<void>(`/api/instances/${id}`, { method: 'DELETE' });
}

export async function updateInstance(id: string, payload: Partial<CreateInstancePayload>): Promise<Instance> {
  return apiClient<Instance>(`/api/instances/${id}`, {
    method: 'PATCH',
    body: payload,
  });
}
