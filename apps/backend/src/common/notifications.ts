import type { Notification, SharedInstanceList } from '@amberite/contracts/backend'

import { randomId } from './crypto'
import type { Env } from './types'
import { notifyUser } from '../social/socket'

export interface NotificationRow {
	id: string
	user_id: string
	body: string
	created: string
	read: number
}

export function notificationResponse(row: NotificationRow): Notification {
	// Every notification is stored with a `type`.
	const body: Notification['body'] = JSON.parse(row.body)
	const invite = body.type === 'shared_instance_invite'
	return {
		id: row.id,
		user_id: row.user_id,
		body,
		created: row.created,
		read: Boolean(row.read),
		name: invite ? 'You have been invited to a shared instance!' : '',
		text: invite ? `An invite has been sent for you to join ${body.shared_instance_name}` : '',
		link: invite ? '#' : '',
		actions: invite
			? [
					{ name: 'Accept', action_route: ['POST', ''] },
					{ name: 'Deny', action_route: ['POST', ''] },
				]
			: [],
	}
}

export async function addNotification(
	env: Env,
	userId: string,
	body: Notification['body'],
): Promise<string> {
	const id = randomId()
	const created = new Date().toISOString()
	await env.DB.prepare('INSERT INTO notifications (id,user_id,body,created) VALUES (?,?,?,?)')
		.bind(id, userId, JSON.stringify(body), created)
		.run()
	await notifyUser(
		env,
		userId,
		notificationResponse({ id, user_id: userId, body: JSON.stringify(body), created, read: 0 }),
	).catch((error: unknown) => console.error('Live notification failed', error))
	return id
}

/**
 * The accounts that joined a shared instance, and its owner, other than `except`. A server's share
 * has no member row for its owner, so the owner comes from the share.
 */
export async function joinedMembers(
	env: Env,
	instanceId: string,
	except: string | null = null,
): Promise<string[]> {
	const { results } = await env.DB.prepare(
		'SELECT user_id FROM shared_instance_users WHERE instance_id = ?1 AND joined_at IS NOT NULL AND user_id IS NOT ?2 UNION SELECT owner_id FROM shared_instances WHERE id = ?1 AND owner_id IS NOT ?2',
	)
		.bind(instanceId, except)
		.all<{ user_id: string }>()
	return results.map(({ user_id }) => user_id)
}

/**
 * Tells the online Apps of `users` that a shared instance's `type` list changed, so an open page
 * fetches it again instead of polling. Members who were just removed, or whose share was deleted,
 * are told too: their App finds out it lost access.
 */
export async function notifyUsers(
	env: Env,
	users: readonly string[],
	instanceId: string,
	type: SharedInstanceList,
): Promise<void> {
	await Promise.all(
		users.map((user) =>
			notifyUser(env, user, { body: { type, shared_instance_id: instanceId } }).catch(
				(error: unknown) => console.error('Live event failed', error),
			),
		),
	)
}

/** {@link notifyUsers} for the share's members. `except` already knows: it was their action. */
export async function notifyMembers(
	env: Env,
	instanceId: string,
	type: SharedInstanceList,
	except: string | null = null,
): Promise<void> {
	await notifyUsers(env, await joinedMembers(env, instanceId, except), instanceId, type)
}
