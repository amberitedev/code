import { describe, expect, it } from 'vite-plus/test'

import { AuthFeature } from '../../../features/auth'
import { SelfHostedFeature } from '../../../features/self-hosted'
import { GenericModrinthClient } from '../../../platform/generic'
import type { ClientConfig } from '../../../types/client'
import type { RequestOptions } from '../../../types/request'

const CORE = 'https://192.168.1.2:16662'
const ACCOUNT = 'mra_account'
const session = { url: CORE, token: 'core-session' }

/** The real request pipeline, with a transport that records each request instead of sending it. */
class CaptureClient extends GenericModrinthClient {
	public sent: { url: string; headers: Record<string, string> }[] = []

	protected async executeRequest<T>(url: string, options: RequestOptions): Promise<T> {
		this.sent.push({ url, headers: { ...options.headers } })
		return { ticket: 'ticket' } as T
	}
}

/** Account auth configured every way the client allows: a default header and both auth features. */
function client(defaultHeader: string) {
	const config: ClientConfig = {
		labrinthBaseUrl: 'https://accounts.example',
		headers: { [defaultHeader]: `Bearer ${ACCOUNT}` },
		features: [
			new AuthFeature({ token: ACCOUNT, headerName: 'X-Account-Token' }),
			new SelfHostedFeature({
				baseUrl: 'https://accounts.example',
				isCoreUrl: (url) => url.startsWith(CORE),
				token: async () => ACCOUNT,
			}),
		],
	}
	return new CaptureClient(config)
}

/** A header by name in any letter case: the self-hosted feature lowercases them. */
function header(headers: Record<string, string>, name: string) {
	return Object.entries(headers).find(([key]) => key.toLowerCase() === name.toLowerCase())?.[1]
}

function carriesAccount(headers: Record<string, string>) {
	return Object.values(headers).some((value) => value.includes(ACCOUNT))
}

describe('copal access', () => {
	for (const defaultHeader of ['Authorization', 'authorization']) {
		it(`never sends the account token to a Core (default header ${defaultHeader})`, async () => {
			const api = client(defaultHeader)
			const access = api.copal.access_v1

			await access.signIn(CORE, 'ticket', 'Laptop')
			await access.me(session)
			await access.invite(
				session,
				'server',
				{ user: { id: 'friend', name: 'Friend', avatar_url: null }, role: 'editor' },
				'fresh-ticket',
			)
			await access.signOut(session)

			expect(api.sent.map((request) => request.url)).toEqual([
				`${CORE}/sessions`,
				`${CORE}/access/me`,
				`${CORE}/servers/server/access`,
				`${CORE}/sessions/current`,
			])
			for (const { headers } of api.sent) expect(carriesAccount(headers)).toBe(false)
			const [signIn, me, invite] = api.sent
			expect(header(signIn.headers, 'Authorization')).toBeUndefined()
			expect(header(me.headers, 'Authorization')).toBe('Bearer core-session')
			expect(header(invite.headers, 'X-Amberite-Ticket')).toBe('fresh-ticket')
		})
	}

	it('asks the account backend for a ticket with the account token', async () => {
		const api = client('Authorization')
		expect(await api.copal.access_v1.ticket('core')).toBe('ticket')
		expect(api.sent[0].url).toBe('https://accounts.example/v3/cores/core/ticket')
		expect(carriesAccount(api.sent[0].headers)).toBe(true)
	})
})
