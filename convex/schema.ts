import { authTables } from '@convex-dev/auth/server'
import { defineSchema, defineTable } from 'convex/server'
import { v } from 'convex/values'
import { userPreferencesValidator } from './_preferencesModel'

const friendRequestStatus = v.union(
	v.literal('pending'),
	v.literal('accepted'),
	v.literal('declined'),
	v.literal('canceled'),
)
const socialNotificationStatus = v.union(
	v.literal('unread'),
	v.literal('read'),
	v.literal('dismissed'),
)
const socialNotificationType = v.union(
	v.literal('friend_request'),
	v.literal('friend_request_accepted'),
	v.literal('client_invite'),
	v.literal('client_access_revoked'),
	v.literal('client_update'),
)
const sharedClientInviteStatus = v.union(
	v.literal('pending'),
	v.literal('accepted'),
	v.literal('declined'),
	v.literal('revoked'),
	v.literal('expired'),
)
const linkedAccountStatus = v.union(
	v.literal('active'),
	v.literal('needs_reconnect'),
	v.literal('revoked'),
)

export default defineSchema({
	...authTables,
	users: defineTable({
		name: v.optional(v.string()),
		displayName: v.optional(v.string()),
		username: v.optional(v.string()),
		normalizedUsername: v.optional(v.string()),
		friendCode: v.optional(v.string()),
		amberiteUserId: v.optional(v.string()),
		email: v.optional(v.string()),
		emailVerificationTime: v.optional(v.number()),
		phone: v.optional(v.string()),
		phoneVerificationTime: v.optional(v.number()),
		image: v.optional(v.string()),
		bio: v.optional(v.string()),
		avatarUrl: v.optional(v.string()),
		avatarStorageId: v.optional(v.id('_storage')),
		avatarMimeType: v.optional(v.string()),
		avatarSizeBytes: v.optional(v.number()),
		profileUpdatedAt: v.optional(v.number()),
		deletedAt: v.optional(v.number()),
		deletedReason: v.optional(v.string()),
		isAnonymous: v.optional(v.boolean()),
		onboardedAt: v.optional(v.number()),
		minecraftUuid: v.optional(v.string()),
		verifiedMinecraftHandle: v.optional(v.string()),
		normalizedVerifiedMinecraftHandle: v.optional(v.string()),
		minecraftVerifiedAt: v.optional(v.number()),
		minecraftLastVerifiedAt: v.optional(v.number()),
		allowFriendRequests: v.optional(v.boolean()),
	})
		.index('email', ['email'])
		.index('phone', ['phone'])
		.index('by_amberite_user_id', ['amberiteUserId'])
		.index('by_friend_code', ['friendCode'])
		.index('by_normalized_username', ['normalizedUsername'])
		.index('by_minecraft_uuid', ['minecraftUuid'])
		.index('by_verified_minecraft_handle', ['normalizedVerifiedMinecraftHandle']),
	linkedMicrosoftAccounts: defineTable({
		amberiteUserId: v.string(),
		microsoftAccountId: v.string(),
		gamertag: v.optional(v.string()),
		minecraftUuid: v.optional(v.string()),
		linkedAt: v.number(),
		verifiedAt: v.optional(v.number()),
		lastVerifiedAt: v.optional(v.number()),
	})
		.index('by_amberite_user', ['amberiteUserId'])
		.index('by_microsoft_account', ['microsoftAccountId'])
		.index('by_minecraft_uuid', ['minecraftUuid']),
	linkedModrinthAccounts: defineTable({
		userId: v.id('users'),
		modrinthUserId: v.string(),
		username: v.string(),
		avatarUrl: v.optional(v.string()),
		scopes: v.array(v.string()),
		encryptedAccessToken: v.string(),
		encryptedRefreshToken: v.optional(v.string()),
		expiresAt: v.optional(v.number()),
		status: linkedAccountStatus,
		needsReconnect: v.optional(v.boolean()),
		reconnectReason: v.optional(v.string()),
		linkedAt: v.number(),
		updatedAt: v.number(),
	})
		.index('by_user', ['userId'])
		.index('by_modrinth_user', ['modrinthUserId']),
	deviceSessions: defineTable({
		userId: v.id('users'),
		authSessionId: v.id('authSessions'),
		lastSeenAt: v.number(),
		os: v.optional(v.string()),
		platform: v.optional(v.string()),
		userAgent: v.string(),
		city: v.optional(v.string()),
		country: v.optional(v.string()),
		ip: v.optional(v.string()),
	})
		.index('by_user', ['userId'])
		.index('by_auth_session', ['authSessionId']),
	userPreferences: defineTable({
		userId: v.id('users'),
		preferences: userPreferencesValidator,
	}).index('by_user', ['userId']),
	friendRequests: defineTable({
		fromUserId: v.id('users'),
		toUserId: v.id('users'),
		status: friendRequestStatus,
		createdAt: v.number(),
		updatedAt: v.number(),
	})
		.index('by_from_to', ['fromUserId', 'toUserId'])
		.index('by_from', ['fromUserId'])
		.index('by_to', ['toUserId'])
		.index('by_to_status', ['toUserId', 'status'])
		.index('by_from_status', ['fromUserId', 'status']),
	friendships: defineTable({
		userAId: v.id('users'),
		userBId: v.id('users'),
		createdAt: v.number(),
	})
		.index('by_pair', ['userAId', 'userBId'])
		.index('by_user_a', ['userAId'])
		.index('by_user_b', ['userBId']),
	blockedUsers: defineTable({
		blockerUserId: v.id('users'),
		blockedUserId: v.id('users'),
		createdAt: v.number(),
	})
		.index('by_blocker_blocked', ['blockerUserId', 'blockedUserId'])
		.index('by_blocker', ['blockerUserId'])
		.index('by_blocked', ['blockedUserId']),
	socialNotifications: defineTable({
		userId: v.id('users'),
		type: socialNotificationType,
		status: socialNotificationStatus,
		actorUserId: v.optional(v.id('users')),
		clientId: v.optional(v.id('sharedClients')),
		friendRequestId: v.optional(v.id('friendRequests')),
		dedupeKey: v.optional(v.string()),
		createdAt: v.number(),
		updatedAt: v.number(),
	})
		.index('by_user', ['userId'])
		.index('by_actor', ['actorUserId'])
		.index('by_user_status', ['userId', 'status'])
		.index('by_user_dedupe_key', ['userId', 'dedupeKey']),
	sharedClients: defineTable({
		ownerUserId: v.id('users'),
		name: v.string(),
		iconStorageId: v.optional(v.id('_storage')),
		currentVersion: v.optional(v.number()),
		quarantinedAt: v.optional(v.number()),
		deletedAt: v.optional(v.number()),
		createdAt: v.number(),
		updatedAt: v.number(),
	})
		.index('by_owner', ['ownerUserId'])
		.index('by_deleted', ['deletedAt']),
	sharedClientMembers: defineTable({
		clientId: v.id('sharedClients'),
		userId: v.id('users'),
		joinType: v.union(v.literal('owner'), v.literal('invite'), v.literal('link')),
		joinedAt: v.number(),
		lastPlayedAt: v.optional(v.number()),
	})
		.index('by_client_user', ['clientId', 'userId'])
		.index('by_client', ['clientId'])
		.index('by_user', ['userId']),
	sharedClientInvites: defineTable({
		clientId: v.id('sharedClients'),
		createdByUserId: v.id('users'),
		inviteeUserId: v.optional(v.id('users')),
		status: sharedClientInviteStatus,
		maxUses: v.number(),
		uses: v.number(),
		expiresAt: v.number(),
		createdAt: v.number(),
		updatedAt: v.number(),
	})
		.index('by_client_status', ['clientId', 'status'])
		.index('by_invitee_client_status', ['inviteeUserId', 'clientId', 'status'])
		.index('by_invitee_status', ['inviteeUserId', 'status'])
		.index('by_expires_at', ['expiresAt']),
	sharedClientVersions: defineTable({
		clientId: v.id('sharedClients'),
		version: v.number(),
		createdByUserId: v.id('users'),
		modrinthIds: v.array(v.string()),
		externalFiles: v.array(
			v.object({
				fileName: v.string(),
				fileType: v.string(),
				size: v.number(),
				sha1: v.optional(v.string()),
				sha512: v.optional(v.string()),
				uploadToken: v.string(),
				storageId: v.optional(v.id('_storage')),
			}),
		),
		modpackId: v.optional(v.string()),
		gameVersion: v.string(),
		loader: v.string(),
		loaderVersion: v.optional(v.string()),
		ready: v.boolean(),
		createdAt: v.number(),
	})
		.index('by_client_version', ['clientId', 'version'])
		.index('by_client', ['clientId']),
	sharedClientUploads: defineTable({
		versionId: v.id('sharedClientVersions'),
		uploadToken: v.string(),
		fileIndex: v.number(),
		expiresAt: v.number(),
	})
		.index('by_upload_token', ['uploadToken'])
		.index('by_expires_at', ['expiresAt']),
	pairingCores: defineTable({
		code: v.string(),
		coreId: v.string(),
		connectionUrl: v.optional(v.string()),
		status: v.union(v.literal('waiting'), v.literal('claimed'), v.literal('expired')),
		ownerUserId: v.optional(v.id('users')),
		metadata: v.optional(
			v.object({ bindHost: v.optional(v.string()), port: v.optional(v.number()) }),
		),
		createdAt: v.number(),
		expiresAt: v.number(),
		claimedAt: v.optional(v.number()),
		syncCredentialHash: v.optional(v.string()),
		syncCredentialIssuedAt: v.optional(v.number()),
	})
		.index('by_code', ['code'])
		.index('by_core_id', ['coreId'])
		.index('by_expires_at', ['expiresAt'])
		.index('by_status', ['status']),
	coreList: defineTable({
		coreId: v.string(),
		ownerUserId: v.id('users'),
		linkState: v.union(v.literal('unlinked'), v.literal('linked')),
		connectionUrl: v.optional(v.string()),
		setupMode: v.optional(v.union(v.literal('remote'), v.literal('local'))),
		createdAt: v.number(),
		lastSeenAt: v.number(),
		projectionRevision: v.number(),
		syncedAt: v.number(),
		syncCredentialHash: v.optional(v.string()),
	})
		.index('by_core_id', ['coreId'])
		.index('by_owner', ['ownerUserId'])
		.index('by_link_state', ['linkState']),
	coreMemberLinks: defineTable({
		coreId: v.string(),
		userId: v.id('users'),
		isOwner: v.boolean(),
		syncedAt: v.number(),
	})
		.index('by_core_user', ['coreId', 'userId'])
		.index('by_core', ['coreId'])
		.index('by_user', ['userId']),
	realtimeBridgeRequests: defineTable({
		requestId: v.string(),
		expiresAt: v.number(),
	})
		.index('by_request_id', ['requestId'])
		.index('by_expires_at', ['expiresAt']),
})
