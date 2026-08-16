import {
	type AmberiteSessionTokens,
	authErrorFromNative,
	type NativeAuthOperation,
	type PersistentQueueStore,
	type PlatformAdapter,
	type QueuedMessage,
} from '@amberite/amberite-api'
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
let accessToken: string | null = null
let developmentSession: AmberiteSessionTokens | null = null

export function createDesktopAdapter(): PlatformAdapter {
	const useBrowserFetchForConvex =
		config.convexUrl.startsWith('http://localhost:') ||
		config.convexUrl.startsWith('http://127.0.0.1:')
	const convexOrigin = new URL(config.convexUrl).origin
	const fetchFn: typeof fetch = async (input, init) => {
		const url = input instanceof Request ? input.url : input.toString()
		if (useBrowserFetchForConvex && new URL(url, window.location.href).origin === convexOrigin) {
			return await window.fetch(input, init)
		}
		return await tauriFetch(input, init)
	}

	return {
		fetchFn,
		convexUrl: config.convexUrl,
		queueStore,
		async getCoreUrl() {
			return getConnectedCore()?.url ?? null
		},
		async getConnectedCoreId() {
			return getConnectedCore()?.coreId ?? null
		},
		async getCurrentJwt() {
			return accessToken
		},
		async setCurrentJwt(token) {
			accessToken = token
		},
		amberiteSessionStorage: {
			async read() {
				return developmentSession
			},
			async write(tokens) {
				developmentSession = tokens
				accessToken = tokens.token
			},
			async clear() {
				developmentSession = null
				accessToken = null
			},
		},
		async signInWithMinecraft(request) {
			const session = await invokeNativeAuth<{ accessToken: string }>(
				'plugin:auth|amberite_product_sign_in',
				{
					convexUrl: config.convexUrl,
					mode: request.mode,
					expectedMinecraftUuid: request.expectedMinecraftUuid ?? null,
				},
				'sign_in',
			)
			accessToken = session.accessToken
			return session
		},
		async restoreAmberiteSession() {
			const session = await invokeNativeAuth<{ accessToken: string } | null>(
				'plugin:auth|restore_amberite_product_session',
				{ convexUrl: config.convexUrl },
				'restore',
			)
			accessToken = session?.accessToken ?? null
			return session
		},
		async refreshAmberiteSession() {
			const session = await invokeNativeAuth<{ accessToken: string } | null>(
				'plugin:auth|refresh_amberite_product_session',
				{ convexUrl: config.convexUrl },
				'refresh',
			)
			accessToken = session?.accessToken ?? null
			return session
		},
		async signOutAmberiteSession() {
			try {
				await invokeNativeAuth(
					'plugin:auth|sign_out_amberite_product_session',
					{ convexUrl: config.convexUrl },
					'sign_out',
				)
			} finally {
				developmentSession = null
				accessToken = null
			}
		},
		async getLocalSetupSecret() {
			return await invoke<string | null>('plugin:auth|get_amberite_local_setup_secret')
		},
		openExternalAuth(url) {
			openUrl(url).catch((error) =>
				console.error('[DesktopAdapter] openExternalAuth failed:', error),
			)
		},
	}
}

async function invokeNativeAuth<T>(
	command: string,
	args: Record<string, unknown>,
	operation: NativeAuthOperation,
): Promise<T> {
	try {
		return await invoke<T>(command, args)
	} catch (error) {
		throw authErrorFromNative(error, operation)
	}
}
