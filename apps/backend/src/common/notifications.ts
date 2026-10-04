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
