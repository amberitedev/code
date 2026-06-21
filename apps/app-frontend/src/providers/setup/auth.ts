import { type AuthProvider, provideAuth } from '@modrinth/ui'
import { computed, type Ref, ref } from 'vue'

export interface AmberiteAuthUser {
	id: string
	username: string
	name: string
	avatar_url: string
	role: string
	badges: number
	created: string
	bio?: string
	email?: string | null
}

export function setupAuthProvider(
	user: Ref<AmberiteAuthUser | null>,
	requestSignIn: (redirectPath: string) => void | Promise<void>,
) {
	const sessionToken = ref<string | null>(null)
	const isReady = computed(() => true)

	const authProvider: AuthProvider = {
		session_token: sessionToken,
		user: user as unknown as AuthProvider['user'],
		isReady,
		requestSignIn,
	}

	provideAuth(authProvider)
}
