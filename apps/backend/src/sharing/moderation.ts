import { sharing } from '@amberite/contracts/backend'

import { requireUser } from '../common/auth'
import { ApiError } from '../common/http'
import { noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { parseUserIds } from './parse'

// Staff tools: accounts barred from sharing (`sharing_blacklist`, checked with each caller's
// session in `sharer`) and quarantined shares, whose files nobody can read.

async function requireModerator(request: Request, env: Env) {
	const user = await requireUser(request, env)
	if (user.role !== 'admin' && user.role !== 'moderator')
		throw new ApiError(403, 'not_moderator', 'Moderator access is required')
}

export const moderationRoutes = [
	route(sharing.GetBlacklistStatus, noBody, async ({ request, env, params }) => {
		await requireUser(request, env)
		return {
			blacklisted: !!(await env.DB.prepare('SELECT 1 FROM sharing_blacklist WHERE user_id = ?')
				.bind(params.user_id)
				.first()),
		}
	}),
	route(sharing.BlacklistUsers, parseUserIds, async ({ request, env, body }) => {
		await requireModerator(request, env)
		await env.DB.prepare(
			'INSERT OR IGNORE INTO sharing_blacklist (user_id) SELECT value FROM json_each(?)',
		)
			.bind(JSON.stringify((await body()).user_ids))
			.run()
	}),
	route(sharing.UnblacklistUsers, parseUserIds, async ({ request, env, body }) => {
		await requireModerator(request, env)
		await env.DB.prepare(
			'DELETE FROM sharing_blacklist WHERE user_id IN (SELECT value FROM json_each(?))',
		)
			.bind(JSON.stringify((await body()).user_ids))
			.run()
	}),
	route(sharing.DeleteModeratedFile, noBody, async ({ request, env, params }) => {
		await requireModerator(request, env)
		// Files are opaque to the backend, so quarantining the instance is what it can do.
		await env.DB.prepare('UPDATE shared_instances SET quarantine = 1 WHERE id = ?')
			.bind(params.id)
			.run()
	}),
]
