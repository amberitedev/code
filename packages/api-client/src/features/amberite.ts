import { AbstractFeature, type FeatureConfig } from '../core/abstract-feature'
import type { RequestContext } from '../types/request'

export interface AmberiteTransport {
	query<T = unknown>(path: string, args?: unknown): Promise<T>
	mutation<T = unknown>(path: string, args?: unknown): Promise<T>
	action?<T = unknown>(path: string, args?: unknown): Promise<T>
}

export interface AmberiteFeatureConfig extends FeatureConfig {
	transport: AmberiteTransport
	getAccessToken(): Promise<string | null>
	fetch: typeof fetch
	/** Convex HTTP Actions origin, for example https://example.convex.site. */
	siteUrl?: string
	/** Platform-owned refresh; refresh tokens never enter the WebView API client. */
	refreshSession?(): Promise<unknown>
	nativeAuth?: {
		signIn(request: {
			mode: 'continue' | 'use_another_account'
			expectedMinecraftUuid?: string
		}): Promise<unknown>
		restore(): Promise<unknown>
		signOut(): Promise<void>
	}
	social?: boolean
	sharedInstances?: boolean
}

/**
 * Keeps Modrinth content on Labrinth while routing only Amberite identity,
 * social, device-session, Core-pairing, and shared-client calls to Convex.
 */
export class AmberiteFeature extends AbstractFeature {
	declare protected config: AmberiteFeatureConfig

	constructor(config: AmberiteFeatureConfig) {
		super({ ...config, name: config.name ?? 'amberite' })
	}

	override shouldApply(context: RequestContext): boolean {
		if (context.options.api === 'amberite') return true
		if (context.options.api === 'sharedinstances') return this.config.sharedInstances !== false
		if (context.options.api !== 'labrinth' || this.config.social === false) return false
		return isSocialPath(context.path, context.options.version)
	}

	override async execute<T>(_next: () => Promise<T>, context: RequestContext): Promise<T> {
		if (context.options.api === 'sharedinstances')
			return await this.sharedInstanceRequest<T>(context)
		if (context.options.api === 'amberite') return (await this.amberiteRequest(context)) as T
		return (await this.socialRequest(context)) as T
	}

	private async socialRequest(context: RequestContext): Promise<unknown> {
		const method = context.options.method ?? 'GET'
		const path = context.path
		const version = context.options.version

		if (version === 3) {
			if (method === 'GET' && path === '/user')
				return await this.config.transport.query('accountsCompat:currentUser', {})
			if (method === 'GET' && path === '/users/search')
				return await this.config.transport.query('accountsCompat:searchUsers', {
					query: stringParam(context, 'query'),
				})
			if (method === 'GET' && path === '/friends')
				return await this.config.transport.query('socialCompat:listFriends', {})
			if (path.startsWith('/friend/')) {
				const args = { idOrUsername: pathPart(path, 2) }
				if (method === 'POST')
					return await this.config.transport.mutation('socialCompat:addFriend', args)
				if (method === 'DELETE')
					return await this.config.transport.mutation('socialCompat:removeFriend', args)
			}
			if (method === 'GET' && path === '/blocks')
				return await this.config.transport.query('socialCompat:listBlocks', {})
			if (path.startsWith('/block/')) {
				const args = { idOrUsername: pathPart(path, 2) }
				if (method === 'POST')
					return await this.config.transport.mutation('socialCompat:blockUser', args)
				if (method === 'DELETE')
					return await this.config.transport.mutation('socialCompat:unblockUser', args)
			}
			if (isNotificationsPath(path)) return await this.notificationsRequest(context)
			if (path.startsWith('/user/')) return await this.userRequest(context)
		}

		if (version === 2) {
			if (path.startsWith('/session/')) return await this.sessionsRequest(context)
			if (isNotificationsPath(path)) return await this.notificationsRequest(context)
			if (method === 'GET' && /^\/user\/[^/]+$/.test(path))
				return await this.getUser(pathPart(path, 2))
			if (method === 'GET' && path === '/users')
				return await this.config.transport.query('accountsCompat:getUsers', {
					ids: stringArrayParam(context, 'ids'),
				})
		}

		throw new Error(`unsupported Amberite social route: ${method} ${path}`)
	}

	private async userRequest(context: RequestContext): Promise<unknown> {
		const method = context.options.method ?? 'GET'
		const path = context.path
		const idOrUsername = pathPart(path, 2)
		if (method === 'GET' && /^\/user\/[^/]+$/.test(path)) return await this.getUser(idOrUsername)
		await this.requireCurrentUser(idOrUsername)
		if (method === 'PATCH' && /^\/user\/[^/]+$/.test(path)) {
			const body = recordBody(context.options.body)
			return await this.config.transport.mutation('profiles:updateCurrent', {
				...(typeof body.display_name === 'string' ? { displayName: body.display_name } : {}),
				...(typeof body.bio === 'string' || body.bio === null ? { bio: body.bio } : {}),
				...(typeof body.allow_friend_requests === 'boolean'
					? { allowFriendRequests: body.allow_friend_requests }
					: {}),
			})
		}
		if (path.endsWith('/icon') && method === 'DELETE')
			return await this.config.transport.mutation('profiles:updateCurrent', { avatar: null })
		if (path.endsWith('/icon') && method === 'PATCH') {
			const body = context.options.body
			if (!(body instanceof Blob)) throw new Error('avatar upload requires a Blob')
			if (body.size > 262_144) throw new Error('avatar must be 256 KiB or smaller')
			return await this.config.transport.mutation('profiles:updateCurrent', {
				avatar: { url: await blobDataUrl(body), mimeType: body.type, sizeBytes: body.size },
			})
		}
		if (method === 'DELETE' && /^\/user\/[^/]+$/.test(path))
			return await this.config.transport.mutation('auth:deleteCurrentAccount', {})
		throw new Error(`unsupported Amberite user route: ${method} ${path}`)
	}

	private async notificationsRequest(context: RequestContext): Promise<unknown> {
		const method = context.options.method ?? 'GET'
		const path = context.path
		if (method === 'GET' && /^\/user\/[^/]+\/notifications$/.test(path))
			return await this.config.transport.query('socialCompat:listNotifications', {
				userId: pathPart(path, 2),
			})
		if (path === '/notifications') {
			const ids = stringArrayParam(context, 'ids')
			if (method === 'GET')
				return await this.config.transport.query('socialCompat:getNotifications', { ids })
			if (method === 'PATCH')
				return await this.config.transport.mutation('socialCompat:markNotificationsRead', { ids })
			if (method === 'DELETE')
				return await this.config.transport.mutation('socialCompat:dismissNotifications', { ids })
		}
		if (path.startsWith('/notification/')) {
			const ids = [pathPart(path, 2)]
			if (method === 'GET') {
				const notifications = await this.config.transport.query<unknown[]>(
					'socialCompat:getNotifications',
					{ ids },
				)
				if (!notifications[0]) throw new Error('notification not found')
				return notifications[0]
			}
			if (method === 'PATCH')
				return await this.config.transport.mutation('socialCompat:markNotificationsRead', { ids })
			if (method === 'DELETE')
				return await this.config.transport.mutation('socialCompat:dismissNotifications', { ids })
		}
		throw new Error(`unsupported Amberite notification route: ${method} ${path}`)
	}

	private async sessionsRequest(context: RequestContext): Promise<unknown> {
		const method = context.options.method ?? 'GET'
		if (method === 'GET' && context.path === '/session/list')
			return await this.config.transport.query('sessions:list', { now: Date.now() })
		if (method === 'DELETE' && /^\/session\/[^/]+$/.test(context.path))
			return await this.config.transport.mutation('sessions:revoke', {
				id: pathPart(context.path, 2),
			})
		if (method === 'POST' && context.path === '/session/refresh') {
			if (!this.config.refreshSession) throw new Error('Amberite session refresh is not configured')
			await this.config.refreshSession()
			const sessions = await this.config.transport.query<Array<{ current: boolean }>>(
				'sessions:list',
				{
					now: Date.now(),
				},
			)
			const current = sessions.find((session) => session.current)
			if (!current) throw new Error('refreshed session was not registered')
			return current
		}
		throw new Error(`unsupported Amberite session route: ${method} ${context.path}`)
	}

	private async amberiteRequest(context: RequestContext): Promise<unknown> {
		const method = context.options.method ?? 'GET'
		if (context.path === '/auth/sign-in' && method === 'POST') {
			if (!this.config.nativeAuth) throw new Error('native Minecraft sign-in is not configured')
			const body = recordBody(context.options.body)
			const mode = body.mode
			if (mode !== 'continue' && mode !== 'use_another_account')
				throw new Error('invalid sign-in mode')
			return await this.config.nativeAuth.signIn({
				mode,
				...(typeof body.expectedMinecraftUuid === 'string'
					? { expectedMinecraftUuid: body.expectedMinecraftUuid }
					: {}),
			})
		}
		if (context.path === '/auth/session' && method === 'GET') {
			if (!this.config.nativeAuth) throw new Error('native session restoration is not configured')
			return await this.config.nativeAuth.restore()
		}
		if (context.path === '/auth/session' && method === 'DELETE') {
			if (!this.config.nativeAuth) throw new Error('native sign-out is not configured')
			return await this.config.nativeAuth.signOut()
		}
		if (context.path === '/account/modrinth' && method === 'GET')
			return await this.config.transport.query('modrinth:current', {})
		if (context.path === '/account/modrinth' && method === 'PUT') {
			if (!this.config.transport.action) throw new Error('Amberite actions are not configured')
			return await this.config.transport.action(
				'modrinth:storeCurrentOAuthTokens',
				recordBody(context.options.body),
			)
		}
		if (context.path === '/account/modrinth/refresh' && method === 'POST') {
			if (!this.config.transport.action) throw new Error('Amberite actions are not configured')
			return await this.config.transport.action('modrinth:refreshCurrentStatus', {})
		}
		if (context.path === '/account/modrinth' && method === 'DELETE')
			return await this.config.transport.mutation('modrinth:disconnectCurrent', {})
		if (method === 'POST' && context.path.startsWith('/friends/code/'))
			return await this.config.transport.mutation('friends:addByCode', {
				code: pathPart(context.path, 3),
			})
		if (method === 'PUT' && context.path === '/sessions/current')
			return await this.config.transport.mutation(
				'sessions:registerCurrent',
				recordBody(context.options.body),
			)
		if (method === 'GET' && context.path === '/cores')
			return await this.config.transport.query('coreList:listLinkedForCurrent', {})
		if (method === 'POST' && context.path === '/cores/pairing/claim')
			return await this.config.transport.mutation(
				'presence:claimPairingCore',
				recordBody(context.options.body),
			)
		if (method === 'POST' && context.path === '/cores/pairing/finalize')
			return await this.config.transport.mutation(
				'presence:finalizePairingCore',
				recordBody(context.options.body),
			)
		if (method === 'POST' && context.path === '/cores/pairing/release')
			return await this.config.transport.mutation(
				'presence:releasePairingCore',
				recordBody(context.options.body),
			)
		throw new Error(`unsupported Amberite route: ${method} ${context.path}`)
	}

	private async getUser(idOrUsername: string) {
		const user = await this.config.transport.query('accountsCompat:getUser', { idOrUsername })
		if (!user) throw new Error('user not found')
		return user
	}

	private async requireCurrentUser(idOrUsername: string) {
		const user = await this.config.transport.query<{ id: string; username: string }>(
			'accountsCompat:currentUser',
			{},
		)
		const normalized = idOrUsername.trim().replace(/^@/, '').replace(/-/g, '').toLowerCase()
		if (
			normalized !== user.id.replace(/-/g, '').toLowerCase() &&
			normalized !== user.username.toLowerCase()
		)
			throw new Error('you do not have permission to edit this user')
	}

	private async sharedInstanceRequest<T>(context: RequestContext): Promise<T> {
		const siteUrl = this.config.siteUrl?.replace(/\/$/, '')
		if (!siteUrl) throw new Error('Amberite siteUrl is required for shared clients')
		const source = new URL(context.url)
		const target = new URL(`${siteUrl}/v1${context.path}`)
		for (const [key, value] of source.searchParams) target.searchParams.append(key, value)
		for (const [key, value] of Object.entries(context.options.params ?? {}))
			if (value !== undefined) target.searchParams.set(key, String(value))
		const headers = new Headers(context.options.headers)
		const token = await this.config.getAccessToken()
		if (token) headers.set('authorization', `Bearer ${token}`)
		const response = await this.config.fetch(target.toString(), {
			method: context.options.method ?? 'GET',
			headers,
			body: requestBody(context.options.body, headers),
			signal: context.options.signal,
		})
		if (!response.ok)
			throw new Error(
				(await response.text()) || `shared client request failed (${response.status})`,
			)
		if (response.status === 204) return undefined as T
		return (await response.json()) as T
	}
}

function isSocialPath(path: string, version: number | string) {
	if (version === 3)
		return (
			path === '/user' ||
			path.startsWith('/user/') ||
			path === '/users/search' ||
			path === '/friends' ||
			path.startsWith('/friend/') ||
			path === '/blocks' ||
			path.startsWith('/block/') ||
			isNotificationsPath(path)
		)
	if (version === 2)
		return (
			path === '/users' ||
			path.startsWith('/user/') ||
			path.startsWith('/notification') ||
			path.startsWith('/session/')
		)
	return false
}

function isNotificationsPath(path: string) {
	return (
		path === '/notifications' ||
		path.startsWith('/notification/') ||
		/^\/user\/[^/]+\/notifications$/.test(path)
	)
}

function pathPart(path: string, index: number) {
	const value = path.split('/')[index]
	if (!value) throw new Error('invalid Amberite route')
	return decodeURIComponent(value)
}

function stringParam(context: RequestContext, key: string): string {
	const option = context.options.params?.[key]
	if (option !== undefined) return String(option)
	return new URL(context.url).searchParams.get(key) ?? ''
}

function stringArrayParam(context: RequestContext, key: string): string[] {
	const value = context.options.params?.[key] ?? new URL(context.url).searchParams.get(key)
	if (Array.isArray(value) && value.every((entry): entry is string => typeof entry === 'string'))
		return value
	if (typeof value !== 'string') return []
	const parsed: unknown = JSON.parse(value)
	if (
		!Array.isArray(parsed) ||
		!parsed.every((entry): entry is string => typeof entry === 'string')
	)
		throw new Error(`${key} must contain strings`)
	return parsed
}

function recordBody(value: unknown): Record<string, unknown> {
	if (typeof value !== 'object' || value === null || Array.isArray(value) || value instanceof Blob)
		throw new Error('request body must be an object')
	return value as Record<string, unknown>
}

function requestBody(body: unknown, headers: Headers): BodyInit | undefined {
	if (body === undefined || body === null) return undefined
	if (
		typeof body === 'string' ||
		body instanceof Blob ||
		body instanceof FormData ||
		body instanceof URLSearchParams ||
		body instanceof ArrayBuffer
	)
		return body
	headers.set('content-type', 'application/json')
	return JSON.stringify(body)
}

async function blobDataUrl(blob: Blob): Promise<string> {
	const bytes = new Uint8Array(await blob.arrayBuffer())
	let binary = ''
	for (let offset = 0; offset < bytes.length; offset += 0x8000)
		binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000))
	return `data:${blob.type || 'image/png'};base64,${btoa(binary)}`
}
