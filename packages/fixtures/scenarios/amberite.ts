// What a scenario imports: e2e's web `test` plus `amberite`, which seeds the dev stack the way a
// fixture does and opens an App's tab. The fixture tool and kit come along, so one import does.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { test as base } from '@e2e-dev/web'

import { stop } from '../src/call.ts'
import { type App, type Core, FixtureError, type Runtime, runtime } from '../src/context.ts'
import { apply } from '../src/run.ts'
import { stackEnv } from './stack-env.ts'

export * from '../src/index.ts'

export interface Amberite {
	/**
	 * Replaces the stack's data with what `build` makes, as a fixture file would. Its first line
	 * is `stack(...)`. Returns what `build` returns, so the test keeps its Apps, Cores and ids.
	 */
	readonly seed: <T>(build: () => Promise<T>) => Promise<T>
	/** Opens `app`'s tab, signed in, at a route of the App such as `/hosting/manage/`. */
	readonly open: (app: App, path?: string) => Promise<void>
	/** Stops an App or a Core, as when its machine is off. */
	readonly stop: (program: App | Core) => Promise<void>
}

const { data, primary, worktree } = stackEnv()
const runner = {
	data,
	primary,
	worktree,
	// The runner rewrites its file as it works: a torn read is "not yet", and the caller asks again.
	runtime: (): Runtime | undefined => {
		let current: Runtime & { readonly buildFailed?: string }
		try {
			current = JSON.parse(NodeFS.readFileSync(NodePath.join(data, 'runtime.json'), 'utf8'))
		} catch {
			return undefined
		}
		// Nothing a scenario waits for will come, so it stops here and not at its timeout.
		if (current.buildFailed) {
			throw new FixtureError(
				`${current.buildFailed} did not build. See ${NodePath.join(data, 'dev.log')}`,
			)
		}
		return current
	},
	send: (command: string) =>
		NodeFS.appendFileSync(
			NodePath.join(data, 'dev-commands'),
			`${command}
`,
		),
}

/**
 * Resolves in the page once the App's shell is drawn and its splash screen has left: the shell is
 * in the page under the splash first, where a locator already finds it.
 */
const APP_DRAWN = `new Promise((resolve, reject) => {
	const drawn = () =>
		document.querySelector('.app-grid-layout') && !document.querySelector('.splash-screen')
	if (drawn()) return resolve()
	const observer = new MutationObserver(() => {
		if (!drawn()) return
		observer.disconnect()
		resolve()
	})
	observer.observe(document.documentElement, { childList: true, subtree: true })
	setTimeout(() => reject(new Error('The App did not finish loading.')), 60000)
})`

export const test = base.extend<{ amberite: Amberite }>({
	amberite: async ({ app, browser }, use) => {
		await use({
			seed: (build) => apply(runner, build),
			open: async (who, path = '/') => {
				const tab = runtime().apps.find((entry) => entry.index === who.index)?.tabUrl
				if (!tab) throw new Error(`${who.name}'s App is not part of the running stack.`)
				// The tab URL carries the App's local API in its fragment, which a route keeps.
				const url = new URL(path, tab)
				url.hash = new URL(tab).hash
				await app.open(url.href)
				await browser.evaluate(APP_DRAWN)
			},
			stop: (program) => stop(`${program.kind}:${program.index}`),
		})
	},
})
