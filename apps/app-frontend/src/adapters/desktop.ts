/**
 * DesktopAdapter — PlatformAdapter implementation for the Amberite desktop app (Tauri).
 *
 * Uses @tauri-apps/plugin-http fetch (routes through Rust) instead of window.fetch,
 * so requests to Core bypass browser CSP entirely. The http plugin allowlist in
 * capabilities/plugins.json permits the HTTP origins of linked Cores.
 *
 * Core URL: read from the identity-bound connected Core record.
 * JWT: persisted in the OS keychain through the Tauri auth plugin.
 */
import type { PersistentQueueStore, PlatformAdapter, QueuedMessage } from '@amberite/amberite-api'
import { invoke } from '@tauri-apps/api/core'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'

import { config } from '@/config'
import { getConnectedCore } from '@/core/connected-core'

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

export function createDesktopAdapter(): PlatformAdapter {
	return {
		// Tauri HTTP plugin fetch — routes through Rust, bypasses browser CSP.
		fetchFn: tauriFetch as typeof fetch,

		// Convex relay URL. Empty string disables Convex-relay transport silently.
		convexUrl: config.convexUrl,

		queueStore,

		// A Core is only reachable after pairing or restoring an identity-bound link.
		async getCoreUrl(): Promise<string | null> {
			return getConnectedCore()?.url ?? null
		},

		async getConnectedCoreId(): Promise<string | null> {
			return getConnectedCore()?.coreId ?? null
		},

		async getCurrentJwt(): Promise<string | null> {
			return await invoke<string | null>('plugin:auth|get_amberite_session_jwt')
		},

		async setCurrentJwt(jwt: string | null): Promise<void> {
			await invoke('plugin:auth|set_amberite_session_jwt', { jwt })
		},

		async getCurrentRefreshToken(): Promise<string | null> {
			return await invoke<string | null>('plugin:auth|get_amberite_session_refresh_token')
		},

		async setCurrentRefreshToken(refreshToken: string | null): Promise<void> {
			await invoke('plugin:auth|set_amberite_session_refresh_token', { refreshToken })
		},

		// Opens external URLs (OAuth, docs, etc.) in the system browser.
		openExternalAuth(url: string): void {
			openUrl(url).catch((e) => console.error('[DesktopAdapter] openExternalAuth failed:', e))
		},
	}
}
