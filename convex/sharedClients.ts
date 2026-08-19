import { v } from 'convex/values'
import { internal } from './_generated/api'
import type { Doc, Id } from './_generated/dataModel'
import { internalMutation, internalQuery, mutation, query } from './_generated/server'
import type { MutationCtx, QueryCtx } from './_generated/server'
import { minecraftUuid, requireUserId } from './_socialRules'
import { dismissNotificationByDedupeKey, upsertSocialNotification } from './_socialNotifications'

const MAX_CLIENT_USERS = 20
const MAX_INVITE_AGE_MS = 7 * 24 * 60 * 60 * 1_000

const clientValidator = v.object({
	name: v.string(),
	icon: v.union(v.string(), v.null()),
	quarantine: v.boolean(),
})
const userValidator = v.object({
	id: v.string(),
	joined_at: v.union(v.string(), v.null()),
	join_type: v.union(v.literal('owner'), v.literal('invite'), v.literal('link')),
	last_played: v.union(v.string(), v.null()),
})
const usersValidator = v.object({ users: v.array(userValidator), tokens: v.number() })
const externalFileValidator = v.object({
	file_name: v.string(),
	file_type: v.string(),
	url: v.string(),
	file_size: v.optional(v.number()),
})
const versionValidator = v.object({
	version: v.number(),
	modrinth_ids: v.array(v.string()),
	ready: v.boolean(),
	external_files: v.array(externalFileValidator),
	modpack_id: v.union(v.string(), v.null()),
	game_version: v.string(),
	loader: v.string(),
	loader_version: v.string(),
})
const inviteValidator = v.object({
	id: v.string(),
	expiration: v.string(),
	max_uses: v.number(),
	uses: v.number(),
})
const inviteInfoValidator = v.object({
	instance_id: v.string(),
	instance_name: v.string(),
	instance_icon: v.union(v.string(), v.null()),
	game_version: v.string(),
	loader_version: v.string(),
	managers: v.array(
		v.object({
			id: v.string(),
			name: v.string(),
			type: v.literal('user'),
			avatar: v.union(v.string(), v.null()),
		}),
	),
	instance_users: v.array(
		v.object({
			id: v.string(),
			name: v.string(),
			avatar: v.union(v.string(), v.null()),
			joined_at: v.union(v.string(), v.null()),
		}),
	),
})
const createVersionArgs = {
	clientId: v.id('sharedClients'),
	modrinthIds: v.array(v.string()),
	externalFiles: v.array(v.object({ fileName: v.string(), fileType: v.string() })),
	modpackId: v.optional(v.string()),
	gameVersion: v.string(),
	loader: v.string(),
	loaderVersion: v.string(),
	origin: v.optional(v.string()),
}

export const create = mutation({
	args: { name: v.string() },
	returns: v.object({ id: v.id('sharedClients') }),
	handler: async (ctx, args) => ({
		id: await createClient(ctx, await requireUserId(ctx), args.name),
	}),
})

export const get = query({
	args: { clientId: v.id('sharedClients') },
	returns: clientValidator,
	handler: async (ctx, args) => clientResponse(ctx, await requireUserId(ctx), args.clientId),
})

export const listMine = query({
	args: {},
	returns: v.array(v.string()),
	handler: async (ctx) => {
		const actorId = await requireUserId(ctx)
		return (
			await ctx.db
				.query('sharedClientMembers')
				.withIndex('by_user', (q) => q.eq('userId', actorId))
				.take(100)
		).map((member) => member.clientId.toString())
	},
})

export const update = mutation({
	args: { clientId: v.id('sharedClients'), name: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		await requireOwner(ctx, await requireUserId(ctx), args.clientId)
		await ctx.db.patch(args.clientId, { name: validName(args.name), updatedAt: Date.now() })
		return null
	},
})

export const remove = mutation({
	args: { clientId: v.id('sharedClients') },
	returns: v.null(),
	handler: async (ctx, args) => {
		await requireOwner(ctx, await requireUserId(ctx), args.clientId)
		await ctx.db.patch(args.clientId, { deletedAt: Date.now(), updatedAt: Date.now() })
		return null
	},
})

export const users = query({
	args: { clientId: v.id('sharedClients'), now: v.number() },
	returns: usersValidator,
	handler: async (ctx, args) =>
		usersResponse(ctx, await requireUserId(ctx), args.clientId, args.now),
})

export const inviteUsers = mutation({
	args: { clientId: v.id('sharedClients'), userIds: v.array(v.id('users')) },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		await inviteClientUsers(ctx, actorId, args.clientId, args.userIds.slice(0, MAX_CLIENT_USERS))
		return null
	},
})

export const removeUsers = mutation({
	args: { clientId: v.id('sharedClients'), userIds: v.array(v.id('users')) },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		await removeClientUsers(ctx, actorId, args.clientId, args.userIds.slice(0, MAX_CLIENT_USERS))
		return null
	},
})

export const acceptPendingInvite = mutation({
	args: { clientId: v.id('sharedClients') },
	returns: v.boolean(),
	handler: async (ctx, args) => acceptPending(ctx, await requireUserId(ctx), args.clientId),
})

export const declinePendingInvite = mutation({
	args: { clientId: v.id('sharedClients') },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const invite = await pendingUserInvite(ctx, actorId, args.clientId)
		if (invite) {
			await ctx.db.patch(invite._id, { status: 'declined', updatedAt: Date.now() })
			await dismissNotificationByDedupeKey(ctx, actorId, `client-invite:${invite._id}`)
		}
		return null
	},
})

export const createInvite = mutation({
	args: { clientId: v.id('sharedClients'), maxAgeSeconds: v.number(), maxUses: v.number() },
	returns: v.object({ id: v.id('sharedClientInvites') }),
	handler: async (ctx, args) => ({
		id: await createLinkInvite(ctx, await requireUserId(ctx), args),
	}),
})

export const listInvites = query({
	args: { clientId: v.id('sharedClients'), now: v.number() },
	returns: v.array(inviteValidator),
	handler: async (ctx, args) =>
		listClientInvites(ctx, await requireUserId(ctx), args.clientId, args.now),
})

export const revokeInvite = mutation({
	args: { clientId: v.id('sharedClients'), inviteId: v.id('sharedClientInvites') },
	returns: v.null(),
	handler: async (ctx, args) => {
		await requireOwner(ctx, await requireUserId(ctx), args.clientId)
		const invite = await ctx.db.get(args.inviteId)
		if (invite?.clientId === args.clientId)
			await ctx.db.patch(invite._id, { status: 'revoked', updatedAt: Date.now() })
		return null
	},
})

export const acceptInvite = mutation({
	args: { clientId: v.id('sharedClients'), inviteId: v.id('sharedClientInvites') },
	returns: v.null(),
	handler: async (ctx, args) => {
		await acceptLinkInvite(ctx, await requireUserId(ctx), args.clientId, args.inviteId)
		return null
	},
})

/** Public by design: the random invite ID is the bearer capability, matching the v1 link contract. */
export const inviteInfo = query({
	args: { inviteId: v.id('sharedClientInvites'), now: v.number() },
	returns: inviteInfoValidator,
	handler: async (ctx, args) => inviteInfoResponse(ctx, args.inviteId, args.now),
})

export const createVersion = mutation({
	args: createVersionArgs,
	returns: versionValidator,
	handler: async (ctx, args) => createClientVersion(ctx, await requireUserId(ctx), args),
})

export const latestVersion = query({
	args: { clientId: v.id('sharedClients') },
	returns: versionValidator,
	handler: async (ctx, args) => versionResponse(ctx, await requireUserId(ctx), args.clientId),
})

export const version = query({
	args: { clientId: v.id('sharedClients'), version: v.number() },
	returns: versionValidator,
	handler: async (ctx, args) =>
		versionResponse(ctx, await requireUserId(ctx), args.clientId, args.version),
})

// HTTP actions use these wrappers so the existing v1 launcher protocol can remain unchanged.
export const httpCreate = internalMutation({
	args: { name: v.string() },
	returns: v.object({ id: v.string() }),
	handler: async (ctx, args) => ({
		id: (await createClient(ctx, await requireUserId(ctx), args.name)).toString(),
	}),
})
export const httpGet = internalQuery({
	args: { clientId: v.id('sharedClients') },
	returns: clientValidator,
	handler: async (ctx, args) => clientResponse(ctx, await requireUserId(ctx), args.clientId),
})
export const httpListMine = internalQuery({
	args: {},
	returns: v.array(v.string()),
	handler: async (ctx) => {
		const actorId = await requireUserId(ctx)
		return (
			await ctx.db
				.query('sharedClientMembers')
				.withIndex('by_user', (q) => q.eq('userId', actorId))
				.take(100)
		).map((row) => row.clientId.toString())
	},
})
export const httpUsers = internalQuery({
	args: { clientId: v.id('sharedClients'), now: v.number() },
	returns: usersValidator,
	handler: async (ctx, args) =>
		usersResponse(ctx, await requireUserId(ctx), args.clientId, args.now),
})
export const httpInviteUsers = internalMutation({
	args: { clientId: v.id('sharedClients'), userIds: v.array(v.string()) },
	returns: v.null(),
	handler: async (ctx, args) => {
		await inviteClientUsers(
			ctx,
			await requireUserId(ctx),
			args.clientId,
			await resolvePublicUserIds(ctx, args.userIds),
		)
		return null
	},
})
export const httpRemoveUsers = internalMutation({
	args: { clientId: v.id('sharedClients'), userIds: v.array(v.string()) },
	returns: v.null(),
	handler: async (ctx, args) => {
		await removeClientUsers(
			ctx,
			await requireUserId(ctx),
			args.clientId,
			await resolvePublicUserIds(ctx, args.userIds),
		)
		return null
	},
})
export const httpAcceptPending = internalMutation({
	args: { clientId: v.id('sharedClients') },
	returns: v.boolean(),
	handler: async (ctx, args) => acceptPending(ctx, await requireUserId(ctx), args.clientId),
})
export const httpCreateInvite = internalMutation({
	args: { clientId: v.id('sharedClients'), maxAgeSeconds: v.number(), maxUses: v.number() },
	returns: v.object({ id: v.string() }),
	handler: async (ctx, args) => ({
		id: (await createLinkInvite(ctx, await requireUserId(ctx), args)).toString(),
	}),
})
export const httpListInvites = internalQuery({
	args: { clientId: v.id('sharedClients'), now: v.number() },
	returns: v.array(inviteValidator),
	handler: async (ctx, args) =>
		listClientInvites(ctx, await requireUserId(ctx), args.clientId, args.now),
})
export const httpInviteInfo = internalQuery({
	args: { inviteId: v.id('sharedClientInvites'), now: v.number() },
	returns: inviteInfoValidator,
	handler: async (ctx, args) => inviteInfoResponse(ctx, args.inviteId, args.now),
})
export const httpCreateVersion = internalMutation({
	args: createVersionArgs,
	returns: versionValidator,
	handler: async (ctx, args) => createClientVersion(ctx, await requireUserId(ctx), args),
})
export const httpLatestVersion = internalQuery({
	args: { clientId: v.id('sharedClients') },
	returns: versionValidator,
	handler: async (ctx, args) => versionResponse(ctx, await requireUserId(ctx), args.clientId),
})
export const httpVersion = internalQuery({
	args: { clientId: v.id('sharedClients'), version: v.number() },
	returns: versionValidator,
	handler: async (ctx, args) =>
		versionResponse(ctx, await requireUserId(ctx), args.clientId, args.version),
})
export const httpUpdate = internalMutation({
	args: { clientId: v.id('sharedClients'), name: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => (
		await requireOwner(ctx, await requireUserId(ctx), args.clientId),
		await ctx.db.patch(args.clientId, { name: validName(args.name), updatedAt: Date.now() }),
		null
	),
})
export const httpDelete = internalMutation({
	args: { clientId: v.id('sharedClients') },
	returns: v.null(),
	handler: async (ctx, args) => (
		await requireOwner(ctx, await requireUserId(ctx), args.clientId),
		await ctx.db.patch(args.clientId, { deletedAt: Date.now(), updatedAt: Date.now() }),
		null
	),
})
export const httpDeclinePending = internalMutation({
	args: { clientId: v.id('sharedClients') },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actor = await requireUserId(ctx)
		const invite = await pendingUserInvite(ctx, actor, args.clientId)
		if (invite) await ctx.db.patch(invite._id, { status: 'declined', updatedAt: Date.now() })
		return null
	},
})
export const httpAcceptInvite = internalMutation({
	args: { clientId: v.id('sharedClients'), inviteId: v.id('sharedClientInvites') },
	returns: v.null(),
	handler: async (ctx, args) => (
		await acceptLinkInvite(ctx, await requireUserId(ctx), args.clientId, args.inviteId),
		null
	),
})
export const httpRevokeInvite = internalMutation({
	args: { clientId: v.id('sharedClients'), inviteId: v.id('sharedClientInvites') },
	returns: v.null(),
	handler: async (ctx, args) => {
		await requireOwner(ctx, await requireUserId(ctx), args.clientId)
		const invite = await ctx.db.get(args.inviteId)
		if (invite?.clientId === args.clientId)
			await ctx.db.patch(invite._id, { status: 'revoked', updatedAt: Date.now() })
		return null
	},
})
export const attachExternalFile = internalMutation({
	args: { uploadToken: v.string(), storageId: v.id('_storage'), size: v.number() },
	returns: v.boolean(),
	handler: async (ctx, args) => {
		const upload = await ctx.db
			.query('sharedClientUploads')
			.withIndex('by_upload_token', (q) => q.eq('uploadToken', args.uploadToken))
			.unique()
		if (!upload || upload.expiresAt <= Date.now()) return false
		const version = await ctx.db.get(upload.versionId)
		if (!version) return false
		const files = version.externalFiles.map((file, index) =>
			index === upload.fileIndex ? { ...file, storageId: args.storageId, size: args.size } : file,
		)
		const ready = files.every((file) => file.storageId)
		await ctx.db.patch(version._id, { externalFiles: files, ready })
		await ctx.db.delete(upload._id)
		if (ready)
			await ctx.db.patch(version.clientId, {
				currentVersion: version.version,
				updatedAt: Date.now(),
			})
		return true
	},
})
export const setIcon = internalMutation({
	args: { clientId: v.id('sharedClients'), storageId: v.union(v.id('_storage'), v.null()) },
	returns: v.null(),
	handler: async (ctx, args) => {
		await requireOwner(ctx, await requireUserId(ctx), args.clientId)
		const client = await ctx.db.get(args.clientId)
		if (!client) throw new Error('client not found')
		if (client.iconStorageId) await ctx.storage.delete(client.iconStorageId)
		await ctx.db.patch(args.clientId, {
			iconStorageId: args.storageId ?? undefined,
			updatedAt: Date.now(),
		})
		return null
	},
})

async function createClient(ctx: MutationCtx, actorId: Id<'users'>, name: string) {
	const now = Date.now()
	const clientId = await ctx.db.insert('sharedClients', {
		ownerUserId: actorId,
		name: validName(name),
		createdAt: now,
		updatedAt: now,
	})
	await ctx.db.insert('sharedClientMembers', {
		clientId,
		userId: actorId,
		joinType: 'owner',
		joinedAt: now,
	})
	return clientId
}

async function clientResponse(ctx: QueryCtx, actorId: Id<'users'>, clientId: Id<'sharedClients'>) {
	const client = await requireMember(ctx, actorId, clientId)
	return {
		name: client.name,
		icon: client.iconStorageId ? await ctx.storage.getUrl(client.iconStorageId) : null,
		quarantine: Boolean(client.quarantinedAt),
	}
}

async function usersResponse(
	ctx: QueryCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	now: number,
) {
	await requireMember(ctx, actorId, clientId)
	const [members, invites] = await Promise.all([
		ctx.db
			.query('sharedClientMembers')
			.withIndex('by_client', (q) => q.eq('clientId', clientId))
			.take(MAX_CLIENT_USERS),
		ctx.db
			.query('sharedClientInvites')
			.withIndex('by_client_status', (q) => q.eq('clientId', clientId).eq('status', 'pending'))
			.take(MAX_CLIENT_USERS),
	])
	const pending = invites.filter(
		(invite) =>
			invite.inviteeUserId &&
			invite.expiresAt > now &&
			!members.some((member) => member.userId === invite.inviteeUserId),
	)
	const memberUsers = await Promise.all(members.map((member) => ctx.db.get(member.userId)))
	const pendingUsers = await Promise.all(pending.map((invite) => ctx.db.get(invite.inviteeUserId!)))
	return {
		users: [
			...members.map((member, index) => ({
				id: memberUsers[index] ? minecraftUuid(memberUsers[index]!) : member.userId.toString(),
				joined_at: new Date(member.joinedAt).toISOString(),
				join_type: member.joinType,
				last_played: member.lastPlayedAt ? new Date(member.lastPlayedAt).toISOString() : null,
			})),
			...pending.map((invite, index) => ({
				id: pendingUsers[index]
					? minecraftUuid(pendingUsers[index]!)
					: invite.inviteeUserId!.toString(),
				joined_at: null,
				join_type: 'invite' as const,
				last_played: null,
			})),
		],
		tokens: Math.max(0, MAX_CLIENT_USERS - members.length - pending.length),
	}
}

async function inviteClientUsers(
	ctx: MutationCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	userIds: Id<'users'>[],
) {
	await requireOwner(ctx, actorId, clientId)
	const current = await ctx.db
		.query('sharedClientMembers')
		.withIndex('by_client', (q) => q.eq('clientId', clientId))
		.take(MAX_CLIENT_USERS)
	if (new Set([...current.map((row) => row.userId), ...userIds]).size > MAX_CLIENT_USERS)
		throw new Error('client user limit reached')
	for (const userId of [...new Set(userIds)]) {
		if (userId === actorId || current.some((member) => member.userId === userId)) continue
		const [outgoingBlock, incomingBlock] = await Promise.all([
			block(ctx, actorId, userId),
			block(ctx, userId, actorId),
		])
		if (outgoingBlock || incomingBlock) continue
		const now = Date.now()
		const existing = await pendingUserInvite(ctx, userId, clientId)
		const inviteId =
			existing?._id ??
			(await ctx.db.insert('sharedClientInvites', {
				clientId,
				createdByUserId: actorId,
				inviteeUserId: userId,
				status: 'pending',
				maxUses: 1,
				uses: 0,
				expiresAt: now + MAX_INVITE_AGE_MS,
				createdAt: now,
				updatedAt: now,
			}))
		if (existing)
			await ctx.db.patch(existing._id, {
				status: 'pending',
				expiresAt: now + MAX_INVITE_AGE_MS,
				updatedAt: now,
			})
		await upsertSocialNotification(ctx, {
			userId,
			type: 'client_invite',
			actorUserId: actorId,
			clientId,
			dedupeKey: `client-invite:${inviteId}`,
		})
	}
}

async function resolvePublicUserIds(ctx: QueryCtx | MutationCtx, values: string[]) {
	const users: Id<'users'>[] = []
	for (const value of [...new Set(values)].slice(0, MAX_CLIENT_USERS)) {
		const input = value.trim().replace(/^@/, '')
		const uuid = input.replace(/-/g, '').toLowerCase()
		const byUuid = await ctx.db
			.query('users')
			.withIndex('by_minecraft_uuid', (index) => index.eq('minecraftUuid', uuid))
			.unique()
		const user =
			byUuid ??
			(await ctx.db
				.query('users')
				.withIndex('by_normalized_username', (index) =>
					index.eq('normalizedUsername', input.toLowerCase()),
				)
				.unique())
		if (!user || user.deletedAt) throw new Error(`user not found: ${value}`)
		users.push(user._id)
	}
	return users
}

async function removeClientUsers(
	ctx: MutationCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	userIds: Id<'users'>[],
) {
	await requireOwner(ctx, actorId, clientId)
	for (const userId of userIds) {
		if (userId === actorId) continue
		const membership = await member(ctx, clientId, userId)
		if (membership) await ctx.db.delete(membership._id)
		const invite = await pendingUserInvite(ctx, userId, clientId)
		if (invite) await ctx.db.patch(invite._id, { status: 'revoked', updatedAt: Date.now() })
		await upsertSocialNotification(ctx, {
			userId,
			type: 'client_access_revoked',
			actorUserId: actorId,
			clientId,
			dedupeKey: `client-revoked:${clientId}:${userId}`,
		})
	}
}

async function acceptPending(
	ctx: MutationCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
) {
	if (await member(ctx, clientId, actorId)) return true
	const invite = await pendingUserInvite(ctx, actorId, clientId)
	if (!invite || invite.expiresAt <= Date.now()) return false
	await addMember(ctx, actorId, clientId, 'invite')
	await ctx.db.patch(invite._id, { status: 'accepted', uses: 1, updatedAt: Date.now() })
	await dismissNotificationByDedupeKey(ctx, actorId, `client-invite:${invite._id}`)
	return true
}

async function createLinkInvite(
	ctx: MutationCtx,
	actorId: Id<'users'>,
	args: { clientId: Id<'sharedClients'>; maxAgeSeconds: number; maxUses: number },
) {
	await requireOwner(ctx, actorId, args.clientId)
	if (
		args.maxAgeSeconds <= 0 ||
		args.maxAgeSeconds * 1_000 > MAX_INVITE_AGE_MS ||
		args.maxUses <= 0
	)
		throw new Error('invalid invite limits')
	const now = Date.now()
	return await ctx.db.insert('sharedClientInvites', {
		clientId: args.clientId,
		createdByUserId: actorId,
		status: 'pending',
		maxUses: args.maxUses,
		uses: 0,
		expiresAt: now + args.maxAgeSeconds * 1_000,
		createdAt: now,
		updatedAt: now,
	})
}

async function listClientInvites(
	ctx: QueryCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	now: number,
) {
	await requireOwner(ctx, actorId, clientId)
	const invites = await ctx.db
		.query('sharedClientInvites')
		.withIndex('by_client_status', (q) => q.eq('clientId', clientId).eq('status', 'pending'))
		.take(100)
	return invites
		.filter((invite) => !invite.inviteeUserId && invite.expiresAt > now)
		.map((invite) => ({
			id: invite._id.toString(),
			expiration: new Date(invite.expiresAt).toISOString(),
			max_uses: invite.maxUses,
			uses: invite.uses,
		}))
}

async function acceptLinkInvite(
	ctx: MutationCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	inviteId: Id<'sharedClientInvites'>,
) {
	if (await member(ctx, clientId, actorId)) return
	const invite = await ctx.db.get(inviteId)
	if (
		!invite ||
		invite.clientId !== clientId ||
		invite.inviteeUserId ||
		invite.status !== 'pending' ||
		invite.expiresAt <= Date.now() ||
		invite.uses >= invite.maxUses
	)
		throw new Error('invite not found')
	await addMember(ctx, actorId, clientId, 'link')
	const uses = invite.uses + 1
	await ctx.db.patch(invite._id, {
		uses,
		status: uses >= invite.maxUses ? 'accepted' : 'pending',
		updatedAt: Date.now(),
	})
}

async function inviteInfoResponse(ctx: QueryCtx, inviteId: Id<'sharedClientInvites'>, now: number) {
	const invite = await ctx.db.get(inviteId)
	if (
		!invite ||
		invite.status !== 'pending' ||
		invite.expiresAt <= now ||
		invite.uses >= invite.maxUses
	)
		throw new Error('invite not found')
	const client = await ctx.db.get(invite.clientId)
	if (!client || client.deletedAt) throw new Error('client not found')
	const [owner, members, latest] = await Promise.all([
		ctx.db.get(client.ownerUserId),
		ctx.db
			.query('sharedClientMembers')
			.withIndex('by_client', (q) => q.eq('clientId', client._id))
			.take(MAX_CLIENT_USERS),
		client.currentVersion === undefined
			? null
			: ctx.db
					.query('sharedClientVersions')
					.withIndex('by_client_version', (q) =>
						q.eq('clientId', client._id).eq('version', client.currentVersion!),
					)
					.unique(),
	])
	if (!owner || !latest) throw new Error('client is not ready')
	const users = await Promise.all(members.map((row) => ctx.db.get(row.userId)))
	return {
		instance_id: client._id.toString(),
		instance_name: client.name,
		instance_icon: client.iconStorageId ? await ctx.storage.getUrl(client.iconStorageId) : null,
		game_version: latest.gameVersion,
		loader_version: latest.loaderVersion ?? '',
		managers: [
			{
				id: minecraftUuid(owner),
				name: owner.displayName ?? owner.verifiedMinecraftHandle ?? 'Owner',
				type: 'user' as const,
				avatar: owner.avatarUrl ?? owner.image ?? null,
			},
		],
		instance_users: users
			.filter((user): user is Doc<'users'> => Boolean(user))
			.map((user) => {
				const membership = members.find((row) => row.userId === user._id)
				return {
					id: minecraftUuid(user),
					name: user.displayName ?? user.verifiedMinecraftHandle ?? 'User',
					avatar: user.avatarUrl ?? user.image ?? null,
					joined_at: membership ? new Date(membership.joinedAt).toISOString() : null,
				}
			}),
	}
}

async function createClientVersion(
	ctx: MutationCtx,
	actorId: Id<'users'>,
	args: {
		clientId: Id<'sharedClients'>
		modrinthIds: string[]
		externalFiles: { fileName: string; fileType: string }[]
		modpackId?: string
		gameVersion: string
		loader: string
		loaderVersion: string
		origin?: string
	},
) {
	await requireOwner(ctx, actorId, args.clientId)
	if (args.modrinthIds.length > 2_000 || args.externalFiles.length > 500)
		throw new Error('client version contains too many files')
	const latest = await ctx.db
		.query('sharedClientVersions')
		.withIndex('by_client', (q) => q.eq('clientId', args.clientId))
		.order('desc')
		.first()
	const version = (latest?.version ?? 0) + 1
	const externalFiles = args.externalFiles.map((file) => ({
		...file,
		size: 0,
		uploadToken: crypto.randomUUID(),
	}))
	const ready = externalFiles.length === 0
	const row = await ctx.db.insert('sharedClientVersions', {
		clientId: args.clientId,
		version,
		createdByUserId: actorId,
		modrinthIds: [...new Set(args.modrinthIds)],
		externalFiles,
		modpackId: args.modpackId,
		gameVersion: args.gameVersion,
		loader: args.loader,
		loaderVersion: args.loaderVersion,
		ready,
		createdAt: Date.now(),
	})
	for (const [fileIndex, file] of externalFiles.entries()) {
		await ctx.db.insert('sharedClientUploads', {
			versionId: row,
			uploadToken: file.uploadToken,
			fileIndex,
			expiresAt: Date.now() + 60 * 60 * 1_000,
		})
	}
	if (ready) await ctx.db.patch(args.clientId, { currentVersion: version, updatedAt: Date.now() })
	const document = await ctx.db.get(row)
	if (!document) throw new Error('version not found')
	return versionDocumentResponse(document, args.origin)
}

async function versionResponse(
	ctx: QueryCtx,
	actorId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	requested?: number,
) {
	const client = await requireMember(ctx, actorId, clientId)
	const version = requested ?? client.currentVersion
	if (version === undefined) throw new Error('version not found')
	const row = await ctx.db
		.query('sharedClientVersions')
		.withIndex('by_client_version', (q) => q.eq('clientId', clientId).eq('version', version))
		.unique()
	if (!row) throw new Error('version not found')
	const urls = await Promise.all(
		row.externalFiles.map((file) => (file.storageId ? ctx.storage.getUrl(file.storageId) : null)),
	)
	return versionDocumentResponse(row, undefined, urls)
}

function versionDocumentResponse(
	version: Doc<'sharedClientVersions'>,
	origin?: string,
	downloadUrls?: (string | null)[],
) {
	return {
		version: version.version,
		modrinth_ids: version.modrinthIds,
		ready: version.ready,
		external_files: version.externalFiles.map((file, index) => ({
			file_name: file.fileName,
			file_type: file.fileType,
			url: downloadUrls?.[index] ?? `${origin ?? ''}/v1/uploads/${file.uploadToken}`,
			...(file.size ? { file_size: file.size } : {}),
		})),
		modpack_id: version.modpackId ?? null,
		game_version: version.gameVersion,
		loader: version.loader,
		loader_version: version.loaderVersion ?? '',
	}
}

async function addMember(
	ctx: MutationCtx,
	userId: Id<'users'>,
	clientId: Id<'sharedClients'>,
	joinType: 'invite' | 'link',
) {
	const count = await ctx.db
		.query('sharedClientMembers')
		.withIndex('by_client', (q) => q.eq('clientId', clientId))
		.take(MAX_CLIENT_USERS)
	if (count.length >= MAX_CLIENT_USERS) throw new Error('client user limit reached')
	await ctx.db.insert('sharedClientMembers', { clientId, userId, joinType, joinedAt: Date.now() })
}

async function requireMember(
	ctx: QueryCtx | MutationCtx,
	userId: Id<'users'>,
	clientId: Id<'sharedClients'>,
) {
	const [client, membership] = await Promise.all([
		ctx.db.get(clientId),
		member(ctx, clientId, userId),
	])
	if (!client || client.deletedAt) throw new Error('client not found')
	if (!membership) throw new Error('not authorized')
	return client
}

async function requireOwner(
	ctx: QueryCtx | MutationCtx,
	userId: Id<'users'>,
	clientId: Id<'sharedClients'>,
) {
	const client = await requireMember(ctx, userId, clientId)
	if (client.ownerUserId !== userId) throw new Error('not authorized')
	return client
}

function member(ctx: QueryCtx | MutationCtx, clientId: Id<'sharedClients'>, userId: Id<'users'>) {
	return ctx.db
		.query('sharedClientMembers')
		.withIndex('by_client_user', (q) => q.eq('clientId', clientId).eq('userId', userId))
		.unique()
}

function pendingUserInvite(
	ctx: QueryCtx | MutationCtx,
	userId: Id<'users'>,
	clientId: Id<'sharedClients'>,
) {
	return ctx.db
		.query('sharedClientInvites')
		.withIndex('by_invitee_client_status', (q) =>
			q.eq('inviteeUserId', userId).eq('clientId', clientId).eq('status', 'pending'),
		)
		.unique()
}

function block(ctx: QueryCtx | MutationCtx, blocker: Id<'users'>, blocked: Id<'users'>) {
	return ctx.db
		.query('blockedUsers')
		.withIndex('by_blocker_blocked', (q) =>
			q.eq('blockerUserId', blocker).eq('blockedUserId', blocked),
		)
		.unique()
}

function validName(name: string) {
	const value = name.trim()
	if (!value || value.length > 64)
		throw new Error('client name must be between 1 and 64 characters')
	return value
}
