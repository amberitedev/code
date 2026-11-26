// What `vp run fixture <name>` does with a fixture file: find it, check its
// types, run it against the dev stack, and tidy up after it.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import { createRequire } from 'node:module'
import * as NodePath from 'node:path'
import * as NodeURL from 'node:url'

import { restartIfPatched } from './call.ts'
import { begin, FixtureError, type Runner, state } from './context.ts'
import { keepGameFiles } from './downloads.ts'
import { rememberJava } from './java.ts'
import { PACKAGES } from './packages.ts'

/** A fixture kept in `packages/fixtures/fixtures` by name, or any fixture file by path. */
export function resolveFixture(worktree: string, name: string): string {
	const kept = NodePath.join(worktree, 'packages', 'fixtures', 'fixtures')
	const candidates = [NodePath.resolve(name), NodePath.join(kept, `${name}.ts`)]
	const file = candidates.find((path) => NodeFS.existsSync(path) && NodeFS.statSync(path).isFile())
	if (file) return file
	const known = NodeFS.readdirSync(kept)
		.filter((entry) => entry.endsWith('.ts'))
		.map((entry) => entry.slice(0, -3))
	throw new FixtureError(`No fixture ${name}. Pass a file, or one of: ${known.join(', ')}`)
}

/**
 * Type-checks the fixture and everything it imports against the generated contracts, before
 * anything is stopped or wiped. Prints the compiler's errors and returns false when there are any.
 */
export async function checkTypes(worktree: string, file: string): Promise<boolean> {
	const local = (path: string) => NodeURL.fileURLToPath(path).replaceAll('\\', '/')
	// One per command: two applies in one checkout must not check each other's fixture.
	const config = NodePath.join(worktree, '.data', `fixture-tsconfig.${process.pid}.json`)
	NodeFS.mkdirSync(NodePath.dirname(config), { recursive: true })
	NodeFS.writeFileSync(
		config,
		JSON.stringify({
			compilerOptions: {
				target: 'ES2023',
				module: 'Preserve',
				moduleResolution: 'Bundler',
				lib: ['ES2023'],
				// A fixture may live outside the repo, where it has no dependencies of its own.
				typeRoots: [NodePath.join(worktree, 'node_modules', '@types').replaceAll('\\', '/')],
				types: ['node'],
				paths: Object.fromEntries(
					Object.entries(PACKAGES).map(([name, url]) => [name, [local(url)]]),
				),
				strict: true,
				noEmit: true,
				skipLibCheck: true,
				allowImportingTsExtensions: true,
			},
			files: [file.replaceAll('\\', '/')],
		}),
	)
	// The compiler itself, not through `vp exec`, which takes longer to start than the check runs.
	const compiler = createRequire(import.meta.url).resolve('typescript/lib/tsc.js')
	const result = NodeChildProcess.spawnSync(process.execPath, [compiler, '-p', config], {
		cwd: worktree,
		encoding: 'utf8',
		windowsHide: true,
	})
	NodeFS.rmSync(config, { force: true })
	if (result.status === 0) return true
	console.error(`[fixture] ${NodePath.basename(file)} does not type-check, so nothing was changed.`)
	console.error((result.stdout + result.stderr).trim())
	return false
}

/** Runs the fixture top to bottom. A line that fails stops it there, with the stack left up. */
export async function runFixture(input: Runner & { readonly file: string }): Promise<void> {
	const started = performance.now()
	console.log(`[fixture] ${NodePath.basename(input.file)}`)
	await apply(input, () => import(NodeURL.pathToFileURL(input.file).href))
	console.log(`[fixture] applied in ${((performance.now() - started) / 1000).toFixed(1)} s`)
}

/**
 * Runs `build`, whose first line is `stack(...)`, against the dev stack `runner` reaches, and
 * finishes what it left open. A fixture file is one `build`; a scenario passes its own.
 */
export async function apply<T>(runner: Runner, build: () => Promise<T>): Promise<T> {
	begin(runner)
	const built = await build()
	const made = state.stack
	if (!made) throw new FixtureError('The fixture never called stack(), so nothing was applied.')
	for (const label of [...state.dirty]) await restartIfPatched(label)
	await Promise.all(made.apps.map(rememberJava))
	// Everything a helper started has finished by now, so no download is half written.
	const kept =
		made.apps.reduce((sum, app) => sum + keepGameFiles(runner.primary, 'app', app.dir), 0) +
		made.cores.reduce((sum, core) => sum + keepGameFiles(runner.primary, 'core', core.dir), 0)
	if (kept > 0) console.log(`[fixture] kept ${kept} downloaded game files for other checkouts`)
	return built
}
