export interface SealedMinecraftAuthFlow {
	state: string
	verifier: string
	intent: 'continue' | 'use_another_account'
	expectedMinecraftUuid?: string
	redirect: string
	expiresAt: number
}

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

export function normalizeAuthUuid(value: unknown): string | undefined {
	if (typeof value !== 'string' || !value.trim()) return undefined
	const compact = value.trim().toLowerCase().replaceAll('-', '')
	if (!/^[0-9a-f]{32}$/.test(compact)) throw new Error('invalid Minecraft UUID')
	const normalized = [
		compact.slice(0, 8),
		compact.slice(8, 12),
		compact.slice(12, 16),
		compact.slice(16, 20),
		compact.slice(20),
	].join('-')
	if (!UUID_PATTERN.test(normalized)) throw new Error('invalid Minecraft UUID')
	return normalized
}

export function normalizeAuthRedirect(value: unknown): string {
	if (typeof value !== 'string') return '/dashboard'
	try {
		const base = new URL('https://amberite.local')
		const redirect = new URL(value, base)
		return redirect.origin === base.origin
			? `${redirect.pathname}${redirect.search}${redirect.hash}`
			: '/dashboard'
	} catch {
		return '/dashboard'
	}
}

export function requestIsSameOrigin(args: {
	expectedOrigin: string
	origin?: string
	referer?: string
	fetchSite?: string
}): boolean {
	if (args.fetchSite && args.fetchSite !== 'same-origin') return false
	if (args.origin) return args.origin === args.expectedOrigin
	if (!args.referer) return false
	try {
		return new URL(args.referer).origin === args.expectedOrigin
	} catch {
		return false
	}
}

export async function sealAuthFlow(flow: SealedMinecraftAuthFlow, secret: string): Promise<string> {
	const payload = base64Url(new TextEncoder().encode(JSON.stringify(flow)))
	const signature = await crypto.subtle.sign(
		'HMAC',
		await hmacKey(secret),
		new TextEncoder().encode(payload),
	)
	return `${payload}.${base64Url(new Uint8Array(signature))}`
}

export async function unsealAuthFlow(
	value: string,
	secret: string,
	now = Date.now(),
): Promise<SealedMinecraftAuthFlow> {
	const [payload, signature, extra] = value.split('.')
	if (!payload || !signature || extra) throw new Error('minecraft_auth_state_invalid')
	try {
		const valid = await crypto.subtle.verify(
			'HMAC',
			await hmacKey(secret),
			base64UrlDecode(signature).buffer as ArrayBuffer,
			new TextEncoder().encode(payload),
		)
		if (!valid) throw new Error('minecraft_auth_state_invalid')
		const flow: unknown = JSON.parse(new TextDecoder().decode(base64UrlDecode(payload)))
		if (!isSealedAuthFlow(flow)) throw new Error('minecraft_auth_state_invalid')
		if (flow.expiresAt <= now) throw new Error('minecraft_auth_state_expired')
		return flow
	} catch (error) {
		if (error instanceof Error && error.message === 'minecraft_auth_state_expired') throw error
		throw new Error('minecraft_auth_state_invalid')
	}
}

function isSealedAuthFlow(value: unknown): value is SealedMinecraftAuthFlow {
	if (!value || typeof value !== 'object') return false
	const flow = value as Record<string, unknown>
	if (
		typeof flow.state !== 'string' ||
		!flow.state ||
		typeof flow.verifier !== 'string' ||
		!flow.verifier ||
		(flow.intent !== 'continue' && flow.intent !== 'use_another_account') ||
		typeof flow.redirect !== 'string' ||
		normalizeAuthRedirect(flow.redirect) !== flow.redirect ||
		typeof flow.expiresAt !== 'number' ||
		!Number.isFinite(flow.expiresAt)
	) {
		return false
	}
	if (flow.expectedMinecraftUuid === undefined) return true
	if (flow.intent === 'use_another_account') return false
	try {
		return normalizeAuthUuid(flow.expectedMinecraftUuid) === flow.expectedMinecraftUuid
	} catch {
		return false
	}
}

export function randomAuthValue(byteLength: number): string {
	return base64Url(crypto.getRandomValues(new Uint8Array(byteLength)))
}

export async function createPkceChallenge(verifier: string): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier))
	return base64Url(new Uint8Array(digest))
}

async function hmacKey(secret: string): Promise<CryptoKey> {
	if (secret.length < 32) throw new Error('auth cookie secret is too short')
	return await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(secret),
		{ name: 'HMAC', hash: 'SHA-256' },
		false,
		['sign', 'verify'],
	)
}

function base64Url(bytes: Uint8Array): string {
	let value = ''
	for (const byte of bytes) value += String.fromCharCode(byte)
	return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}

function base64UrlDecode(value: string): Uint8Array {
	const padded = value
		.replace(/-/g, '+')
		.replace(/_/g, '/')
		.padEnd(Math.ceil(value.length / 4) * 4, '=')
	return Uint8Array.from(atob(padded), (character) => character.charCodeAt(0))
}
