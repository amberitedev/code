import { apiClient } from '@/helpers/api/client';

export type InstanceAction = 'start' | 'stop' | 'kill' | 'restart';

export async function startInstance(id: string): Promise<void> {
  return apiClient(`/api/instances/${id}/start`, { method: 'POST' });
}

export async function stopInstance(id: string): Promise<void> {
  return apiClient(`/api/instances/${id}/stop`, { method: 'POST' });
}

export async function killInstance(id: string): Promise<void> {
  return apiClient(`/api/instances/${id}/kill`, { method: 'POST' });
}

export async function restartInstance(id: string): Promise<void> {
  return apiClient(`/api/instances/${id}/restart`, { method: 'POST' });
}

export async function sendCommand(id: string, command: string): Promise<{ success: boolean }> {
  return apiClient(`/api/instances/${id}/command`, {
    method: 'POST',
    body: { command },
  });
}

export async function performInstanceAction(id: string, action: InstanceAction): Promise<void> {
  switch (action) {
    case 'start':
      return startInstance(id);
    case 'stop':
      return stopInstance(id);
    case 'kill':
      return killInstance(id);
    case 'restart':
      return restartInstance(id);
  }
}
