import type {
	AmberiteAuthClient,
	AmberiteDevAccountSignInRequest,
	AmberiteMinecraftSignInRequest,
	AmberiteMinecraftTokenSignInRequest,
	AmberiteSession,
} from './auth-client'
import type { AmberiteAccountUser, AmberiteProfilePatch } from './profile'
import type { AmberiteSessionTokens } from './session'

export interface MockAmberiteAuthClientOptions {
	user?: AmberiteAccountUser | null
	tokens?: AmberiteSessionTokens
}

export class MockAmberiteAuthClient implements AmberiteAuthClient {
	private user: AmberiteAccountUser | null
	private tokens: AmberiteSessionTokens

	constructor(options: MockAmberiteAuthClientOptions = {}) {
		this.user = options.user ?? defaultMockUser()
		this.tokens = options.tokens ?? {
			token: 'mock-amberite-token',
			refreshToken: 'mock-amberite-refresh-token',
		}
	}

	async restoreSession(): Promise<AmberiteSession | null> {
		return this.user ? { tokens: this.tokens, user: this.user } : null
	}

	async refreshSession(): Promise<AmberiteSession | null> {
		return await this.restoreSession()
	}

	async signInWithMinecraft(_request: AmberiteMinecraftSignInRequest): Promise<AmberiteSession> {
		return this.requireSession()
	}

	async signInWithMinecraftToken(
		_request: AmberiteMinecraftTokenSignInRequest,
	): Promise<AmberiteSession> {
		return this.requireSession()
	}

	async signInWithDevAccount(_request: AmberiteDevAccountSignInRequest): Promise<AmberiteSession> {
		return this.requireSession()
	}

	async currentUser(): Promise<AmberiteAccountUser | null> {
		return this.user
	}

	async updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser> {
		const current = this.requireUser()
		this.user = {
			...current,
			...(patch.displayName !== undefined ? { name: patch.displayName } : {}),
			...(patch.bio !== undefined ? { bio: patch.bio } : {}),
			...(patch.avatar !== undefined
				? { avatar_url: patch.avatar === null ? null : patch.avatar.url }
				: {}),
		}
		return this.user
	}

	async deleteCurrentAccount(): Promise<void> {
		this.user = null
	}

	async logOut(): Promise<void> {
		this.user = null
	}

	private requireSession(): AmberiteSession {
		const user = this.requireUser()
		return { tokens: this.tokens, user }
	}

	private requireUser(): AmberiteAccountUser {
		if (!this.user) throw new Error('mock Amberite user is signed out')
		return this.user
	}
}

export function defaultMockUser(): AmberiteAccountUser {
	return {
		id: 'mock-user-id',
		userId: 'mock-user-id',
		username: 'devuser',
		minecraftUuid: '00000000-0000-4000-8000-000000000001',
		verifiedMinecraftHandle: 'devuser',
		name: 'Dev User',
		email: null,
		avatar_url: null,
		bio: null,
		created: '2024-01-01T00:00:00.000Z',
		role: 'developer',
		badges: 0,
		auth_providers: ['minecraft'],
		email_verified: false,
		has_password: false,
		has_totp: false,
	}
}
