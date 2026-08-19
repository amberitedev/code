import {
	authErrorFromNative,
	type AmberitePlatformAdapter,
	type AmberiteSessionTokens,
	type CoreClientAdapter,
	type NativeAuthOperation,
} from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'

import { config } from '@/config'
import { getConnectedCore } from '@/core/connected-core'

let accessToken: string | null = null
let userId: string | null = null
let developmentSession: AmberiteSessionTokens | null = null

export type DesktopAdapter = AmberitePlatformAdapter & CoreClientAdapter

export function createDesktopAdapter(): DesktopAdapter {
	const convexOrigin = new URL(config.convexUrl).origin
	const localConvex = ['localhost', '127.0.0.1'].includes(new URL(config.convexUrl).hostname)
	const fetchFn: typeof fetch = async (input, init) => {
		const url = input instanceof Request ? input.url : input.toString()
		if (localConvex && new URL(url, window.location.href).origin === convexOrigin)
			return await window.fetch(input, init)
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
			await syncSharedClientsSession()
		},
		async setCurrentAmberiteUserId(nextUserId) {
			userId = nextUserId
			await syncSharedClientsSession()
		},
		amberiteSessionStorage: {
			async read() {
				return developmentSession
			},
			async write(tokens) {
				developmentSession = tokens
				accessToken = tokens.token
				await syncSharedClientsSession()
			},
			async clear() {
				developmentSession = null
				accessToken = null
				userId = null
				await syncSharedClientsSession()
			},
		},
		async signInWithMinecraft(request) {
			const session = await nativeAuth<{ accessToken: string; expiresAt?: string; user?: unknown }>(
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
			const session = await nativeAuth<{
				accessToken: string
				expiresAt?: string
				user?: unknown
			} | null>(
				'plugin:auth|restore_amberite_product_session',
				{ convexUrl: config.convexUrl },
				'restore',
			)
			accessToken = session?.accessToken ?? null
			return session
		},
		async refreshAmberiteSession() {
			const session = await nativeAuth<{
				accessToken: string
				expiresAt?: string
				user?: unknown
			} | null>(
				'plugin:auth|refresh_amberite_product_session',
				{ convexUrl: config.convexUrl },
				'refresh',
			)
			accessToken = session?.accessToken ?? null
			return session
		},
		async signOutAmberiteSession() {
			try {
				await nativeAuth(
					'plugin:auth|sign_out_amberite_product_session',
					{ convexUrl: config.convexUrl },
					'sign_out',
				)
			} finally {
				developmentSession = null
				accessToken = null
				userId = null
				await syncSharedClientsSession()
			}
		},
		async getLocalSetupSecret() {
			return await invoke<string | null>('plugin:auth|get_amberite_local_setup_secret')
		},
	}
}

async function syncSharedClientsSession() {
	if (!config.convexSiteUrl) throw new Error('VITE_CONVEX_SITE_URL is not configured.')
	await invoke('plugin:auth|set_amberite_shared_clients_session', {
		convexSiteUrl: config.convexSiteUrl,
		accessToken,
		userId,
	})
}

async function nativeAuth<T>(
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
