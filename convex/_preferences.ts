import type { Id } from './_generated/dataModel'
import type { MutationCtx, QueryCtx } from './_generated/server'
import { acceptedFriendship } from './_socialRules'
import {
	DEFAULT_USER_PREFERENCES,
	mergeUserPreferences,
	type PartialUserPreferences,
	type UserPreferences,
} from './_preferencesModel'

const MAX_FRIENDS = 200

export async function getUserPreferences(
	ctx: QueryCtx | MutationCtx,
	userId: Id<'users'>,
): Promise<UserPreferences> {
	const stored = await ctx.db
		.query('userPreferences')
		.withIndex('by_user', (index) => index.eq('userId', userId))
		.unique()
	if (stored) return stored.preferences

	const user = await ctx.db.get(userId)
	return mergeUserPreferences(DEFAULT_USER_PREFERENCES, {
		...(user?.allowFriendRequests === false ? { social: { friend_privacy: 'none' as const } } : {}),
	})
}

export async function updateUserPreferences(
	ctx: MutationCtx,
	userId: Id<'users'>,
	patch: PartialUserPreferences,
): Promise<UserPreferences> {
	const current = await getUserPreferences(ctx, userId)
	const preferences = mergeUserPreferences(current, patch)
	const stored = await ctx.db
		.query('userPreferences')
		.withIndex('by_user', (index) => index.eq('userId', userId))
		.unique()
	if (stored) await ctx.db.patch(stored._id, { preferences })
	else await ctx.db.insert('userPreferences', { userId, preferences })

	if (patch.social?.friend_privacy !== undefined) {
		await ctx.db.patch(userId, {
			allowFriendRequests: patch.social.friend_privacy !== 'none',
		})
	}
	return preferences
}

export async function canSendFriendRequest(
	ctx: QueryCtx | MutationCtx,
	fromUserId: Id<'users'>,
	toUserId: Id<'users'>,
): Promise<boolean> {
	const privacy = (await getUserPreferences(ctx, toUserId)).social.friend_privacy
	if (privacy === 'none') return false
	if (privacy === 'everyone') return true
	return await haveMutualFriend(ctx, fromUserId, toUserId)
}

export async function canSendSharedInstanceInvite(
	ctx: QueryCtx | MutationCtx,
	fromUserId: Id<'users'>,
	toUserId: Id<'users'>,
): Promise<boolean> {
	const privacy = (await getUserPreferences(ctx, toUserId)).social.shared_instances_privacy
	if (privacy === 'none') return false
	if (privacy === 'everyone') return true
	return Boolean(await acceptedFriendship(ctx, fromUserId, toUserId))
}

async function haveMutualFriend(
	ctx: QueryCtx | MutationCtx,
	firstUserId: Id<'users'>,
	secondUserId: Id<'users'>,
): Promise<boolean> {
	const [firstLeft, firstRight, secondLeft, secondRight] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (index) => index.eq('userAId', firstUserId))
			.take(MAX_FRIENDS),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (index) => index.eq('userBId', firstUserId))
			.take(MAX_FRIENDS),
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (index) => index.eq('userAId', secondUserId))
			.take(MAX_FRIENDS),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (index) => index.eq('userBId', secondUserId))
			.take(MAX_FRIENDS),
	])
	const firstFriends = new Set([
		...firstLeft.map((friendship) => friendship.userBId),
		...firstRight.map((friendship) => friendship.userAId),
	])
	return [
		...secondLeft.map((friendship) => friendship.userBId),
		...secondRight.map((friendship) => friendship.userAId),
	].some((userId) => firstFriends.has(userId))
}
