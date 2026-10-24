import { AbstractModule } from '../../../core/abstract-module'
import { ModrinthApiError } from '../../../core/errors'
import type { CoreCallError, CoreSessions } from '../../../types/client'
import type { RequestOptions } from '../../../types/request'
import type { Archon } from '../../archon/types'
import type { Copal } from '../types'

/** The header a dangerous action carries its fresh ticket in. */
export const TICKET_HEADER = 'X-Amberite-Ticket'

/**
 * Signing in to a Core and managing who may use it. The account backend issues a ticket for one
 * Core; the platform trades it with the Core for a session of its own (`ClientConfig.coreSessions`)
 * and adds that session to every request to the Core. The account session never leaves the App,
 * and the page never sees a Core session.
 *
 * Dangerous actions (inviting, removing, changing a role, revoking someone else's session,
 * removing the Core from its account, deleting a server, restoring a backup) also carry a fresh
 * ticket, fetched right before the call: each ticket is good for one use, for five minutes. A
 * request marked `dangerous` gets one from `CoreSessionFeature`.
 */
export class CopalAccessV1Module extends AbstractModule {
	/** Sign-ins in flight, by Core id, so requests that fail together sign in once. */
	private signingIn = new Map<string, Promise<Copal.Session>>()

	public getModuleID(): string {
		return 'copal_access_v1'
	}

	/** A ticket for `coreId`, for its owner or someone invited to one of its servers. */
	public async ticket(coreId: string): Promise<string> {
		const { ticket } = await this.client.request<{ ticket: string }>(
			`/cores/${encodeURIComponent(coreId)}/ticket`,
			{ api: 'labrinth', version: 3, method: 'POST', retry: false },
		)
		return ticket
	}

	/** Sign in to `core` unless this device already holds a session there. */
	public async ensureSession(core: Archon.Cores.v1.Core): Promise<void> {
		if (await this.platform().exists(core.id)) return
		await this.signIn(core)
	}

	/**
	 * Sign this device in to `core` with a fresh ticket, replacing any session it held. Throws a
	 * `ModrinthApiError` with Core's error, such as `not_invited` or `clock_wrong`.
	 */
	public signIn(core: Archon.Cores.v1.Core): Promise<Copal.Session> {
		const pending = this.signingIn.get(core.id)
		if (pending) return pending
		const signing = (async () => {
			const fingerprint = core.cert_fingerprint
			if (!fingerprint)
				throw new ModrinthApiError('This Core does not serve HTTPS yet', { statusCode: 400 })
			const ticket = await this.ticket(core.id)
			const address = this.client.archon.cores_v1.address(core)
			return this.platform()
				.signIn(core.id, address, fingerprint, ticket)
				.catch((error: unknown) => {
					throw coreError(error)
				})
		})().finally(() => this.signingIn.delete(core.id))
		this.signingIn.set(core.id, signing)
		return signing
	}

	/** Sign this device out of the Core, and forget its session even when the Core is off. */
	public signOut(coreId: string): Promise<void> {
		return this.platform().signOut(coreId)
	}

	/** Every session on the Core for its owner; only the caller's own for anyone else. */
	public sessions(coreId: string): Promise<Copal.Session[]> {
		return this.core(coreId, '/sessions')
	}

	/** End a session. Someone else's needs the owner, and the fresh ticket this sends. */
	public async revokeSession(coreId: string, id: string): Promise<void> {
		await this.core(coreId, `/sessions/${encodeURIComponent(id)}`, {
			method: 'DELETE',
			dangerous: true,
		})
	}

	/** The caller's own access: whether they own the Core, and their role on each server. */
	public me(coreId: string): Promise<Copal.MyAccess> {
		return this.core(coreId, '/access/me')
	}

	/** The owner and members of a server. Owner only. */
	public serverAccess(coreId: string, serverId: string): Promise<Copal.ServerAccess> {
		return this.core(coreId, `/servers/${serverId}/access`)
	}

	/** Give `invite.user` a role on a server, at once. The backend notifies them. */
	public invite(coreId: string, serverId: string, invite: Copal.Invite): Promise<Copal.Member> {
		return this.core(coreId, `/servers/${serverId}/access`, {
			method: 'POST',
			body: invite,
			dangerous: true,
		})
	}

	public changeRole(
		coreId: string,
		serverId: string,
		userId: string,
		role: Copal.Role,
	): Promise<Copal.Member> {
		return this.core(coreId, member(serverId, userId), {
			method: 'PATCH',
			body: { role } satisfies Copal.ChangeRole,
			dangerous: true,
		})
	}

	/** Take someone's role on a server. Every session they have on the Core ends. */
	public async remove(coreId: string, serverId: string, userId: string): Promise<void> {
		await this.core(coreId, member(serverId, userId), { method: 'DELETE', dangerous: true })
	}

	/**
	 * Give up your own role on a server. No ticket: it only takes away what this device's session
	 * could do. Every session you have on the Core ends; the App signs in again for what is left.
	 */
	public async leave(coreId: string, serverId: string): Promise<void> {
		const { user } = await this.me(coreId)
		await this.core(coreId, member(serverId, user.id), { method: 'DELETE' })
	}

	/**
	 * Tell the Core it is being removed from its account: it ends every session and shows a new
	 * pairing code. Owner only. `archon.cores_v1.revoke` calls this before the backend forgets the
	 * Core.
	 */
	public async unpair(coreId: string): Promise<void> {
		await this.core(coreId, '/pairing', { method: 'DELETE', dangerous: true })
	}

	/** The activity log, newest first. Owner only. Pass `next_before` back as `before` for more. */
	public activity(coreId: string, query: Copal.ActivityQuery = {}): Promise<Copal.ActivityPage> {
		return this.core(coreId, '/activity', { params: query })
	}

	private platform(): CoreSessions {
		const sessions = this.client.coreSessions
		if (!sessions) throw new Error('This platform cannot sign in to a Core')
		return sessions
	}

	/**
	 * Call a Core route. The platform adds the session on the way, and `CoreSessionFeature` the
	 * ticket of a `dangerous` call. The account token never goes along: `skipAuth` keeps every auth
	 * feature off, and `ownAuthorization` leaves off an `Authorization` from the client's default
	 * headers. The first path segment goes in `version`, which is how the client builds a URL on a
	 * custom base.
	 */
	private async core<T>(
		coreId: string,
		route: string,
		options: Pick<RequestOptions, 'method' | 'body' | 'params' | 'dangerous'> = {},
	): Promise<T> {
		const [version, ...rest] = route.slice(1).split('/')
		return this.client.request<T>(rest.length ? `/${rest.join('/')}` : '', {
			...options,
			api: await this.client.archon.cores_v1.url(coreId),
			version,
			retry: false,
			skipAuth: true,
			ownAuthorization: true,
		})
	}
}

function member(serverId: string, userId: string): string {
	return `/servers/${serverId}/access/${encodeURIComponent(userId)}`
}

/** A platform sign-in failure as the error a Core request would have thrown. */
function coreError(error: unknown): ModrinthApiError {
	if (error instanceof ModrinthApiError) return error
	if (isCoreCallError(error))
		return new ModrinthApiError(error.description, {
			statusCode: error.status ?? undefined,
			responseData: { error: error.error, description: error.description },
		})
	return ModrinthApiError.fromUnknown(error)
}

function isCoreCallError(error: unknown): error is CoreCallError {
	return (
		typeof error === 'object' &&
		error !== null &&
		'description' in error &&
		typeof error.description === 'string' &&
		'error' in error &&
		typeof error.error === 'string'
	)
}
