import { AbstractModule } from '../../../core/abstract-module'
import type { RequestOptions } from '../../../types/request'
import type { Copal } from '../types'

/** The header a dangerous action carries its fresh ticket in. */
export const TICKET_HEADER = 'X-Amberite-Ticket'

/**
 * A Core this device is signed in to: the base URL `archon_cores_v1` routes it to, and the session
 * token sign-in returned. The client drops credentials sent to any URL that is not a listed Core.
 */
export type CoreSession = { url: string; token: string }

/**
 * Signing in to a Core and managing who may use it. The account backend issues a ticket for one
 * Core; the Core turns it into a session of its own, so the account session never leaves the App.
 *
 * Dangerous actions (inviting, removing, changing a role, revoking someone else's session) take a
 * fresh `ticket()` as well as the session. Each ticket is good for one use, for five minutes.
 */
export class CopalAccessV1Module extends AbstractModule {
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

	/** Sign in to the Core at `url` with a ticket for it. Keep the token; it is shown once. */
	public signIn(url: string, ticket: string, deviceName: string): Promise<Copal.SignedIn> {
		return this.core(url, null, '/sessions', {
			method: 'POST',
			body: { ticket, device_name: deviceName } satisfies Copal.SignIn,
		})
	}

	/** Every session on the Core for its owner; only the caller's own for anyone else. */
	public sessions(session: CoreSession): Promise<Copal.Session[]> {
		return this.core(session.url, session.token, '/sessions')
	}

	/** End a session. Someone else's needs a `ticket` and the owner. */
	public async revokeSession(session: CoreSession, id: string, ticket?: string): Promise<void> {
		await this.core(session.url, session.token, `/sessions/${encodeURIComponent(id)}`, {
			method: 'DELETE',
			ticket,
		})
	}

	/** Sign this device out of the Core. */
	public async signOut(session: CoreSession): Promise<void> {
		await this.core(session.url, session.token, '/sessions/current', { method: 'DELETE' })
	}

	/** The caller's own access: whether they own the Core, and their role on each server. */
	public me(session: CoreSession): Promise<Copal.MyAccess> {
		return this.core(session.url, session.token, '/access/me')
	}

	/** The owner and members of a server. Owner only. */
	public serverAccess(session: CoreSession, serverId: string): Promise<Copal.ServerAccess> {
		return this.core(session.url, session.token, `/servers/${serverId}/access`)
	}

	/** Give `invite.user` a role on a server, at once. The backend notifies them. */
	public invite(
		session: CoreSession,
		serverId: string,
		invite: Copal.Invite,
		ticket: string,
	): Promise<Copal.Member> {
		return this.core(session.url, session.token, `/servers/${serverId}/access`, {
			method: 'POST',
			body: invite,
			ticket,
		})
	}

	public changeRole(
		session: CoreSession,
		serverId: string,
		userId: string,
		role: Copal.Role,
		ticket: string,
	): Promise<Copal.Member> {
		return this.core(session.url, session.token, member(serverId, userId), {
			method: 'PATCH',
			body: { role } satisfies Copal.ChangeRole,
			ticket,
		})
	}

	/** Take someone's role on a server. Every session they have on the Core ends. */
	public async remove(
		session: CoreSession,
		serverId: string,
		userId: string,
		ticket: string,
	): Promise<void> {
		await this.core(session.url, session.token, member(serverId, userId), {
			method: 'DELETE',
			ticket,
		})
	}

	/** The activity log, newest first. Owner only. Pass `next_before` back as `before` for more. */
	public activity(
		session: CoreSession,
		query: Copal.ActivityQuery = {},
	): Promise<Copal.ActivityPage> {
		return this.core(session.url, session.token, '/activity', { params: query })
	}

	/**
	 * Call a Core route with this device's session, or none for sign-in. The account token never
	 * goes along: `skipAuth` keeps every auth feature off, and `ownAuthorization` leaves off an
	 * `Authorization` from the client's default headers. The first path segment goes in `version`,
	 * which is how the client builds a URL on a custom base.
	 */
	private core<T>(
		url: string,
		token: string | null,
		route: string,
		{
			ticket,
			...options
		}: Pick<RequestOptions, 'method' | 'body' | 'params'> & { ticket?: string } = {},
	): Promise<T> {
		const [version, ...rest] = route.slice(1).split('/')
		return this.client.request<T>(rest.length ? `/${rest.join('/')}` : '', {
			...options,
			api: url,
			version,
			retry: false,
			skipAuth: true,
			ownAuthorization: true,
			headers: {
				...(token === null ? {} : { Authorization: `Bearer ${token}` }),
				...(ticket ? { [TICKET_HEADER]: ticket } : {}),
			},
		})
	}
}

function member(serverId: string, userId: string): string {
	return `/servers/${serverId}/access/${encodeURIComponent(userId)}`
}
