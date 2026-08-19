import { AbstractFeature } from '@modrinth/api-client'
import type { RequestContext } from '@modrinth/api-client'
import type { PlatformAdapter } from './adapter'
import type { ConvexApiClient } from './convex-api'

type CompatTransport = Pick<ConvexApiClient, 'rawMutation' | 'rawQuery'>

export interface AmberiteModrinthCompatFeatureOptions {
	adapter: PlatformAdapter
	client: CompatTransport
	/** Convex HTTP Actions origin, for example https://example.convex.site. */
	sharedClientsSiteUrl?: string
	social?: boolean
	sharedClients?: boolean
}

/**
 * Routes only identity/social/client-sharing calls to Amberite. Modrinth content
 * calls continue through the normal generated API client unchanged.
 */
export class AmberiteModrinthCompatFeature extends AbstractFeature {
	constructor(private readonly options: AmberiteModrinthCompatFeatureOptions) {
		super({ name: 'amberite-modrinth-compat' })
	}

	override shouldApply(context: RequestContext): boolean {
		if (context.options.api === 'sharedinstances') return this.options.sharedClients !== false
		if (context.options.api !== 'labrinth') return false
		return this.options.social !== false && isSocialPath(context.path, context.options.version)
	}

	override async execute<T>(_next: () => Promise<T>, context: RequestContext): Promise<T> {
		if (context.options.api === 'sharedinstances') return await this.sharedClientRequest<T>(context)
		return (await this.socialRequest(context)) as T
	}

	private async socialRequest(context: RequestContext): Promise<unknown> {
		const method = context.options.method ?? 'GET'
		const path = context.path
		const url = new URL(context.url)

		if (context.options.version === 3) {
			if (method === 'GET' && path === '/user')
				return await this.options.client.rawQuery('accountsCompat:currentUser')
			if (method === 'GET' && path === '/users/search')
				return await this.options.client.rawQuery('accountsCompat:searchUsers', {
					query: url.searchParams.get('query') ?? '',
				})
			if (method === 'GET' && path.startsWith('/user/')) {
				const user = await this.options.client.rawQuery('accountsCompat:getUser', {
					idOrUsername: pathPart(path, 2),
				})
				if (!user) throw new Error('user not found')
				return user
			}
			if (method === 'GET' && path === '/friends')
				return await this.options.client.rawQuery('socialCompat:listFriends')
			if (path.startsWith('/friend/')) {
				const args = { idOrUsername: pathPart(path, 2) }
				if (method === 'POST')
					return await this.options.client.rawMutation('socialCompat:addFriend', args)
				if (method === 'DELETE')
					return await this.options.client.rawMutation('socialCompat:removeFriend', args)
			}
			if (method === 'GET' && path === '/blocks')
				return await this.options.client.rawQuery('socialCompat:listBlocks')
			if (path.startsWith('/block/')) {
				const args = { idOrUsername: pathPart(path, 2) }
				if (method === 'POST')
					return await this.options.client.rawMutation('socialCompat:blockUser', args)
				if (method === 'DELETE')
					return await this.options.client.rawMutation('socialCompat:unblockUser', args)
			}
		}

		if (context.options.version === 2) {
			if (method === 'GET' && /^\/user\/[^/]+\/notifications$/.test(path))
				return await this.options.client.rawQuery('socialCompat:listNotifications', {
					userId: pathPart(path, 2),
				})
			if (method === 'GET' && /^\/user\/[^/]+$/.test(path)) {
				const user = await this.options.client.rawQuery('accountsCompat:getUser', {
					idOrUsername: pathPart(path, 2),
				})
				if (!user) throw new Error('user not found')
				return user
			}
			if (method === 'GET' && path === '/users')
				return await this.options.client.rawQuery('accountsCompat:getUsers', {
					ids: jsonStringArray(url.searchParams.get('ids') ?? context.options.params?.ids),
				})
			if (path === '/notifications') {
				const ids = jsonStringArray(url.searchParams.get('ids') ?? context.options.params?.ids)
				if (method === 'GET')
					return await this.options.client.rawQuery('socialCompat:getNotifications', { ids })
				if (method === 'PATCH')
					return await this.options.client.rawMutation('socialCompat:markNotificationsRead', {
						ids,
					})
				if (method === 'DELETE')
					return await this.options.client.rawMutation('socialCompat:dismissNotifications', { ids })
			}
			if (path.startsWith('/notification/')) {
				const ids = [pathPart(path, 2)]
				if (method === 'PATCH')
					return await this.options.client.rawMutation('socialCompat:markNotificationsRead', {
						ids,
					})
				if (method === 'DELETE')
					return await this.options.client.rawMutation('socialCompat:dismissNotifications', { ids })
			}
		}

		throw new Error(`unsupported Amberite social route: ${method} ${path}`)
	}

	private async sharedClientRequest<T>(context: RequestContext): Promise<T> {
		const siteUrl = (
			this.options.sharedClientsSiteUrl ?? this.options.adapter.convexSiteUrl
		)?.replace(/\/$/, '')
		if (!siteUrl) throw new Error('sharedClientsSiteUrl is required for Amberite client sharing')
		const source = new URL(context.url)
		const target = `${siteUrl}/v1${context.path}${source.search}`
		const headers = new Headers(context.options.headers)
		const token = await this.options.adapter.getCurrentJwt()
		if (token) headers.set('authorization', `Bearer ${token}`)
		const body = requestBody(context.options.body, headers)
		const response = await this.options.adapter.fetchFn(target, {
			method: context.options.method ?? 'GET',
			headers,
			body,
			signal: context.options.signal,
		})
		if (!response.ok)
			throw new Error(
				(await response.text()) || `client sharing request failed (${response.status})`,
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
			path.startsWith('/block/')
		)
	if (version === 2)
		return (
			path === '/users' ||
			/^\/user\/[^/]+$/.test(path) ||
			path === '/notifications' ||
			path.startsWith('/notification/') ||
			/^\/user\/[^/]+\/notifications$/.test(path)
		)
	return false
}

function pathPart(path: string, index: number) {
	const value = path.split('/')[index]
	if (!value) throw new Error('invalid social route')
	return decodeURIComponent(value)
}

function jsonStringArray(value: unknown) {
	if (Array.isArray(value) && value.every((entry): entry is string => typeof entry === 'string'))
		return value
	if (typeof value !== 'string') return []
	const parsed: unknown = JSON.parse(value)
	if (
		!Array.isArray(parsed) ||
		!parsed.every((entry): entry is string => typeof entry === 'string')
	)
		throw new Error('ids must contain strings')
	return parsed
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
