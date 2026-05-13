import type { Labrinth } from '@modrinth/api-client'
import { type AuthProvider, provideAuth } from '@modrinth/ui'
import { computed, type Ref, ref, watchEffect } from 'vue'

type AppCredentials = {
	session?: string | null
	user?: Labrinth.Users.v2.User | null
}

// AMBERITE PATCH: Mock user returned in dev mode so auth is always satisfied.
const MOCK_USER: Labrinth.Users.v2.User = {
	id: '00000000-0000-0000-0000-000000000001',
	username: 'amberite_dev',
	name: 'Amberite Dev',
	email: 'dev@amberite.dev',
	created: '2024-01-01T00:00:00Z',
	role: 'developer',
	badges: 0,
}

export function setupAuthProvider(
	credentials: Ref<AppCredentials | null | undefined>,
	requestSignIn: (redirectPath: string) => void | Promise<void>,
) {
	// AMBERITE PATCH: In dev mode, always inject a mock user so hosting pages
	// never show the sign-in / subscription paywall.
	// eslint-disable-next-line turbo/no-undeclared-env-vars
	if (import.meta.env.DEV) {
		const sessionToken = ref<string | null>('mock-session-token')
		const user = ref<Labrinth.Users.v2.User | null>(MOCK_USER)
		const isReady = computed(() => true)

		provideAuth({ session_token: sessionToken, user, isReady, requestSignIn })
		return
	}

	const sessionToken = ref<string | null>(null)
	const user = ref<Labrinth.Users.v2.User | null>(null)
	const isReady = computed(() => credentials.value !== undefined)

	const authProvider: AuthProvider = {
		session_token: sessionToken,
		user,
		isReady,
		requestSignIn,
	}

	watchEffect(() => {
		sessionToken.value = credentials.value?.session ?? null
		user.value = credentials.value?.user ?? null
	})

	provideAuth(authProvider)
}
