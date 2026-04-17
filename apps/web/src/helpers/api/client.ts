import { ofetch } from 'ofetch';
import { getApiKeyHeader } from '@/helpers/auth/key-store';
import type { ApiError } from './types';

const STORAGE_KEY_CORE_URL = 'LODESTONE_CORE_URL';
const DEFAULT_CORE_URL = 'http://localhost:16662';

export function getCoreUrl(): string {
  return localStorage.getItem(STORAGE_KEY_CORE_URL) || DEFAULT_CORE_URL;
}

export function setCoreUrl(url: string): void {
  localStorage.setItem(STORAGE_KEY_CORE_URL, url);
}

function isApiError(error: unknown): error is ApiError {
  return typeof error === 'object' && error !== null && 'code' in error;
}

export const apiClient = ofetch.create({
  baseURL: getCoreUrl(),
  headers: {
    'Content-Type': 'application/json',
  },
  onRequest({ options }) {
    const apiKey = getApiKeyHeader();
    if (apiKey) {
      options.headers.set('Authorization', `Bearer ${apiKey}`);
    }
  },
  onResponseError({ response, error }) {
    if (response.status === 401) {
      window.dispatchEvent(new CustomEvent('lodestone-auth-failed', {
        detail: isApiError(error) ? error : { code: 'UNAUTHORIZED', message: 'Invalid or expired API key' }
      }));
    } else if (response.status === 0 || response.status === 502 || response.status === 503) {
      window.dispatchEvent(new CustomEvent('lodestone-connection-lost'));
    }
    return error;
  },
});

export function createApiClient(baseUrl: string) {
  return ofetch.create({
    baseURL: baseUrl,
    headers: {
      'Content-Type': 'application/json',
    },
    onRequest({ options }) {
      const apiKey = getApiKeyHeader();
      if (apiKey) {
        options.headers.set('Authorization', `Bearer ${apiKey}`);
      }
    },
  });
}

export async function healthCheck(url?: string): Promise<boolean> {
  try {
    const targetUrl = url || getCoreUrl();
    const response = await ofetch(`${targetUrl}/health`, {
      method: 'GET',
      headers: { 'Content-Type': 'application/json' },
    });
    return response.status === 'ok';
  } catch {
    return false;
  }
}
