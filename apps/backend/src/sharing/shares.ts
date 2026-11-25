import { ALLOWANCE_BYTES } from '@amberite/contracts/protocol'
import { type SharedInstanceUpdate, sharing } from '@amberite/contracts/backend'

import { requireUser } from '../common/auth'
import { ApiError, invalid } from '../common/http'
import { IMAGE_HEADERS, imageType } from '../common/images'
import { joinedMembers, notifyUsers } from '../common/notifications'
import { noBody, rawBody, route } from '../common/router'
import type { Env } from '../common/types'
import {
	type InstanceRow,
	PLAYERS,
	iconConfig,
	now,
	owned,
	owner,
	readable,
	sharer,
} from './access'
import { parseNewShare, parseUpdate, validIconConfig } from './parse'
import { manifestKey, ownerShares } from './pool'
import { cameThrough, joinAddress } from './servers'

// A share itself: creating, naming, deleting it and its icon, and what it stores.

/**
 * Deletes the instance's manifests that no remaining version row uses. Called after the rows are
 * gone, so it clears everything once the instance itself is deleted. The Cores holding the files
 * learn what to drop at their next storage report.
 */
export async function sweepFiles(env: Env, id: string): Promise<void> {
	const stored: string[] = []
	let cursor: string | undefined
	do {
		const page = await env.FILES.list({ prefix: `manifests/${id}/`, cursor })
		stored.push(...page.objects.map((object) => object.key))
		cursor = page.truncated ? page.cursor : undefined
	} while (cursor)
	const used = new Set(
		(
			await env.DB.prepare('SELECT version FROM shared_instance_versions WHERE instance_id = ?')
				.bind(id)
				.all<{ version: number }>()
		).results.map((row) => manifestKey(id, row.version)),
	)
	const unused = stored.filter((key) => !used.has(key))
	if (unused.length) await env.FILES.delete(unused)
	const icons = await env.FILES.list({ prefix: `icons/${id}/` })
	if (icons.objects.length) await env.FILES.delete(icons.objects.map((object) => object.key))
}

export async function deleteShare(env: Env, id: string): Promise<void> {
	const { results: players } = await env.DB.prepare(
		`SELECT DISTINCT p.user_id FROM ${PLAYERS} WHERE s.instance_id = ? AND p.joined_at IS NOT NULL`,
	)
		.bind(id)
		.all<{ user_id: string }>()
	const told = new Set([...(await joinedMembers(env, id)), ...players.map((p) => p.user_id)])
	await env.DB.prepare('DELETE FROM shared_instances WHERE id = ?').bind(id).run()
	await notifyUsers(env, [...told], id, 'shared_instance_users')
	await sweepFiles(env, id)
}

export async function renameShare(
	env: Env,
	id: string,
	update: SharedInstanceUpdate,
): Promise<void> {
	await env.DB.prepare('UPDATE shared_instances SET name = ? WHERE id = ?')
		.bind(update.name, id)
		.run()
}

export const ICON_BYTES = 256 * 1024

/** Where an uploaded icon is kept in R2. Under the share, so `sweepFiles` clears them with it. */
const iconKey = (share: string, upload: string) => `icons/${share}/${upload}`

/** Replaces the share's icon with an uploaded image's `url` or icon editor `config`, and drops
 * the image it had. */
export async function setIcon(
	env: Env,
	item: InstanceRow,
	url: string | null,
	config: string | null,
) {
	await env.DB.prepare('UPDATE shared_instances SET icon = ?,icon_config = ? WHERE id = ?')
		.bind(url, config, item.id)
		.run()
	const upload = item.icon?.split('/').pop()
	if (upload) await env.FILES.delete(iconKey(item.id, upload))
}

/** Replaces the share's icon with `image`, served from this backend as the image its bytes are. */
export async function uploadIcon(request: Request, env: Env, item: InstanceRow, image: Blob) {
	const bytes = new Uint8Array(await image.arrayBuffer())
	const contentType = imageType(bytes) ?? invalid('Icons must be PNG, JPEG, GIF or WebP images')
	const upload = crypto.randomUUID()
	await env.FILES.put(iconKey(item.id, upload), bytes, { httpMetadata: { contentType } })
	await setIcon(env, item, `${new URL(request.url).origin}/v1/icons/${item.id}/${upload}`, null)
}

/** Anyone may fetch an icon, signed in or not: invite previews show it before anyone signs in. */
export const publicShareRoutes = [
	route(sharing.GetIcon, noBody, async ({ env, params }) => {
		const object = await env.FILES.get(iconKey(params.id, params.upload))
		if (!object) throw new ApiError(404, 'icon_not_found', 'Icon was not found')
		return new Response(object.body, {
			headers: {
				'Content-Type': object.httpMetadata?.contentType ?? 'image/png',
				...IMAGE_HEADERS,
			},
		})
	}),
]

export const shareRoutes = [
	route(sharing.ListSharedInstances, noBody, async ({ request, env }) => {
		const user = await sharer(request, env)
		const query = new URL(request.url).searchParams
		if (query.has('user') && query.get('user') !== user.id)
			throw new ApiError(403, 'not_own_list', 'Private instance lists belong to their account')
		// The caller's own shares, and with `include_joined` every share they can play too.
		const sql =
			query.get('include_joined') === 'true'
				? `SELECT instance_id FROM shared_instance_users WHERE user_id = ?1 AND joined_at IS NOT NULL UNION SELECT s.instance_id FROM ${PLAYERS} WHERE p.user_id = ?1 AND p.joined_at IS NOT NULL`
				: 'SELECT id AS instance_id FROM shared_instances WHERE owner_id = ?1'
		return (await env.DB.prepare(sql).bind(user.id).all<{ instance_id: string }>()).results.map(
			(row) => row.instance_id,
		)
	}),
	route(sharing.CreateSharedInstance, parseNewShare, async ({ request, env, body }) => {
		const user = await sharer(request, env)
		const { name } = await body()
		const instanceId = crypto.randomUUID()
		await env.DB.batch([
			env.DB.prepare(
				'INSERT INTO shared_instances (id,owner_id,name,created) VALUES (?,?,?,?)',
			).bind(instanceId, user.id, name, now()),
			env.DB.prepare(
				"INSERT INTO shared_instance_users (instance_id,user_id,joined_at,join_type) VALUES (?,?,?,'owner')",
			).bind(instanceId, user.id, now()),
		])
		return { id: instanceId }
	}),
	route(sharing.GetSharedInstance, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		const server = await cameThrough(env, item, user.id)
		return {
			name: item.name,
			icon: item.icon,
			icon_config: iconConfig(item),
			quarantine: item.quarantine === 1,
			installed_once: item.installed_once === 1,
			// The App adds it to the instance's server list.
			linked_server: server
				? { domain: joinAddress(server), region: '', online_status: null }
				: null,
		}
	}),
	route(sharing.UpdateSharedInstance, parseUpdate, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		await owned(env, params.id, user.id)
		await renameShare(env, params.id, await body())
	}),
	route(sharing.DeleteSharedInstance, noBody, async ({ request, env, params }) => {
		// An owner may always delete their share, locked or not.
		const user = await requireUser(request, env)
		await owned(env, params.id, user.id)
		await deleteShare(env, params.id)
	}),
	route(
		sharing.SetIcon,
		rawBody,
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			const item = await readable(env, params.id, user.id)
			owner(item, user.id)
			await uploadIcon(request, env, item, await body())
		},
		ICON_BYTES,
	),
	route(sharing.SetGeneratedIcon, validIconConfig, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		owner(item, user.id)
		await setIcon(env, item, null, JSON.stringify(await body()))
	}),
	route(sharing.RemoveIcon, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		owner(item, user.id)
		await setIcon(env, item, null, null)
	}),
	route(sharing.GetShareStorage, noBody, async ({ request, env }) => {
		const shares = await ownerShares(env, (await requireUser(request, env)).id)
		return {
			used: shares.reduce((sum, share) => sum + share.used, 0),
			allowance: ALLOWANCE_BYTES,
			shares,
		}
	}),
]
