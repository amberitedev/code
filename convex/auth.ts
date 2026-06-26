import { ConvexCredentials } from '@convex-dev/auth/providers/ConvexCredentials'
import { convexAuth, createAccount, getAuthUserId, retrieveAccount } from '@convex-dev/auth/server'
import { v } from 'convex/values'
import { internal } from './_generated/api'
import { internalMutation, internalQuery, mutation, query } from './_generated/server'
import {
	currentAccountFields,
	publicCurrentProfile,
	publicUser,
	requireUserId,
} from './_socialRules'

interface MinecraftProfile {
	id: string
	name: string
}

interface ModrinthProfile {
	id: string
	username: string
	name?: string
	avatar_url?: string | null
	bio?: string | null
	created?: string
	email?: string | null
	email_verified?: boolean
	role?: string
	badges?: number
}

const WEB_PASSWORD_PROVIDER_ID = 'web-password'
const MINECRAFT_TOKEN_PROVIDER_ID = 'minecraft-token'
const MODRINTH_TOKEN_PROVIDER_ID = 'modrinth-token'
const INVALID_ACCOUNT_ID = 'InvalidAccountId'
const INVALID_SECRET = 'InvalidSecret'
const TOO_MANY_FAILED_ATTEMPTS = 'TooManyFailedAttempts'
const DEV_PERSONA_ID_PATTERN = /^[a-z0-9_-]{1,32}$/
const USERNAME_PATTERN = /^[a-zA-Z0-9_]{3,24}$/
const PASSWORD_HASH_PREFIX = 'pbkdf2-sha256'
const PASSWORD_HASH_ITERATIONS = 100_000

function normalizeUuid(raw: string): string {
	const stripped = raw.replace(/-/g, '')
	if (stripped.length !== 32) return raw
	return [
		stripped.slice(0, 8),
		stripped.slice(8, 12),
		stripped.slice(12, 16),
		stripped.slice(16, 20),
		stripped.slice(20, 32),
	].join('-')
}

async function verifyMinecraftAccessToken(token: string): Promise<MinecraftProfile | null> {
	const response = await fetch('https://api.minecraftservices.com/minecraft/profile', {
		headers: { Authorization: `Bearer ${token}` },
	})
	if (!response.ok) return null
	return (await response.json()) as MinecraftProfile
}

async function verifyModrinthToken(token: string): Promise<ModrinthProfile | null> {
	const response = await fetch('https://api.modrinth.com/v3/user', {
		headers: {
			Authorization: token,
			'User-Agent': 'Amberite/0.2 (auth exchange)',
		},
	})
	if (!response.ok) return null
	return (await response.json()) as ModrinthProfile
}

function devPersonaId(credentials: Record<string, unknown>): string | null {
	const raw = credentials.devPersonaId
	if (raw === undefined || raw === null || raw === '') return null
	if (process.env.AMBERITE_DEV_MODE !== 'true') {
		throw new Error('devPersonaId is only accepted in Amberite dev mode')
	}
	if (typeof raw !== 'string' || !DEV_PERSONA_ID_PATTERN.test(raw)) {
		throw new Error('devPersonaId must be 1-32 lowercase letters, numbers, underscores, or hyphens')
	}
	return raw
}

function personaAccountId(minecraftUuid: string, personaId: string | null): string {
	const baseAccountId = `minecraft:${minecraftUuid}`
	return personaId ? `${baseAccountId}:dev:${personaId}` : baseAccountId
}

function personaProfile(gamertag: string, accountId: string, personaId: string | null) {
	if (!personaId) {
		return {
			amberiteUserId: accountId,
			friendCode: createFriendCode(),
			displayName: gamertag,
			username: gamertag,
			normalizedUsername: gamertag.toLowerCase(),
			onboardedAt: Date.now(),
		}
	}

	const label = personaId
		.split(/[-_]/g)
		.filter(Boolean)
		.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
		.join(' ')
	const username = personaId.replace(/-/g, '_')
	return {
		amberiteUserId: accountId,
		friendCode: createFriendCode(),
		displayName: label || gamertag,
		username,
		normalizedUsername: username.toLowerCase(),
		onboardedAt: Date.now(),
	}
}

function createFriendCode(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(5))
	return `AMB-${Array.from(bytes, (byte) => byte.toString(36).padStart(2, '0'))
		.join('')
		.slice(0, 8)
		.toUpperCase()}`
}

export const { auth, signIn, signOut, store, isAuthenticated } = convexAuth({
	providers: [
		ConvexCredentials({
			id: WEB_PASSWORD_PROVIDER_ID,
			authorize: authorizeWebPassword,
			crypto: {
				hashSecret,
				verifySecret,
			},
		}),
		ConvexCredentials({
			id: MINECRAFT_TOKEN_PROVIDER_ID,
			authorize: async (credentials, ctx) => {
				const minecraftAccessToken = credentials.minecraftAccessToken as string | undefined
				if (!minecraftAccessToken) return null

				const profile = await verifyMinecraftAccessToken(minecraftAccessToken)
				if (!profile) return null

				const minecraftUuid = normalizeUuid(profile.id)
				const gamertag = profile.name
				const personaId = devPersonaId(credentials)
				const accountId = personaAccountId(minecraftUuid, personaId)

				const existing = await retrieveAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
				}).catch((error) => {
					if (error instanceof Error && error.message === INVALID_ACCOUNT_ID) return null
					throw error
				})
				if (existing) {
					const amberiteUserId = existing.user.amberiteUserId ?? accountId
					await ctx.runMutation(internal.auth.ensureLinkedMinecraftAccount, {
						userId: existing.user._id,
						amberiteUserId,
						accountId,
						gamertag,
						minecraftUuid,
					})
					return { userId: existing.user._id }
				}

				const accountProfile = personaProfile(gamertag, accountId, personaId)
				const { user } = await createAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
					profile: accountProfile,
				})
				await ctx.runMutation(internal.auth.ensureLinkedMinecraftAccount, {
					userId: user._id,
					amberiteUserId: accountProfile.amberiteUserId,
					accountId,
					gamertag,
					minecraftUuid,
				})

				return { userId: user._id }
			},
		}),
		ConvexCredentials({
			id: MODRINTH_TOKEN_PROVIDER_ID,
			authorize: async (credentials, ctx) => {
				const modrinthToken = credentialString(credentials, 'modrinthToken')
				const profile = await verifyModrinthToken(modrinthToken)
				if (!profile) return null

				const accountId = `modrinth:${profile.id}`
				const existing = await retrieveAccount(ctx, {
					provider: MODRINTH_TOKEN_PROVIDER_ID,
					account: { id: accountId },
				}).catch((error) => {
					if (isMissingAccount(error)) return null
					throw error
				})
				if (existing) {
					await ctx.runMutation(internal.auth.patchUserFromModrinthProfile, {
						userId: existing.user._id,
						email: profile.email ?? undefined,
						emailVerified: profile.email_verified ?? undefined,
						avatarUrl: profile.avatar_url ?? undefined,
						bio: profile.bio ?? undefined,
					})
					return { userId: existing.user._id }
				}

				const username = await uniqueExternalUsername(ctx, profile.username, profile.id)
				const { user } = await createAccount(ctx, {
					provider: MODRINTH_TOKEN_PROVIDER_ID,
					account: { id: accountId },
					profile: {
						amberiteUserId: accountId,
						friendCode: createFriendCode(),
						displayName: profile.name ?? username,
						name: profile.name ?? username,
						username,
						normalizedUsername: username.toLowerCase(),
						email: profile.email ?? undefined,
						emailVerificationTime: profile.email_verified ? Date.now() : undefined,
						image: profile.avatar_url ?? undefined,
						avatarUrl: profile.avatar_url ?? undefined,
						bio: profile.bio ?? undefined,
						onboardedAt: Date.now(),
					},
				})
				return { userId: user._id }
			},
		}),
	],
})

async function authorizeWebPassword(credentials: Record<string, unknown>, ctx: any) {
	const flow = credentialString(credentials, 'flow')
	const password = credentialString(credentials, 'password')
	validatePassword(password)

	if (flow === 'signUp') {
		const email = normalizeEmail(credentialString(credentials, 'email'))
		const username = normalizeUsername(credentialString(credentials, 'username'))
		const availability = await ctx.runQuery(internal.auth.webPasswordAccountAvailability, {
			email,
			username,
		})
		if (!availability.emailAvailable) throw new Error('email is already registered')
		if (!availability.usernameAvailable) throw new Error('username is already taken')

		const { user } = await createAccount(ctx, {
			provider: WEB_PASSWORD_PROVIDER_ID,
			account: { id: email, secret: password },
			profile: {
				amberiteUserId: `web:${email}`,
				friendCode: createFriendCode(),
				displayName: username,
				name: username,
				username,
				normalizedUsername: username.toLowerCase(),
				email,
				emailVerificationTime: Date.now(),
				onboardedAt: Date.now(),
			},
		})
		return { userId: user._id }
	}

	if (flow === 'signIn') {
		const login = credentialString(credentials, 'login')
		const email = await ctx.runQuery(internal.auth.emailByWebPasswordLogin, { login })
		if (!email) return null
		const existing = await retrieveAccount(ctx, {
			provider: WEB_PASSWORD_PROVIDER_ID,
			account: { id: email, secret: password },
		}).catch((error) => {
			if (isCredentialFailure(error)) return null
			throw error
		})
		return existing ? { userId: existing.user._id } : null
	}

	throw new Error('unsupported password auth flow')
}

export const validateWebPasswordAccount = query({
	args: {
		email: v.string(),
		password: v.string(),
		username: v.string(),
	},
	handler: async (ctx, args) => {
		const email = normalizeEmail(args.email)
		const username = normalizeUsername(args.username)
		validatePassword(args.password)
		const availability = await webPasswordAvailability(ctx, email, username)
		if (!availability.emailAvailable) throw new Error('email is already registered')
		if (!availability.usernameAvailable) throw new Error('username is already taken')
		return { ok: true }
	},
})

export const webPasswordAccountAvailability = internalQuery({
	args: {
		email: v.string(),
		username: v.string(),
	},
	handler: async (ctx, args) => webPasswordAvailability(ctx, args.email, args.username),
})

export const emailByWebPasswordLogin = internalQuery({
	args: { login: v.string() },
	handler: async (ctx, args) => {
		const login = args.login.trim()
		if (login.includes('@')) return normalizeEmail(login)
		const normalizedUsername = login.toLowerCase()
		const user = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', normalizedUsername))
			.first()
		return user?.email ?? null
	},
})

export const usernameAvailable = internalQuery({
	args: { username: v.string() },
	handler: async (ctx, args) => {
		const username = normalizeUsername(args.username)
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', username.toLowerCase()))
			.first()
		return existing === null
	},
})

export const patchUserFromModrinthProfile = internalMutation({
	args: {
		userId: v.id('users'),
		email: v.optional(v.string()),
		emailVerified: v.optional(v.boolean()),
		avatarUrl: v.optional(v.string()),
		bio: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const patch: Record<string, unknown> = {}
		if (args.email !== undefined) {
			patch.email = normalizeEmail(args.email)
			if (args.emailVerified) patch.emailVerificationTime = Date.now()
		}
		if (args.avatarUrl !== undefined) {
			patch.avatarUrl = args.avatarUrl
			patch.image = args.avatarUrl
		}
		if (args.bio !== undefined) patch.bio = args.bio.slice(0, 1_024)
		if (Object.keys(patch).length > 0) await ctx.db.patch(args.userId, patch)
	},
})

export const updateCurrentEmail = mutation({
	args: { email: v.string() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const email = normalizeEmail(args.email)
		const existingAccount = await ctx.db
			.query('authAccounts')
			.withIndex('providerAndAccountId', (q) =>
				q.eq('provider', WEB_PASSWORD_PROVIDER_ID).eq('providerAccountId', email),
			)
			.first()
		if (existingAccount && existingAccount.userId !== userId) {
			throw new Error('email is already registered')
		}

		const passwordAccount = await ctx.db
			.query('authAccounts')
			.withIndex('userIdAndProvider', (q) =>
				q.eq('userId', userId).eq('provider', WEB_PASSWORD_PROVIDER_ID),
			)
			.first()
		if (passwordAccount) await ctx.db.patch(passwordAccount._id, { providerAccountId: email })

		await ctx.db.patch(userId, {
			email,
			emailVerificationTime: Date.now(),
		})
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		return publicCurrentProfile(user, await currentAccountFields(ctx, userId))
	},
})

export const updateCurrentPassword = mutation({
	args: {
		oldPassword: v.optional(v.union(v.string(), v.null())),
		newPassword: v.optional(v.union(v.string(), v.null())),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		const email = user.email ? normalizeEmail(user.email) : null
		const passwordAccount = await ctx.db
			.query('authAccounts')
			.withIndex('userIdAndProvider', (q) =>
				q.eq('userId', userId).eq('provider', WEB_PASSWORD_PROVIDER_ID),
			)
			.first()

		if (passwordAccount?.secret) {
			if (!args.oldPassword) throw new Error('current password is required')
			if (!(await verifySecret(args.oldPassword, passwordAccount.secret))) {
				throw new Error('current password is incorrect')
			}
		}

		if (args.newPassword === null) {
			if (passwordAccount) await ctx.db.delete(passwordAccount._id)
		} else if (args.newPassword !== undefined) {
			if (!email) throw new Error('email is required before setting a password')
			validatePassword(args.newPassword)
			const secret = await hashSecret(args.newPassword)
			if (passwordAccount) {
				await ctx.db.patch(passwordAccount._id, { providerAccountId: email, secret })
			} else {
				await ctx.db.insert('authAccounts', {
					userId,
					provider: WEB_PASSWORD_PROVIDER_ID,
					providerAccountId: email,
					secret,
				})
			}
		}

		return publicCurrentProfile(user, await currentAccountFields(ctx, userId))
	},
})

export const deleteCurrentAccount = mutation({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		await ctx.db.patch(userId, {
			deletedAt: Date.now(),
			deletedReason: 'user requested deletion',
		})
		return { ok: true }
	},
})

async function webPasswordAvailability(ctx: any, email: string, username: string) {
	const [existingAccount, existingUsername] = await Promise.all([
		ctx.db
			.query('authAccounts')
			.withIndex('providerAndAccountId', (q: any) =>
				q.eq('provider', WEB_PASSWORD_PROVIDER_ID).eq('providerAccountId', email),
			)
			.first(),
		ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q: any) =>
				q.eq('normalizedUsername', username.toLowerCase()),
			)
			.first(),
	])
	return {
		emailAvailable: existingAccount === null,
		usernameAvailable: existingUsername === null,
	}
}

async function uniqueExternalUsername(ctx: any, preferred: string, externalId: string): Promise<string> {
	const base = sanitizeExternalUsername(preferred)
	const suffix = externalId.replace(/[^a-zA-Z0-9]/g, '').slice(0, 8) || 'user'
	const candidates = [
		base,
		`${base.slice(0, Math.max(3, 23 - suffix.length))}_${suffix}`,
		`user_${suffix}`,
	]
	for (const candidate of candidates) {
		if (await ctx.runQuery(internal.auth.usernameAvailable, { username: candidate })) {
			return candidate
		}
	}
	return `user_${crypto.randomUUID().replace(/-/g, '').slice(0, 12)}`
}

function credentialString(credentials: Record<string, unknown>, key: string): string {
	const value = credentials[key]
	if (typeof value !== 'string' || !value.trim()) throw new Error(`missing ${key}`)
	return value.trim()
}

function normalizeEmail(value: string): string {
	const email = value.trim().toLowerCase()
	if (email.length > 320 || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) {
		throw new Error('email must be a valid address')
	}
	return email
}

function normalizeUsername(value: string): string {
	const username = value.trim()
	if (!USERNAME_PATTERN.test(username)) {
		throw new Error('username must be 3-24 letters, numbers, or underscores')
	}
	return username
}

function sanitizeExternalUsername(value: string): string {
	const sanitized = value
		.trim()
		.replace(/[^a-zA-Z0-9_]/g, '_')
		.replace(/_+/g, '_')
		.replace(/^_+|_+$/g, '')
		.slice(0, 24)
	if (USERNAME_PATTERN.test(sanitized)) return sanitized
	const padded = `${sanitized || 'user'}___`.slice(0, 3)
	return USERNAME_PATTERN.test(padded) ? padded : 'user'
}

function validatePassword(value: string): void {
	if (value.length < 8 || value.length > 2_048) {
		throw new Error('password must be between 8 and 2048 characters')
	}
}

function isMissingAccount(error: unknown): boolean {
	return error instanceof Error && error.message === INVALID_ACCOUNT_ID
}

function isCredentialFailure(error: unknown): boolean {
	return (
		error instanceof Error &&
		[INVALID_ACCOUNT_ID, INVALID_SECRET, TOO_MANY_FAILED_ATTEMPTS].includes(error.message)
	)
}

async function hashSecret(secret: string): Promise<string> {
	const salt = randomHex(16)
	const hash = await derivePasswordHash(secret, salt, PASSWORD_HASH_ITERATIONS)
	return `${PASSWORD_HASH_PREFIX}:${PASSWORD_HASH_ITERATIONS}:${salt}:${hash}`
}

async function verifySecret(secret: string, encoded: string): Promise<boolean> {
	const [prefix, iterationsRaw, salt, expected] = encoded.split(':')
	const iterations = Number(iterationsRaw)
	if (
		prefix !== PASSWORD_HASH_PREFIX ||
		!Number.isInteger(iterations) ||
		iterations < 1 ||
		!salt ||
		!expected
	) {
		return false
	}
	const actual = await derivePasswordHash(secret, salt, iterations)
	return constantTimeEqual(actual, expected)
}

async function derivePasswordHash(
	secret: string,
	saltHex: string,
	iterations: number,
): Promise<string> {
	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(secret),
		'PBKDF2',
		false,
		['deriveBits'],
	)
	const bits = await crypto.subtle.deriveBits(
		{
			name: 'PBKDF2',
			hash: 'SHA-256',
			salt: hexToBytes(saltHex),
			iterations,
		},
		key,
		256,
	)
	return bytesToHex(new Uint8Array(bits))
}

function randomHex(byteLength: number): string {
	const bytes = crypto.getRandomValues(new Uint8Array(byteLength))
	return bytesToHex(bytes)
}

function bytesToHex(bytes: Uint8Array): string {
	return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

function hexToBytes(hex: string): Uint8Array {
	const bytes = new Uint8Array(hex.length / 2)
	for (let i = 0; i < bytes.length; i++) bytes[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16)
	return bytes
}

function constantTimeEqual(left: string, right: string): boolean {
	if (left.length !== right.length) return false
	let diff = 0
	for (let i = 0; i < left.length; i++) diff |= left.charCodeAt(i) ^ right.charCodeAt(i)
	return diff === 0
}

export const ensureLinkedMinecraftAccount = internalMutation({
	args: {
		userId: v.id('users'),
		amberiteUserId: v.string(),
		accountId: v.string(),
		gamertag: v.string(),
		minecraftUuid: v.string(),
	},
	handler: async (ctx, args) => {
		const user = await ctx.db.get(args.userId)
		if (!user) throw new Error('user does not exist')
		if (!user.amberiteUserId) {
			await ctx.db.patch(args.userId, { amberiteUserId: args.amberiteUserId })
		}

		const existing = await ctx.db
			.query('linkedMicrosoftAccounts')
			.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', args.amberiteUserId))
			.first()
		if (existing) {
			await ctx.db.patch(existing._id, {
				microsoftAccountId: args.accountId,
				gamertag: args.gamertag,
				minecraftUuid: args.minecraftUuid,
			})
			return
		}
		await ctx.db.insert('linkedMicrosoftAccounts', {
			amberiteUserId: args.amberiteUserId,
			microsoftAccountId: args.accountId,
			gamertag: args.gamertag,
			minecraftUuid: args.minecraftUuid,
			linkedAt: Date.now(),
		})
	},
})

export const currentUser = query({
	args: {},
	handler: async (ctx) => {
		const userId = await getAuthUserId(ctx)
		if (userId === null) return null
		const user = await ctx.db.get(userId)
		return user
			? publicUser({ ...user, _id: userId }, true, await currentAccountFields(ctx, userId))
			: null
	},
})

export const setUsername = mutation({
	args: {
		username: v.string(),
		displayName: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)

		const username = args.username.trim()
		if (!/^[a-zA-Z0-9_]{3,24}$/.test(username)) {
			throw new Error('username must be 3-24 letters, numbers, or underscores')
		}

		const normalizedUsername = username.toLowerCase()
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', normalizedUsername))
			.unique()
		if (existing && existing._id !== userId) throw new Error('username is already taken')

		const user = await ctx.db.get(userId)
		const now = Date.now()
		await ctx.db.patch(userId, {
			amberiteUserId: user?.amberiteUserId ?? crypto.randomUUID(),
			username,
			normalizedUsername,
			displayName: args.displayName?.trim() || username,
			onboardedAt: user?.onboardedAt ?? now,
		})

		return { userId, username }
	},
})
