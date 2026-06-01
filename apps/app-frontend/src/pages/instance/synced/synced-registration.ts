import { useSocialClient } from '@/composables/useSocialClient'
import type { GameInstance } from '@/helpers/types'

/**
 * Best-effort registration of a synced profile with the Convex backend.
 *
 * Registration links the local synced profile (its client profile id + the Core
 * instance backing the server side) to the user's friend group so membership
 * roles, snapshots and presets apply. It is best-effort: a solo user with no
 * friend group, or no social session, simply skips registration and keeps the
 * fully-local owner experience. All failures are swallowed so creating or
 * converting a synced profile never fails because of the social backend.
 */
export async function registerSyncedProfileBackend(args: {
	profilePath: string
	serverInstanceId: string
	instance: Pick<GameInstance, 'name' | 'game_version' | 'loader'>
}): Promise<void> {
	try {
		const client = useSocialClient()
		const groups = await client.listMyFriendGroups()
		// The primary friend group is the one bound to the local Core; its coreId
		// is the Core server identity (distinct from a per-instance id).
		const group = groups.find((g) => !!g.group.coreId) ?? groups[0]
		if (!group?.group.coreId) return

		await client.registerSyncedProfile({
			friendGroupId: group.group.id,
			coreId: group.group.coreId,
			coreInstanceId: args.serverInstanceId,
			clientProfileId: args.profilePath,
			name: args.instance.name,
			gameVersion: args.instance.game_version,
			loader: args.instance.loader,
		})
	} catch (error) {
		console.warn('[synced] backend registration skipped:', error)
	}
}
