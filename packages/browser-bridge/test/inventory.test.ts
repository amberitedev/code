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

const expectedRuntimeImports: Record<string, ReadonlyArray<string>> = {
	'@tauri-apps/api/app': ['getVersion'],
	'@tauri-apps/api/core': ['convertFileSrc', 'invoke'],
	'@tauri-apps/api/event': ['listen'],
	'@tauri-apps/api/path': ['join'],
	'@tauri-apps/api/webview': ['getCurrentWebview'],
	'@tauri-apps/api/window': ['getCurrentWindow'],
	'@tauri-apps/plugin-dialog': ['open', 'save'],
	'@tauri-apps/plugin-fs': [
		'mkdir',
		'readDir',
		'readFile',
		'readTextFile',
		'remove',
		'rename',
		'stat',
		'writeFile',
		'writeTextFile',
	],
	'@tauri-apps/plugin-http': ['fetch'],
	'@tauri-apps/plugin-opener': ['openUrl'],
	'@tauri-apps/plugin-os': ['platform', 'type', 'version'],
	'@tauri-apps/plugin-window-state': ['StateFlags', 'saveWindowState'],
}

describe.sequential('Amberite Tauri API inventory', () => {
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

		expect(appCargo).toContain('[dependencies.amberite-browser-bridge]')
		expect(appCargo).toMatch(/\[dependencies\.amberite-browser-bridge\][\s\S]*?optional = true/)
		expect(appCargo).toContain('browser-bridge = ["dep:amberite-browser-bridge"]')
		expect(appMain).toContain('#[cfg(all(debug_assertions, feature = "browser-bridge"))]')
		expect(appPackage).toContain('tauri dev --features browser-bridge')
		expect(devRunner).toMatch(
			/["']tauri["'],\s*["']dev["'],\s*["']--features["'],\s*["']browser-bridge["']/,
		)
		expect(appPackage).not.toMatch(/"build"\s*:\s*"[^"]*browser-bridge/)
	})

	it('fails when Amberite adds a Tauri runtime surface without bridge coverage', () => {
		const actual = collectRuntimeImports(readAmberiteSources())
		expect(normalizeInventory(actual)).toEqual(normalizeInventory(expectedRuntimeImports))
	})

	it('round-trips every direct command Amberite sends through invoke', async () => {
		const commands = collectCommands(readAmberiteSources())
		expect(commands.length).toBeGreaterThan(150)
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

function collectRuntimeImports(sources: ReadonlyArray<string>): Record<string, string[]> {
	const imports = new Map<string, Set<string>>()
	const add = (module: string, name: string) => {
		const names = imports.get(module) ?? new Set<string>()
		names.add(name)
		imports.set(module, names)
	}
	for (const source of sources) {
		const staticImports = source.matchAll(
			/import\s*\{([^}]*)\}\s*from\s*['"](@tauri-apps\/[^'"]+)['"]/g,
		)
		for (const match of staticImports) {
			for (const raw of match[1]?.split(',') ?? []) {
				const item = raw.trim()
				if (!item || /^type\s+/.test(item)) continue
				add(match[2]!, item.split(/\s+as\s+/)[0]!.trim())
			}
		}
		for (const match of source.matchAll(/import\(['"](@tauri-apps\/[^'"]+)['"]\)/g)) {
			if (match[1] === '@tauri-apps/plugin-http') add(match[1], 'fetch')
		}
	}
	return Object.fromEntries([...imports].map(([module, names]) => [module, [...names]]))
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

function normalizeInventory(inventory: Record<string, ReadonlyArray<string>>) {
	return Object.fromEntries(
		Object.entries(inventory)
			.sort(([left], [right]) => left.localeCompare(right))
			.map(([module, names]) => [module, [...names].sort()]),
	)
}
