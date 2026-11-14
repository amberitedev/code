import * as NodeFS from 'node:fs'
import * as NodeVM from 'node:vm'

import { describe, expect, it } from 'vite-plus/test'

// Every webview the App creates runs this script, the `signin` window on Microsoft's page too.
const script = NodeFS.readFileSync(
	new URL('../apps/app/src/server/local-init.js', import.meta.url),
	'utf8',
)
const BASE = 'http://127.0.0.1:4100/secret'
const ORIGINS = ['tauri://localhost', 'http://localhost:1420']

/** The global a webview ends up with, rendered the way `init_script` in server/mod.rs does. */
function localGlobal(label: string, origin: string, topFrame = true): unknown {
	const window: Record<string, unknown> = {
		__TAURI_INTERNALS__: { metadata: { currentWebview: { label } } },
		location: { origin },
	}
	window.window = window
	window.top = topFrame ? window : {}
	NodeVM.runInNewContext(
		`{\n${script.trimEnd()}\nlocalInit(${JSON.stringify(BASE)}, ${JSON.stringify(ORIGINS)});\n}`,
		window,
	)
	return window.__AMBERITE_LOCAL__
}

describe('local-init.js', () => {
	it('gives the base to the main window at an App origin', () => {
		expect(localGlobal('main', 'tauri://localhost')).toBe(BASE)
		expect(localGlobal('main', 'http://localhost:1420')).toBe(BASE)
	})

	it.each([
		['the signin window', 'signin', 'tauri://localhost', true],
		['a child frame', 'main', 'tauri://localhost', false],
		['a foreign origin', 'main', 'https://login.live.com', true],
	])('gives nothing to %s', (_, label, origin, topFrame) => {
		expect(localGlobal(label, origin, topFrame)).toBeUndefined()
	})
})
