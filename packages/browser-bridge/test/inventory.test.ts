import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { installBrowserBridge } from './harness'

const repositoryRoot = NodePath.resolve(
	NodePath.dirname(fileURLToPath(import.meta.url)),
	'..',
	'..',
	'..',
)
const sourceRoots = [
	NodePath.join(repositoryRoot, 'apps', 'app-frontend', 'src'),
	NodePath.join(repositoryRoot, 'packages', 'api-client', 'src'),
	NodePath.join(repositoryRoot, 'packages', 'ui', 'src'),
]

describe.sequential('Amberite bridge wiring and commands', () => {
	it('keeps the Amberite hook optional, debug-only, and enabled only by dev launchers', () => {
		const appCargo = NodeFS.readFileSync(
			NodePath.join(repositoryRoot, 'apps', 'app', 'Cargo.toml'),
			'utf8',
		)
		const appMain = NodeFS.readFileSync(
			NodePath.join(repositoryRoot, 'apps', 'app', 'src', 'main.rs'),
			'utf8',
		)
		const appPackage = NodeFS.readFileSync(
			NodePath.join(repositoryRoot, 'apps', 'app', 'package.json'),
			'utf8',
		)
		const devRunner = NodeFS.readFileSync(
			NodePath.join(repositoryRoot, 'scripts', 'dev-runner.ts'),
			'utf8',
		)

		expect(appCargo).toContain('[dependencies.browser-bridge]')
		expect(appCargo).toMatch(/\[dependencies\.browser-bridge\][\s\S]*?optional = true/)
		expect(appCargo).toContain('browser-bridge = ["dep:browser-bridge"]')
		expect(appMain).toContain('#[cfg(all(debug_assertions, feature = "browser-bridge"))]')
		expect(appPackage).toMatch(/tauri dev --features [\w,-]*browser-bridge/)
		expect(devRunner).toMatch(
			/["']build["'],[\s\S]*?["']theseus_gui["'],[\s\S]*?["']--features["'],\s*["']browser-bridge["']/,
		)
		expect(appPackage).not.toMatch(/"build"\s*:\s*"[^"]*browser-bridge/)
	})

	it('round-trips every direct command Amberite sends through invoke', async () => {
		// `show_window` raises the native window, so the bridge answers it in the browser.
		const commands = collectCommands(readAmberiteSources()).filter((cmd) => cmd !== 'show_window')
		const { socket, window } = await installBrowserBridge()

		for (const [index, cmd] of commands.entries()) {
			const args = { commandIndex: index, nested: { cmd }, values: [null, true, 42] }
			const options = { headers: { 'x-amberite-command': encodeURIComponent(cmd) } }
			const success = window.__TAURI_INTERNALS__.invoke(cmd, args, options)
			const request = await socket.nextInvoke()
			expect(request).toMatchObject({ args, cmd, options })
			const result = { cmd, index, ok: true }
			socket.respond(request, 'success', result)
			await expect(success).resolves.toEqual(result)

			const rejected = window.__TAURI_INTERNALS__.invoke(cmd, args).catch((error: unknown) => error)
			const errorRequest = await socket.nextInvoke()
			const error = { cmd, code: 'bridge-test-error', index }
			socket.respond(errorRequest, 'error', error)
			await expect(rejected).resolves.toEqual(error)
		}
	})
})

function readAmberiteSources(): ReadonlyArray<string> {
	return sourceRoots.flatMap((root) => walk(root).map((path) => NodeFS.readFileSync(path, 'utf8')))
}

function walk(directory: string): string[] {
	const output: string[] = []
	for (const entry of NodeFS.readdirSync(directory, { withFileTypes: true })) {
		const path = NodePath.join(directory, entry.name)
		if (entry.isDirectory()) output.push(...walk(path))
		else if (/\.(?:js|mjs|ts|tsx|vue)$/.test(entry.name)) output.push(path)
	}
	return output
}

function collectCommands(sources: ReadonlyArray<string>): string[] {
	const commands = new Set<string>()
	for (const source of sources) {
		for (const match of source.matchAll(/\binvoke(?:<[^>]+>)?\(\s*(['"`])([^'"`]+)\1/g)) {
			commands.add(match[2]!)
		}
	}
	return [...commands].sort()
}
