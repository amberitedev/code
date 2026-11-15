// An App's long operations answer 202 at once and report how they ended only as an event, so a
// helper that starts one listens for that event.

import type { LocalEvent } from '@amberite/contracts'

import { appUrl } from './call.ts'
import { type App, FixtureError, runtime } from './context.ts'

/**
 * Runs `start`, which starts the operation `id` in `app`, and returns once that operation has
 * ended. An operation that fails throws with the App's own error.
 */
export async function ended(app: App, id: string, start: () => Promise<unknown>): Promise<void> {
	const tab = runtime().apps.find((entry) => entry.index === app.index)?.tabUrl
	// The App answers its socket only to its own pages, which it knows by their origin.
	const socket = new WebSocket(`${appUrl(app).replace(/^http/, 'ws')}/events`, {
		headers: { origin: new URL(tab!).origin },
	})
	const outcome = new Promise<void>((resolve, reject) => {
		const timeout = setTimeout(
			() => reject(new FixtureError(`${app.name}'s App never said how ${id} ended.`)),
			15 * 60_000,
		)
		socket.addEventListener('error', () =>
			reject(new FixtureError(`${app.name}'s App closed its event socket.`)),
		)
		socket.addEventListener('message', (message) => {
			const event = JSON.parse(String(message.data)) as LocalEvent
			if (event.type !== 'op_ended' || event.payload.op !== id) return
			clearTimeout(timeout)
			const { error } = event.payload
			if (error) {
				reject(
					new FixtureError(
						`${app.name}'s App could not finish the operation ${id}: ${error.error} (${error.description})`,
					),
				)
			} else resolve()
		})
	})
	try {
		await new Promise<void>((resolve, reject) => {
			socket.addEventListener('open', () => resolve())
			socket.addEventListener('error', () =>
				reject(new FixtureError(`${app.name}'s App refused its event socket.`)),
			)
		})
		// Listening first: a short operation can end before its 202 is read.
		await Promise.all([outcome, start()])
	} finally {
		// A failed start leaves `outcome` waiting, and nothing must report it later.
		outcome.catch(() => undefined)
		socket.close()
	}
}
