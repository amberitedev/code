import { core as coreRoutes, type Server } from '@amberite/contracts/core'
import type { AbstractModrinthClient, ContractClient, SharedInstances } from '@modrinth/api-client'

import { type ShareUnlinkPrompt, unlinkBeforeShare } from './unlink-before-share'

// The invite rules for instance and server shares alike; the App's share pages import them too.
// The age and default uses mirror SHARED_INSTANCE_INVITE_MAX_AGE_SECONDS and
// SHARED_INSTANCE_INVITE_MAX_USES in packages/app-lib/src/api/instance/shared/invites.rs (upstream),
// which refuses a longer age for the App's own shares. The player cap is upstream's
// SHARED_INSTANCE_USER_LIMIT; nothing on the backend enforces it yet.
export const SHARED_INSTANCE_MAX_PLAYERS = 50
export const INVITE_LINK_MAX_AGE_SECONDS = 7 * 24 * 60 * 60
export const DEFAULT_INVITE_LINK_MAX_USES = 10

export function remainingPlayerSlots(playerCount: number) {
	return Math.max(0, SHARED_INSTANCE_MAX_PLAYERS - playerCount)
}

/**
 * The page an invite link opens. Every invite URL, for instance and server shares alike, is built
 * here; `siteUrl` is the site the App passes, its account site (the App's `config.accountSiteUrl`).
 */
export function inviteLinkUrl(siteUrl: string, inviteId: string) {
	return `${siteUrl.replace(/\/$/, '')}/share/${encodeURIComponent(inviteId)}`
}

/** The lifetime of an edited invite link: until `expiresAt`, within what a link may last. */
export function inviteLinkMaxAge(expiresAt: Date) {
	const requested = Math.floor((expiresAt.getTime() - Date.now()) / 1000)
	return Math.max(1, Math.min(INVITE_LINK_MAX_AGE_SECONDS, requested))
}

/** The uses of an edited invite link: what was asked for, within the open player slots. */
export function inviteLinkMaxUses(requested: number, remainingSlots: number) {
	return Math.max(1, Math.min(requested, Math.floor(remainingSlots)))
}

export function findActiveInviteLink(
	links: SharedInstances.Invites.v1.InviteLink[] | null | undefined,
) {
	return links?.find(
		(link) => new Date(link.expiration).getTime() > Date.now() && link.uses < link.max_uses,
	)
}

export async function ensureServerInviteLink(
	client: AbstractModrinthClient,
	serverId: string,
	instanceId: string,
	remainingSlots?: number,
	options: { maximizeUses?: boolean } = {},
): Promise<string> {
	const players = client.sharedinstances.players_v1
	const active = findActiveInviteLink(await players.links(serverId, instanceId))
	if (active) return active.id

	let availableSlots = remainingSlots
	if (availableSlots === undefined) {
		const listed = await players.list(serverId, instanceId)
		availableSlots = remainingPlayerSlots(listed.users.length)
	}
	if (availableSlots === 0) throw new Error('This shared instance has no available player slots.')

	const invite = await players.createLink(serverId, instanceId, {
		max_age: INVITE_LINK_MAX_AGE_SECONDS,
		max_uses: options.maximizeUses
			? availableSlots
			: Math.min(DEFAULT_INVITE_LINK_MAX_USES, availableSlots),
	})
	return invite.id
}

/**
 * Shares the server when it has no shared instance yet, unlinking an uploaded modpack first through
 * `shareUnlink`, then returns a share URL for an invite link that still has uses left. Returns
 * `null` once `isCurrent` reports the caller moved on. With `afterInstall`, it first waits for the
 * server's install to end: an uploaded pack is linked as it installs, and unlinked only after.
 */
export async function createServerInviteLink(
	client: AbstractModrinthClient,
	serverId: string,
	siteUrl: string,
	options: {
		isCurrent: () => boolean
		shareUnlink: ShareUnlinkPrompt | null
		afterInstall?: boolean
	},
): Promise<string | null> {
	const { isCurrent } = options
	const core = await client.cores.serverRoutes(serverId)
	const params = { id: serverId }
	const server = options.afterInstall
		? await installedServer(client, core, serverId)
		: await core.call(coreRoutes.GetServer, { params })
	if (!isCurrent()) return null
	let instanceId = server.shared_instance_id
	if (!instanceId) {
		await unlinkBeforeShare(core, server, options.shareUnlink)
		if (!isCurrent()) return null
		instanceId = (
			await core.call(coreRoutes.ShareServer, {
				params,
				body: { file_paths: [], removed_paths: [], restart: false },
			})
		).shared_instance_id
	}
	if (!isCurrent()) return null
	const inviteId = await ensureServerInviteLink(client, serverId, instanceId, undefined, {
		maximizeUses: true,
	})
	return inviteLinkUrl(siteUrl, inviteId)
}

/** The server once it no longer installs, read again on each change its Core announces. */
async function installedServer(
	client: AbstractModrinthClient,
	core: ContractClient,
	serverId: string,
): Promise<Server> {
	const events = client.coreEvents((await client.cores.coreOf(serverId)).id)
	let changed = () => {}
	const stops = [
		events.subscribe({ type: 'server', id: serverId }),
		events.on('server_changed', (change) => {
			if (change.server === serverId) changed()
		}),
		events.on('resync', () => changed()),
	]
	try {
		// Subscribed before the first read, so a change after it cannot pass unseen.
		await events.ready()
		for (;;) {
			const next = new Promise<void>((resolve) => (changed = resolve))
			const server = await core.call(coreRoutes.GetServer, { params: { id: serverId } })
			if (server.status !== 'installing') return server
			await next
		}
	} finally {
		for (const stop of stops) stop()
	}
}
