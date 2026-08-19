import type { Id } from './_generated/dataModel'
import type { MutationCtx, QueryCtx } from './_generated/server'
import { minecraftUuid } from './_socialRules'

export type SocialNotificationType =
	| 'friend_request'
	| 'friend_request_accepted'
	| 'client_invite'
	| 'client_access_revoked'
	| 'client_update'

export async function upsertSocialNotification(
	ctx: MutationCtx,
	args: {
		userId: Id<'users'>
		type: SocialNotificationType
		actorUserId?: Id<'users'>
		clientId?: Id<'sharedClients'>
		friendRequestId?: Id<'friendRequests'>
		dedupeKey: string
	},
) {
	const existing = await ctx.db
		.query('socialNotifications')
		.withIndex('by_user_dedupe_key', (q) =>
			q.eq('userId', args.userId).eq('dedupeKey', args.dedupeKey),
		)
		.unique()
	const now = Date.now()
	const value = {
		type: args.type,
		status: 'unread' as const,
		actorUserId: args.actorUserId,
		clientId: args.clientId,
		friendRequestId: args.friendRequestId,
		updatedAt: now,
	}
	if (existing) {
		await ctx.db.patch(existing._id, value)
		return existing._id
	}
	return await ctx.db.insert('socialNotifications', {
		userId: args.userId,
		dedupeKey: args.dedupeKey,
		createdAt: now,
		...value,
	})
}

export async function dismissNotificationByDedupeKey(
	ctx: MutationCtx,
	userId: Id<'users'>,
	dedupeKey: string,
) {
	const notification = await ctx.db
		.query('socialNotifications')
		.withIndex('by_user_dedupe_key', (q) => q.eq('userId', userId).eq('dedupeKey', dedupeKey))
		.unique()
	if (notification && notification.status !== 'dismissed') {
		await ctx.db.patch(notification._id, { status: 'dismissed', updatedAt: Date.now() })
	}
}

export async function notificationToLabrinth(
	ctx: QueryCtx | MutationCtx,
	notification: {
		_id: Id<'socialNotifications'>
		userId: Id<'users'>
		type: SocialNotificationType
		status: 'unread' | 'read' | 'dismissed'
		actorUserId?: Id<'users'>
		clientId?: Id<'sharedClients'>
		createdAt: number
	},
) {
	const actor = notification.actorUserId ? await ctx.db.get(notification.actorUserId) : null
	const recipient = await ctx.db.get(notification.userId)
	if (!recipient) throw new Error('notification recipient not found')
	const client = notification.clientId ? await ctx.db.get(notification.clientId) : null
	const actorName = actor?.displayName ?? actor?.verifiedMinecraftHandle ?? 'Someone'
	const actorId = actor?.minecraftUuid
	const clientName = client?.name ?? 'a client profile'
	const base = {
		id: notification._id.toString(),
		user_id: minecraftUuid(recipient),
		read: notification.status !== 'unread',
		created: new Date(notification.createdAt).toISOString(),
		actions: [] as { title: string; action_route: [string, string] }[],
	}

	if (notification.type === 'client_invite' && client) {
		return {
			...base,
			type: 'shared_instance_invite',
			title: 'Client profile invite',
			name: 'Client profile invite',
			text: `${actorName} invited you to ${clientName}.`,
			link: '/',
			body: {
				type: 'shared_instance_invite',
				shared_instance_id: client._id.toString(),
				shared_instance_name: client.name,
				shared_instance_icon: null,
				invited_by: actorId,
			},
		}
	}

	const copy = notificationCopy(notification.type, actorName, clientName)
	return {
		...base,
		type: 'custom',
		title: copy.title,
		name: copy.title,
		text: copy.text,
		link: copy.link,
		body: {
			type: 'custom',
			key: `amberite_${notification.type}`,
			actor_user_id: actorId,
			shared_instance_id: notification.clientId?.toString(),
		},
	}
}

function notificationCopy(type: SocialNotificationType, actor: string, client: string) {
	if (type === 'friend_request')
		return {
			title: 'Friend request',
			text: `${actor} sent you a friend request.`,
			link: '/friends',
		}
	if (type === 'friend_request_accepted')
		return {
			title: 'Friend request accepted',
			text: `${actor} accepted your friend request.`,
			link: '/friends',
		}
	if (type === 'client_access_revoked')
		return {
			title: 'Client access removed',
			text: `Your access to ${client} was removed.`,
			link: '/',
		}
	return { title: 'Client profile updated', text: `${client} has a new version.`, link: '/' }
}
