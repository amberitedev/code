import {
	type AmberiteSessionTokens,
	AuthError,
	NetworkError,
	type PersistentQueueStore,
	type PlatformAdapter,
	ProviderAuthError,
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
		async readAmberiteSession() {
			return developmentSession
		},
		async writeAmberiteSession(tokens) {
			developmentSession = tokens
			accessToken = tokens.token
		},
		async clearAmberiteSession() {
			developmentSession = null
			accessToken = null
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
		async restoreMinecraftSession() {
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
		async signOutMinecraftSession() {
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

type NativeAuthOperation = 'sign_in' | 'restore' | 'refresh' | 'sign_out'

async function invokeNativeAuth<T>(
	command: string,
	args: Record<string, unknown>,
	operation: NativeAuthOperation,
): Promise<T> {
	try {
		return await invoke<T>(command, args)
	} catch (error) {
		throw mapNativeAuthError(error, operation)
	}
}

function mapNativeAuthError(error: unknown, operation: NativeAuthOperation): Error {
	const message = nativeErrorMessage(error)
	const normalized = message.toLowerCase()
	if (normalized.includes('cancel')) return new ProviderAuthError(message, 'cancelled')
	if (normalized.includes('state')) return new ProviderAuthError(message, 'state_failure')
	if (normalized.includes('uuid mismatch')) return new AuthError(message, 'identity_mismatch')
	if (normalized.includes('java') && normalized.includes('profile'))
		return new ProviderAuthError(message, 'java_profile_missing')
	if (normalized.includes('xbox')) return new ProviderAuthError(message, 'xbox_restriction')
	if (normalized.includes('throttl') || normalized.includes('429'))
		return new ProviderAuthError(message, 'throttled')
	if (normalized.includes('configur') || normalized.includes('client id'))
		return new ProviderAuthError(message, 'configuration_failure')
	if (
		normalized.includes('network') ||
		normalized.includes('connect') ||
		normalized.includes('timeout') ||
		normalized.includes('offline') ||
		normalized.includes('unreachable') ||
		normalized.includes('error sending request')
	)
		return new NetworkError(message, 'amberite_unreachable')
	if (
		normalized.includes('corrupt') ||
		normalized.includes('decrypt') ||
		normalized.includes('keyring') ||
		normalized.includes('missing key') ||
		normalized.includes('incomplete') ||
		normalized.includes('bundle authentication')
	)
		return new AuthError(message, 'corrupt_session')
	if (normalized.includes('refresh') && normalized.includes('reuse'))
		return new AuthError(message, 'refresh_reuse')
	if (normalized.includes('expired')) return new AuthError(message, 'expired_session')
	if (normalized.includes('revoked')) return new AuthError(message, 'revoked_session')
	if (
		normalized.includes('not authenticated') ||
		normalized.includes('invalid session') ||
		normalized.includes('invalid refresh') ||
		normalized.includes('401') ||
		normalized.includes('403')
	)
		return new AuthError(message, 'invalid_session')
	if (operation === 'sign_in') return new ProviderAuthError(message, 'provider_failure')
	if (operation === 'restore') return new AuthError(message, 'corrupt_session')
	return new NetworkError(message, 'amberite_unreachable')
}

function nativeErrorMessage(error: unknown): string {
	if (error instanceof Error) return error.message
	if (typeof error === 'string') return error
	if (error && typeof error === 'object' && 'message' in error) {
		const message = (error as { message?: unknown }).message
		if (typeof message === 'string') return message
	}
	return String(error)
}
