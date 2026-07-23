import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { QueryCtx } from './_generated/server'
import {
	blockByPair,
	canViewProfileSection,
	coreById,
	coreListById,
	currentAccountFields,
	normalizeProfileVisibilitySettings,
	publicCurrentProfile,
	publicProfile,
	publicUser,
	requireUserId,
	profileRelationship,
	profileVisibilitySettings,
} from './_socialRules'
import type { ProfileRelationshipContext } from './_socialRules'

const MAX_BIO_LENGTH = 1_024
const MAX_SEARCH_LIMIT = 20
const DEFAULT_SEARCH_LIMIT = 8
const MAX_FAVORITE_MODPACKS = 6
const MAX_SHOWCASE_ACHIEVEMENTS = 8
const MAX_VISIBLE_PROFILE_FRIENDS = 8
const MAX_PROFILE_FRIEND_SCAN = 64
const SHOWCASE_ID_PATTERN = /^[a-zA-Z0-9_.:-]{1,128}$/

const avatarInput = v.union(
	v.null(),
	v.object({
		url: v.string(),
		storageId: v.optional(v.string()),
		mimeType: v.optional(v.string()),
		sizeBytes: v.optional(v.number()),
	}),
)

const sectionVisibility = v.union(
	v.literal('everyone'),
	v.literal('friends'),
	v.literal('friend_group'),
	v.literal('private'),
)

const profileVisibilityInput = v.object({
	friends: v.optional(sectionVisibility),
	friendGroup: v.optional(sectionVisibility),
	corePresence: v.optional(sectionVisibility),
	favoriteModpacks: v.optional(sectionVisibility),
	achievements: v.optional(sectionVisibility),
})

export const current = query({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		return publicCurrentProfile(user, await currentAccountFields(ctx, userId))
	},
})

export const get = query({
	args: { idOrUsername: v.string() },
	handler: async (ctx, args) => {
		await requireUserId(ctx)
		const user = await findUser(ctx, args.idOrUsername)
		return user && !user.deletedAt ? publicProfile(user) : null
	},
})

export const view = query({
	args: { idOrUsername: v.string() },
	handler: async (ctx, args) => {
		const viewerId = await requireUserId(ctx)
		const user = await findUser(ctx, args.idOrUsername)
		if (!user || user.deletedAt) return null

		const relationship = await profileRelationship(ctx, viewerId, user._id)
		const settings = profileVisibilitySettings(user)
		const result: any = {
			user: publicProfile(user),
			viewer: {
				relationship: relationship.kind,
				isSelf: relationship.self,
				isFriend: relationship.friend,
				isFriendGroup: relationship.friendGroup,
				isGroupManager: relationship.groupManager,
				blocked: relationship.blocked,
				viewerBlockedTarget: relationship.viewerBlockedTarget,
				targetBlockedViewer: relationship.targetBlockedViewer,
			},
			actions: profileActions(relationship),
		}

		if (relationship.self) {
			result.settings = settings
		}

		if (canViewProfileSection(settings.favoriteModpacks, relationship)) {
			result.favoriteModpacks = {
				projectIds: boundedExistingList(user.favoriteModpackProjectIds, MAX_FAVORITE_MODPACKS),
			}
		}

		if (canViewProfileSection(settings.achievements, relationship)) {
			result.achievements = {
				achievementIds: boundedExistingList(user.showcaseAchievementIds, MAX_SHOWCASE_ACHIEVEMENTS),
			}
		}

		if (canViewProfileSection(settings.friendGroup, relationship)) {
			const friendGroup = profileFriendGroupSection(relationship)
			if (friendGroup) result.friendGroup = friendGroup
		}

		if (canViewProfileSection(settings.corePresence, relationship)) {
			const corePresence = await profileCoreSection(ctx, user._id, relationship)
			if (corePresence) result.corePresence = corePresence
		}

		if (canViewProfileSection(settings.friends, relationship)) {
			result.friends = await profileFriendsSection(ctx, viewerId, user._id)
		}

		const management = profileManagementSection(relationship)
		if (management) result.management = management

		return result
	},
})

export const search = query({
	args: { query: v.string(), limit: v.optional(v.number()) },
	handler: async (ctx, args) => {
		const viewerId = await requireUserId(ctx)
		const term = args.query.trim()
		if (term.length < 2 || term.length > 32) return []
		const limit = strictLimit(args.limit)
		const normalizedTerm = term.toLowerCase()
		const upperBound = `${normalizedTerm}\uffff`
		const users = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) =>
				q.gte('normalizedUsername', normalizedTerm).lt('normalizedUsername', upperBound),
			)
			.take(limit)
		return users
			.filter((user) => {
				if (user._id === viewerId || user.deletedAt) return false
				return user.normalizedUsername?.startsWith(normalizedTerm)
			})
			.map((user) => publicProfile(user))
	},
})

export const updateCurrent = mutation({
	args: {
		displayName: v.optional(v.string()),
		username: v.optional(v.string()),
		bio: v.optional(v.string()),
		avatar: v.optional(avatarInput),
		profileVisibility: v.optional(profileVisibilityInput),
		favoriteModpackProjectIds: v.optional(v.array(v.string())),
		showcaseAchievementIds: v.optional(v.array(v.string())),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		const patch: any = { profileUpdatedAt: Date.now() }

		if (
			args.username !== undefined &&
			args.username.trim() !== (user.verifiedMinecraftHandle ?? user.username)
		) {
			throw new Error('verified Minecraft handle cannot be edited')
		}

		if (args.displayName !== undefined) {
			const displayName = normalizeDisplayName(args.displayName)
			patch.displayName = displayName
			patch.name = displayName
			patch.onboardedAt = user.onboardedAt ?? Date.now()
			patch.amberiteUserId = user.amberiteUserId ?? crypto.randomUUID()
		}

		if (args.bio !== undefined) patch.bio = normalizeBio(args.bio)

		if (args.profileVisibility !== undefined) {
			patch.profileVisibility = normalizeProfileVisibilitySettings(
				args.profileVisibility,
				user.profileVisibility,
			)
		}

		if (args.favoriteModpackProjectIds !== undefined) {
			patch.favoriteModpackProjectIds = normalizeShowcaseIds(
				args.favoriteModpackProjectIds,
				MAX_FAVORITE_MODPACKS,
				'favorite modpacks',
			)
		}

		if (args.showcaseAchievementIds !== undefined) {
			patch.showcaseAchievementIds = normalizeShowcaseIds(
				args.showcaseAchievementIds,
				MAX_SHOWCASE_ACHIEVEMENTS,
				'achievements',
			)
		}

		if (args.avatar !== undefined) {
			if (args.avatar === null) {
				patch.avatarUrl = undefined
				patch.avatarStorageId = undefined
				patch.avatarMimeType = undefined
				patch.avatarSizeBytes = undefined
				patch.image = undefined
			} else {
				patch.avatarUrl = normalizeAvatarUrl(args.avatar.url)
				patch.avatarStorageId = args.avatar.storageId
				patch.avatarMimeType = args.avatar.mimeType
				patch.avatarSizeBytes = args.avatar.sizeBytes
				patch.image = patch.avatarUrl
			}
		}

		await ctx.db.patch(userId, patch)
		const updated = await ctx.db.get(userId)
		if (!updated) throw new Error('user not found')
		return publicCurrentProfile(updated, await currentAccountFields(ctx, userId))
	},
})

async function findUser(ctx: QueryCtx, idOrUsername: string) {
	const normalized = idOrUsername.trim().toLowerCase()
	const byUsername = await ctx.db
		.query('users')
		.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', normalized))
		.unique()
	if (byUsername) return byUsername
	try {
		return await ctx.db.get(idOrUsername as Id<'users'>)
	} catch {
		return null
	}
}

function profileActions(relationship: ProfileRelationshipContext) {
	const actions: any = {}
	if (relationship.self) {
		actions.editProfile = true
		actions.editAccount = true
		return actions
	}

	if (relationship.targetBlockedViewer) return actions
	if (relationship.viewerBlockedTarget) {
		actions.unblock = true
		return actions
	}

	if (relationship.friend) {
		actions.removeFriend = true
	} else if (relationship.incomingRequest?.status === 'pending') {
		actions.acceptFriendRequest = { requestId: relationship.incomingRequest._id }
		actions.declineFriendRequest = { requestId: relationship.incomingRequest._id }
	} else if (relationship.outgoingRequest?.status === 'pending') {
		actions.cancelFriendRequest = { requestId: relationship.outgoingRequest._id }
	} else {
		actions.addFriend = true
	}

	actions.block = true

	if (
		relationship.viewerManageableMembership &&
		!relationship.targetMembership &&
		!relationship.targetBan
	) {
		actions.inviteToFriendGroup = {
			friendGroupId: relationship.viewerManageableMembership.friendGroupId,
		}
	}

	if (relationship.groupManager && relationship.targetMembership) {
		actions.updateMemberRole = {
			friendGroupId: relationship.targetMembership.friendGroupId,
			roles: manageableRoles(relationship.viewerMembership?.role),
		}
		actions.kick = { friendGroupId: relationship.targetMembership.friendGroupId }
		actions.ban = { friendGroupId: relationship.targetMembership.friendGroupId }
	}

	if (relationship.targetBan) {
		actions.unban = { friendGroupId: relationship.targetBan.friendGroupId }
	}

	return actions
}

function manageableRoles(role: unknown): Array<'admin' | 'member'> {
	return role === 'owner' ? ['admin', 'member'] : ['member']
}

function profileManagementSection(relationship: ProfileRelationshipContext) {
	if (relationship.blocked) return null
	if (relationship.groupManager && relationship.targetMembership) {
		return {
			friendGroupId: relationship.targetMembership.friendGroupId,
			userId: relationship.targetMembership.userId,
			role: relationship.targetMembership.role,
			permissionPreset: relationship.targetMembership.permissionPreset ?? null,
			canUpdateRole: true,
			canKick: true,
			canBan: true,
			roles: manageableRoles(relationship.viewerMembership?.role),
		}
	}

	if (relationship.targetBan) {
		return {
			friendGroupId: relationship.targetBan.friendGroupId,
			userId: relationship.targetBan.userId,
			banned: true,
			canUnban: true,
		}
	}

	return null
}

function profileFriendGroupSection(relationship: ProfileRelationshipContext) {
	if (!relationship.friendGroupDoc || !relationship.targetMembership) return null
	const group = relationship.friendGroupDoc
	return {
		group: {
			id: group._id,
			_id: group._id,
			name: group.name ?? null,
			description: group.description ?? null,
			banner: group.banner ?? null,
			subdomain: group.subdomain ?? null,
			coreId: group.coreId ?? null,
			ownerUserId: group.ownerUserId,
			createdAt: group.createdAt,
			updatedAt: group.updatedAt ?? null,
		},
		membership: {
			role: relationship.targetMembership.role,
			permissionPreset: relationship.targetMembership.permissionPreset ?? null,
			createdAt: relationship.targetMembership.createdAt,
			updatedAt: relationship.targetMembership.updatedAt ?? null,
		},
	}
}

async function profileCoreSection(
	ctx: QueryCtx,
	targetUserId: string,
	relationship: ProfileRelationshipContext,
) {
	const groupCoreId = relationship.friendGroupDoc?.coreId
	const coreListEntry = groupCoreId
		? await coreListById(ctx, groupCoreId)
		: await ctx.db
				.query('coreList')
				.withIndex('by_owner', (q) => q.eq('ownerUserId', targetUserId))
				.first()
	const legacyCore = groupCoreId
		? await coreById(ctx, groupCoreId)
		: coreListEntry
			? await coreById(ctx, coreListEntry.coreId)
			: null

	if (!coreListEntry && !legacyCore) return null
	const coreId = coreListEntry?.coreId ?? legacyCore?.coreId
	const ownerUserId = coreListEntry?.ownerUserId ?? legacyCore?.ownerUserId
	if (!coreId || !ownerUserId) return null

	return {
		coreId,
		ownerUserId,
		linkState: coreListEntry?.linkState ?? null,
		name: legacyCore?.name ?? null,
		subdomain: legacyCore?.subdomain ?? null,
		setupMode: coreListEntry?.setupMode ?? legacyCore?.setupMode ?? null,
		lastSeenAt: coreListEntry?.lastSeenAt ?? legacyCore?.lastSeenAt ?? null,
		status: legacyCore?.status ?? null,
	}
}

async function profileFriendsSection(ctx: QueryCtx, viewerId: string, targetUserId: string) {
	const [targetFriends, viewerFriends] = await Promise.all([
		friendIdsForUser(ctx, targetUserId),
		friendIdsForUser(ctx, viewerId),
	])
	const viewerFriendIds = new Set(viewerFriends.ids)
	const visibleFriendIds = targetFriends.ids.filter((userId) => userId !== viewerId)
	const mutualFriendIds = visibleFriendIds.filter((userId) => viewerFriendIds.has(userId))

	return {
		count: targetFriends.count,
		hasMore: targetFriends.hasMore,
		items: await publicUsersForIds(
			ctx,
			viewerId,
			visibleFriendIds.slice(0, MAX_VISIBLE_PROFILE_FRIENDS),
		),
		mutualCount: mutualFriendIds.length,
		mutualHasMore: mutualFriendIds.length > MAX_VISIBLE_PROFILE_FRIENDS,
		mutual: await publicUsersForIds(
			ctx,
			viewerId,
			mutualFriendIds.slice(0, MAX_VISIBLE_PROFILE_FRIENDS),
		),
	}
}

async function friendIdsForUser(ctx: QueryCtx, userId: string) {
	const [left, right] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (q) => q.eq('userAId', userId))
			.take(MAX_PROFILE_FRIEND_SCAN + 1),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (q) => q.eq('userBId', userId))
			.take(MAX_PROFILE_FRIEND_SCAN + 1),
	])
	const ids = uniqueStrings([
		...left.map((friendship) => friendship.userBId),
		...right.map((friendship) => friendship.userAId),
	])
	const hasMore =
		left.length > MAX_PROFILE_FRIEND_SCAN ||
		right.length > MAX_PROFILE_FRIEND_SCAN ||
		ids.length > MAX_PROFILE_FRIEND_SCAN
	return {
		ids: ids.slice(0, MAX_PROFILE_FRIEND_SCAN),
		count: Math.min(ids.length, MAX_PROFILE_FRIEND_SCAN),
		hasMore,
	}
}

async function publicUsersForIds(ctx: QueryCtx, viewerId: string, userIds: string[]) {
	const users = await Promise.all(
		userIds.map(async (userId) => {
			const [viewerBlock, targetBlock, user] = await Promise.all([
				blockByPair(ctx, viewerId, userId),
				blockByPair(ctx, userId, viewerId),
				ctx.db.get(userId as Id<'users'>),
			])
			if (viewerBlock || targetBlock || !user || user.deletedAt) return null
			return publicUser(user)
		}),
	)
	return users.filter(Boolean)
}

function boundedExistingList(value: string[] | undefined, max: number): string[] {
	return uniqueStrings(value ?? []).slice(0, max)
}

function normalizeShowcaseIds(values: string[], max: number, field: string): string[] {
	const ids = uniqueStrings(
		values.map((value) => {
			const trimmed = value.trim()
			if (!SHOWCASE_ID_PATTERN.test(trimmed)) throw new Error(`invalid ${field} item`)
			return trimmed
		}),
	)
	if (ids.length > max) throw new Error(`${field} can include at most ${max} items`)
	return ids
}

function uniqueStrings(values: string[]): string[] {
	return [...new Set(values.filter((value) => typeof value === 'string' && value.length > 0))]
}

function normalizeDisplayName(value: string): string {
	const displayName = value.trim()
	if (displayName.length < 1 || displayName.length > 64)
		throw new Error('display name must be between 1 and 64 characters')
	return displayName
}

function normalizeBio(value: string): string {
	const bio = value.trim()
	if (bio.length > MAX_BIO_LENGTH)
		throw new Error(`bio must be ${MAX_BIO_LENGTH} characters or fewer`)
	return bio
}

function normalizeAvatarUrl(value: string): string {
	const url = value.trim()
	if (/^data:image\/(png|jpeg|gif|webp);base64,[a-zA-Z0-9+/=]+$/.test(url)) {
		if (url.length > 400_000) throw new Error('avatar image is too large')
		return url
	}
	if (url.length > 2_048) throw new Error('avatar URL is too long')
	try {
		const parsed = new URL(url)
		if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') throw new Error()
	} catch {
		throw new Error('avatar URL must be a valid HTTP or HTTPS URL')
	}
	return url
}

function strictLimit(value: number | undefined): number {
	if (value === undefined) return DEFAULT_SEARCH_LIMIT
	if (!Number.isInteger(value) || value < 1 || value > MAX_SEARCH_LIMIT)
		throw new Error(`limit must be between 1 and ${MAX_SEARCH_LIMIT}`)
	return value
}
