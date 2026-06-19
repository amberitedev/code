/**
 * DesktopAdapter — PlatformAdapter implementation for the Amberite desktop app (Tauri).
 *
 * Uses @tauri-apps/plugin-http fetch (routes through Rust) instead of window.fetch,
 * so requests to Core bypass browser CSP entirely. The http plugin allowlist in
 * capabilities/plugins.json must include http://localhost:16662/*.
 *
 * Core URL: reads VITE_CORE_URL env var, falls back to http://localhost:16662.
 * JWT: persisted by the Amberite account sign-in flow.
 */
import type { PersistentQueueStore, PlatformAdapter, QueuedMessage } from '@amberite/amberite-api'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'

import { config } from '@/config'

class LocalStorageQueueStore implements PersistentQueueStore {
	async list(queueName: string): Promise<QueuedMessage[]> {
		return this.read(queueName)
	}

	async push(queueName: string, message: QueuedMessage): Promise<void> {
		this.write(queueName, [...this.read(queueName), message])
	}

	async remove(queueName: string, id: string): Promise<void> {
		this.write(
			queueName,
			this.read(queueName).filter((message) => message.id !== id),
		)
	}

	private read(queueName: string): QueuedMessage[] {
		const raw = window.localStorage.getItem(this.key(queueName))
		if (!raw) return []
		try {
			const parsed = JSON.parse(raw)
			return Array.isArray(parsed) ? parsed : []
		} catch {
			return []
		}
	}

	private write(queueName: string, messages: QueuedMessage[]): void {
		window.localStorage.setItem(this.key(queueName), JSON.stringify(messages))
	}

	private key(queueName: string): string {
		return `amberite-api:queue:${queueName}`
	}
}

const queueStore = new LocalStorageQueueStore()

export const AMBERITE_AUTH_TOKEN_KEY = 'amberite:auth:jwt'
export const CORE_URL_STORAGE_KEY = 'amberite:core:url'

export function createDesktopAdapter(): PlatformAdapter {
	return {
		// Tauri HTTP plugin fetch — routes through Rust, bypasses browser CSP.
		fetchFn: tauriFetch as typeof fetch,

		// Convex relay URL. Empty string disables Convex-relay transport silently.
		convexUrl: config.convexUrl,

		queueStore,

		// Returns the Core HTTP base URL. Falls back to local dev default.
		async getCoreUrl(): Promise<string | null> {
			const envUrl = import.meta.env.VITE_CORE_URL as string | undefined
			if (envUrl) return envUrl
			try {
				return window.localStorage.getItem(CORE_URL_STORAGE_KEY) ?? 'http://localhost:16662'
			} catch {
				return 'http://localhost:16662'
			}
		},

		async getCurrentJwt(): Promise<string | null> {
			try {
				return window.localStorage.getItem(AMBERITE_AUTH_TOKEN_KEY)
			} catch {
				return null
			}
		},

		async setCurrentJwt(jwt: string | null): Promise<void> {
			try {
				if (jwt) window.localStorage.setItem(AMBERITE_AUTH_TOKEN_KEY, jwt)
				else window.localStorage.removeItem(AMBERITE_AUTH_TOKEN_KEY)
			} catch {
				// localStorage can be unavailable in tests.
			}
		},

		// Opens external URLs (OAuth, docs, etc.) in the system browser.
		openExternalAuth(url: string): void {
			openUrl(url).catch((e) => console.error('[DesktopAdapter] openExternalAuth failed:', e))
		},
	}
}
