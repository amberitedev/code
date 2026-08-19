import {
	type AmberiteSessionTokens,
	authErrorFromNative,
	type NativeAuthOperation,
	type PlatformAdapter,
} from '@amberite/amberite-api'
import type { CoreClientAdapter } from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'

import { config } from '@/config'
import { getConnectedCore } from '@/core/connected-core'

let accessToken: string | null = null
let amberiteUserId: string | null = null
let developmentSession: AmberiteSessionTokens | null = null

export function createDesktopCoreAdapter(): CoreClientAdapter & PlatformAdapter {
	return createDesktopAdapter()
}

export function createDesktopAdapter(): CoreClientAdapter & PlatformAdapter {
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
		convexSiteUrl: config.convexSiteUrl,
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
			await syncNativeSharedClientsSession(token)
		},
		async setCurrentAmberiteUserId(userId) {
			amberiteUserId = userId
			await syncNativeSharedClientsSession(accessToken)
		},
		amberiteSessionStorage: {
			async read() {
				return developmentSession
			},
			async write(tokens) {
				developmentSession = tokens
				accessToken = tokens.token
				await syncNativeSharedClientsSession(tokens.token)
			},
			async clear() {
				developmentSession = null
				accessToken = null
				amberiteUserId = null
				await syncNativeSharedClientsSession(null)
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
			await syncNativeSharedClientsSession(session.accessToken)
			return session
		},
		async restoreAmberiteSession() {
			const session = await invokeNativeAuth<{ accessToken: string } | null>(
				'plugin:auth|restore_amberite_product_session',
				{ convexUrl: config.convexUrl },
				'restore',
			)
			accessToken = session?.accessToken ?? null
			await syncNativeSharedClientsSession(accessToken)
			return session
		},
		async refreshAmberiteSession() {
			const session = await invokeNativeAuth<{ accessToken: string } | null>(
				'plugin:auth|refresh_amberite_product_session',
				{ convexUrl: config.convexUrl },
				'refresh',
			)
			accessToken = session?.accessToken ?? null
			await syncNativeSharedClientsSession(accessToken)
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
				amberiteUserId = null
				await syncNativeSharedClientsSession(null)
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

async function syncNativeSharedClientsSession(token: string | null): Promise<void> {
	if (!config.convexSiteUrl)
		throw new Error('VITE_CONVEX_SITE_URL must be configured for Amberite client sharing.')
	await invoke('plugin:auth|set_amberite_shared_clients_session', {
		convexSiteUrl: config.convexSiteUrl,
		accessToken: token,
		userId: amberiteUserId,
	})
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
