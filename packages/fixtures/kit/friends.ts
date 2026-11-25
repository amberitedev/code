import { social } from '@amberite/contracts/backend'

import { call } from '../src/call.ts'
import type { Account, App } from '../src/context.ts'

/** Makes two accounts friends the way they would do it: one asks, the other accepts. */
export async function friends(first: Account | App, second: Account | App): Promise<void> {
	const [asks, accepts] = [first, second].map((who) => (who.kind === 'app' ? who.account : who))
	await call(asks!, social.AddFriend, { id: accepts!.id })
	await call(accepts!, social.AddFriend, { id: asks!.id })
}
