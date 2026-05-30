/**
 * DesktopAdapter — PlatformAdapter implementation for the Amberite desktop app (Tauri).
 *
 * Uses native window.fetch instead of the Tauri HTTP plugin; Core's dev-mode CORS allows
 * any origin. For production deployments ALLOWED_ORIGIN in Core must be set to the Tauri
 * webview origin (tauri://localhost or similar).
 *
 * Core URL: reads VITE_CORE_URL env var, falls back to http://localhost:16662.
 * JWT: returns null — Core dev mode bypasses auth. TODO: wire up real auth here.
 */
import type { PersistentQueueStore, PlatformAdapter, QueuedMessage } from '@amberite/amberite-api'
import { open } from '@tauri-apps/plugin-opener'

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
		// Native browser fetch — no Tauri HTTP plugin needed.
		fetchFn: window.fetch.bind(window),

		// Convex relay URL. Empty string disables Convex-relay transport silently.
		convexUrl: (import.meta.env.VITE_CONVEX_URL as string | undefined) ?? '',

		queueStore,

		// Returns the Core HTTP base URL. Falls back to local dev default.
		async getCoreUrl(): Promise<string | null> {
			return (import.meta.env.VITE_CORE_URL as string | undefined) ?? 'http://localhost:16662'
		},

		// TODO: return real JWT when auth is implemented.
		async getCurrentJwt(): Promise<string | null> {
			return null
		},

		// Opens external URLs (OAuth, docs, etc.) in the system browser.
		openExternalAuth(url: string): void {
			open(url).catch((e) => console.error('[DesktopAdapter] openExternalAuth failed:', e))
		},
	}
}
