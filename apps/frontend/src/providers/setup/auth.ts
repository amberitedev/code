import type { Labrinth } from '@modrinth/api-client'
import { type AuthProvider, type AuthUser, provideAuth } from '@modrinth/ui'
import { ref, watchEffect } from 'vue'

import { getSignInRedirectPath } from '~/composables/auth.ts'

export function setupAuthProvider(auth: Awaited<ReturnType<typeof useAuth>>) {
	const router = useRouter()
	const route = useRoute()
	const sessionToken = ref<string | null>(null)
	const user = ref<AuthUser | null>(null)

	const authProvider: AuthProvider = {
		session_token: sessionToken,
		user,
		isReady: ref(true),
		requestSignIn: async (redirectPath: string) => {
			await router.push({
				path: '/auth/sign-in',
				query: {
					redirect: redirectPath || getSignInRedirectPath(route),
				},
			})
		},
	}

	watchEffect(() => {
		// Amberite JWTs are accepted only by Amberite/Convex and must never authenticate Labrinth.
		sessionToken.value = null
		user.value = (auth.value.user as Labrinth.Users.v3.User | null) ?? null
	})

	provideAuth(authProvider)
}
