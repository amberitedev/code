import { dev, type OutboxEmail, type ScenarioLogin } from '@amberite/contracts'

import { validateUsername } from './accounts'
import { findUser, issueSession, requireLocal } from './common/auth'
import { hashPassword, randomId } from './common/crypto'
import { ApiError, objectBody, stringField, unauthorized } from './common/http'
import { type Route, noBody, route } from './common/router'

function parseScenarioLogin(raw: unknown): ScenarioLogin {
	const username = stringField(objectBody(raw), 'username', 39)
	validateUsername(username)
	return { username }
}

/** The dev runner's routes. `requireLocal` answers 404 outside a local dev backend. */
export const devRoutes: Route[] = [
	route(dev.Session, parseScenarioLogin, async ({ request, env, body }) => {
		requireLocal(request, env)
		const { username } = await body()
		let user = await findUser(env, username)
		if (user && !user.is_dev)
			throw new ApiError(403, 'invalid_credentials', 'Scenario login cannot access a real account')
		if (!user) {
			await env.DB.prepare(
				'INSERT INTO users (id,username,email,created,is_dev,email_verified,password_hash) VALUES (?,?,?,?,1,1,?)',
			)
				.bind(
					randomId(),
					username,
					`${username}@scenario.invalid`,
					new Date().toISOString(),
					await hashPassword(`Scenario-${username}-Local-only!`),
				)
				.run()
			user = await findUser(env, username)
		}
		if (!user) unauthorized()
		return issueSession(request, env, user.id)
	}),
	route(dev.Outbox, noBody, async ({ request, env }) => {
		requireLocal(request, env)
		return (
			await env.DB.prepare(
				'SELECT * FROM email_outbox ORDER BY created DESC LIMIT 100',
			).all<OutboxEmail>()
		).results
	}),
]
