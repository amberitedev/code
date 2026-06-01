/**
 * Dev-only DevTools API exposed as `window.__amberite`.
 *
 * Lets a developer act as any seeded Convex user, seed/reset social state, and
 * call any social action from the console — driving the /core page end to end
 * before real Microsoft auth is wired. Guarded by import.meta.env.DEV; never
 * installed in production builds. The Convex deployment additionally ignores the
 * acting-user override unless AMBERITE_DEV_MODE is set.
 *
 * Usage from DevTools:
 *   await __amberite.seed()                 // create the 4 seeded users
 *   await __amberite.users()                // list them
 *   await __amberite.actAs('amber')         // act as a user (by username or id)
 *   __amberite.whoami()                     // current acting user id
 *   await __amberite.reset()                // wipe social state (keeps users)
 *   await __amberite.state()                // full social snapshot
 *   __amberite.social                       // the AmberiteSocialClient
 */
import type { AmberiteUser } from '@amberite/amberite-api'

import { DEV_ACTING_USER_KEY } from '@/adapters/desktop'
import { useSocial } from '@/composables/useSocial'
import { useSocialClientRaw } from '@/composables/useSocialClient'

type SeededUser = AmberiteUser & { userId: string }

let installed = false

export function installAmberiteDevApi(): void {
	if (installed || !import.meta.env.DEV) return
	installed = true

	const client = useSocialClientRaw()
	const social = useSocial()

	async function users(): Promise<SeededUser[]> {
		return client.rawQuery<SeededUser[]>('dev:listDevUsers', {})
	}

	async function resolveUserId(usernameOrId: string): Promise<string> {
		if (usernameOrId.length > 20 && !usernameOrId.includes(' ')) return usernameOrId
		const all = await users()
		const match = all.find(
			(u) => u.username?.toLowerCase() === usernameOrId.toLowerCase() || u.userId === usernameOrId,
		)
		if (!match) throw new Error(`no seeded user matching "${usernameOrId}"`)
		return match.userId
	}

	const api = {
		social,
		client,
		async seed(): Promise<SeededUser[]> {
			const seeded = await client.rawMutation<SeededUser[]>('dev:seedDevUsers', {})
			await social.refresh()
			return seeded
		},
		users,
		async actAs(usernameOrId: string): Promise<string> {
			const userId = await resolveUserId(usernameOrId)
			window.localStorage.setItem(DEV_ACTING_USER_KEY, userId)
			await social.refresh()
			return userId
		},
		whoami(): string | null {
			return window.localStorage.getItem(DEV_ACTING_USER_KEY)
		},
		clearActor(): void {
			window.localStorage.removeItem(DEV_ACTING_USER_KEY)
			void social.refresh()
		},
		async reset(includeUsers = false): Promise<void> {
			await client.rawMutation('dev:resetSocial', { includeUsers })
			await social.refresh()
		},
		state(): Promise<unknown> {
			return client.rawQuery('dev:devState', {})
		},
		refresh(): Promise<void> {
			return social.refresh()
		},
	}

	;(window as unknown as { __amberite: typeof api }).__amberite = api
	console.info(
		'[amberite] dev API ready — try `await __amberite.seed()` then `await __amberite.actAs("amber")`',
	)
}
