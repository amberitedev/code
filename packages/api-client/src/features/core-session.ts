import type { AbstractModrinthClient } from '../core/abstract-client'
import { AbstractFeature } from '../core/abstract-feature'
import { ModrinthApiError } from '../core/errors'
import { TICKET_HEADER } from '../modules/copal/access/v1'
import type { RequestContext } from '../types/request'

/**
 * Requests to a listed Core. The client adds this itself when `ClientConfig.coreSessions` is set,
 * innermost, so it acts on each attempt as it is sent:
 *
 * - A `dangerous` request gets a fresh ticket in `X-Amberite-Ticket`.
 * - A request Core answers with 401 `unauthorized` (no session, or it ended) signs in with a new
 *   ticket once and is sent again. Any other 401, such as a refused ticket, is the answer.
 */
export class CoreSessionFeature extends AbstractFeature {
	constructor(private readonly client: AbstractModrinthClient) {
		super({ name: 'core-session' })
	}

	shouldApply(context: RequestContext): boolean {
		return (
			super.shouldApply(context) &&
			this.client.archon.cores_v1.coreForUrl(context.url) !== undefined
		)
	}

	async execute<T>(next: () => Promise<T>, context: RequestContext): Promise<T> {
		const core = this.client.archon.cores_v1.coreForUrl(context.url)
		if (!core) return next()
		const access = this.client.copal.access_v1
		const send = async () => {
			if (context.options.dangerous)
				context.options.headers = {
					...context.options.headers,
					[TICKET_HEADER]: await access.ticket(core.id),
				}
			return next()
		}
		try {
			return await send()
		} catch (error) {
			if (!signedOut(error)) throw error
			await access.signIn(core)
			return send()
		}
	}
}

function signedOut(error: unknown): boolean {
	if (!(error instanceof ModrinthApiError) || error.statusCode !== 401) return false
	const data = error.responseData
	return (
		typeof data === 'object' && data !== null && 'error' in data && data.error === 'unauthorized'
	)
}
