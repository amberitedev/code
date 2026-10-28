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

export function notificationResponse(row: NotificationRow) {
	const body: Record<string, unknown> = JSON.parse(row.body)
	const serverInvite = body.type === 'server_invite'
	const invite = body.type === 'shared_instance_invite' || serverInvite
	return {
		id: row.id,
		user_id: row.user_id,
		body,
		created: row.created,
		read: Boolean(row.read),
		name: serverInvite
			? 'You have been invited to manage a server!'
			: invite
				? 'You have been invited to a shared instance!'
				: '',
		text: serverInvite
			? `An invite has been sent for you to manage ${body.server_name}`
			: invite
				? `An invite has been sent for you to join ${body.shared_instance_name}`
				: '',
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
	body: Record<string, unknown>,
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

/** What a {@link notifyMembers} event says changed. Typed for the App in `SharedInstances.LiveEvent`. */
export type MembersEvent = 'shared_instance_users' | 'shared_instance_servers'

/**
 * Tells the online Apps of a shared instance's members that its member list or hosted servers
 * changed, so an open page fetches it again instead of polling. The event is shaped like a
 * notification so the App's socket passes it on, but it is not stored and has no id. `except` is
 * the account whose own action this was; its App already knows.
 */
export async function notifyMembers(
	env: Env,
	instanceId: string,
	type: MembersEvent,
	except: string | null = null,
): Promise<void> {
	const { results } = await env.DB.prepare(
		'SELECT user_id FROM shared_members WHERE instance_id = ? AND joined_at IS NOT NULL AND user_id IS NOT ?',
	)
		.bind(instanceId, except)
		.all<{ user_id: string }>()
	await Promise.all(
		results.map(({ user_id }) =>
			notifyUser(env, user_id, { body: { type, shared_instance_id: instanceId } }).catch(
				(error: unknown) => console.error('Live event failed', error),
			),
		),
	)
}
