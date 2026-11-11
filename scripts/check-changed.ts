#!/usr/bin/env node

// CI's fast checks for this branch's changes, so they fail here instead of after a push:
// lint, typos and TOML on the changed files, typecheck and tests of the affected packages,
// `dev:check` when the dev scripts change, and for Rust, `cargo fmt`, a build through the dev
// runner (the same compile check `up` does, sharing its build folder), and the tests of every
// workspace crate with a changed file, also through the runner and in that folder.
//
//   vp run check:changed          compare against the merge base with origin/main
//   vp run check:changed <ref>    compare against the merge base with <ref>
//
// Uncommitted and untracked files count. CI still runs the full suites.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

const root = git(['rev-parse', '--show-toplevel'])
const base = git(['merge-base', 'HEAD', process.argv[2] ?? 'origin/main'])
// Deletions still decide which packages and builds run; only checks that read files skip them.
const touched = [
	...new Set([
		...git(['diff', '--name-only', '-z', base]).split('\0'),
		...git(['ls-files', '-o', '--exclude-standard', '-z']).split('\0'),
	]),
].filter(Boolean)
const changed = touched.filter((file) => NodeFS.existsSync(NodePath.join(root, file)))

const under = (...folders: ReadonlyArray<string>) =>
	touched.some((file) => folders.some((folder) => file.startsWith(`${folder}/`)))
const matching = (pattern: RegExp) => changed.filter((file) => pattern.test(file))

// The packages CI typechecks and tests (ci.yml, js job), and what reaches each.
const PACKAGES = [
	{ name: '@amberite/backend', test: true, reachedBy: ['apps/backend', 'packages/contracts'] },
	{ name: '@amberite/browser-bridge', test: true, reachedBy: ['packages/browser-bridge'] },
	{
		name: '@modrinth/app-frontend',
		test: false,
		reachedBy: [
			'apps/app-frontend',
			'packages/ui',
			'packages/api-client',
			'packages/assets',
			'packages/contracts',
			'packages/utils',
		],
	},
] as const

type Step = {
	readonly name: string
	readonly command: string
	readonly args: ReadonlyArray<string>
	/** Added after `args`, over as many runs as it takes to keep each command line short. */
	readonly files?: ReadonlyArray<string>
}
const steps: Step[] = []
const skipped: string[] = []
const failed: string[] = []

const scripts = matching(/\.(?:[cm]?[jt]s|vue)$/)
if (scripts.length > 0) {
	steps.push({ name: 'lint', command: 'vp', args: ['lint', '--deny-warnings'], files: scripts })
}
// Excluded files stay excluded when named, as in CI's run over the whole repo.
if (onPath('typos')) {
	steps.push({ name: 'typos', command: 'typos', args: ['--force-exclude'], files: changed })
} else skipped.push('typos (not installed)')
const toml = matching(/\.toml$/)
if (toml.length > 0) {
	if (onPath('tombi')) {
		steps.push({ name: 'tombi lint', command: 'tombi', args: ['lint'], files: toml })
		steps.push({ name: 'tombi fmt', command: 'tombi', args: ['fmt', '--check'], files: toml })
	} else skipped.push('tombi (not installed)')
}
for (const pack of PACKAGES) {
	if (!under(...pack.reachedBy)) continue
	steps.push({
		name: `typecheck ${pack.name}`,
		command: 'vp',
		args: ['run', '--filter', pack.name, 'typecheck'],
	})
	if (pack.test) {
		steps.push({
			name: `test ${pack.name}`,
			command: 'vp',
			args: ['run', '--filter', pack.name, 'test'],
		})
	}
}
// The root test run (vite.config.ts) covers scripts/ and api-client's unit tests.
const unitTests = changed.flatMap((file) => {
	if (!/^(?:scripts|packages\/api-client\/src)\/.+\.ts$/.test(file)) return []
	const test = file.replace(/(?<!\.test)\.ts$/, '.test.ts')
	return NodeFS.existsSync(NodePath.join(root, test)) ? [test] : []
})
if (unitTests.length > 0) {
	steps.push({ name: 'unit tests', command: 'vp', args: ['test', 'run', ...new Set(unitTests)] })
}
if (under('scripts') || touched.includes('dev.json')) {
	steps.push({ name: 'dev:check', command: 'vp', args: ['run', 'dev:check'] })
}
const rust = matching(/\.rs$/)
if (rust.length > 0) {
	steps.push({ name: 'rustfmt', command: 'rustfmt', args: ['--check'], files: rust })
}
if (touched.some((file) => /\.rs$|(?:^|\/)Cargo\.(?:toml|lock)$/.test(file))) {
	steps.push({ name: 'rust build', command: 'node', args: ['scripts/dev-runner.ts', 'build'] })
}
// Files outside a crate's folder that its tests read (ci.yml's rust filter lists them too).
const READ_BY_CRATE: Readonly<Record<string, string>> = {
	'packages/assets/external/illustrations/minecraft_server_icon.png': 'engine',
}
const crates = under('apps', 'packages') ? workspaceCrates() : []
if (crates === 'no cargo') skipped.push('rust tests (cargo is not installed)')
else if (crates === 'failed') failed.push('rust tests (cargo metadata failed)')
else {
	// The crates CI leaves out of the workspace tests (ci.yml, rust job).
	const untested = new Set(['toasty', 'serde-binhum'])
	const readBy = new Set(touched.flatMap((file) => READ_BY_CRATE[file] ?? []))
	const tested = crates.filter(
		(crate) => !untested.has(crate.name) && (under(crate.folder) || readBy.has(crate.name)),
	)
	if (tested.some((crate) => crate.name === 'theseus_gui')) {
		// Tauri only checks that the frontend build folder exists, as in CI.
		NodeFS.mkdirSync(NodePath.join(root, 'apps', 'app-frontend', 'dist'), { recursive: true })
	}
	if (tested.length > 0) {
		steps.push({
			name: 'rust tests',
			command: 'node',
			args: ['scripts/dev-runner.ts', 'test', ...tested.map((crate) => crate.name)],
		})
	}
}

console.log(`[check] ${changed.length} changed files since ${base.slice(0, 9)}`)
const results = steps.map((step) => {
	console.log(`\n[check] ${step.name}`)
	const runs = batches(step.files ?? []).map((files) => run(step.command, [...step.args, ...files]))
	return { name: step.name, ok: runs.every(Boolean) }
})
console.log('\n[check] summary')
for (const { name, ok } of results) console.log(`  ${ok ? 'pass' : 'FAIL'}  ${name}`)
for (const name of failed) console.log(`  FAIL  ${name}`)
for (const name of skipped) console.log(`  skip  ${name}`)
if (failed.length > 0 || results.some((result) => !result.ok)) process.exitCode = 1

/** The root workspace's crates, with folders relative to the checkout. */
function workspaceCrates() {
	const result = NodeChildProcess.spawnSync(
		'cargo',
		['metadata', '--no-deps', '--offline', '--format-version', '1'],
		{ cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
	)
	if (result.error && 'code' in result.error && result.error.code === 'ENOENT') return 'no cargo'
	if (result.error || result.status !== 0) {
		console.error(result.error?.message ?? result.stderr.trim())
		return 'failed'
	}
	const { packages } = JSON.parse(result.stdout) as {
		packages: ReadonlyArray<{ manifest_path: string; name: string }>
	}
	const checkout = NodeFS.realpathSync.native(root)
	return packages.map((crate) => ({
		name: crate.name,
		folder: NodePath.relative(checkout, NodePath.dirname(crate.manifest_path)).replaceAll(
			'\\',
			'/',
		),
	}))
}

/**
 * The files in groups that keep each command line under cmd.exe's 8191 characters, which Windows
 * applies to vp through its shell. No files is one empty group, so the step still runs once.
 */
function batches(files: ReadonlyArray<string>): ReadonlyArray<ReadonlyArray<string>> {
	const groups: string[][] = [[]]
	let length = 0
	for (const file of files) {
		if (length + file.length > 6000 && groups.at(-1)!.length > 0) {
			groups.push([])
			length = 0
		}
		groups.at(-1)!.push(file)
		length += file.length + 3
	}
	return groups
}

/** Whether the command succeeded. */
function run(command: string, args: ReadonlyArray<string>): boolean {
	const options = { cwd: root, stdio: 'inherit' } as const
	// Windows runs vp through its .cmd shim, which needs the shell. File names are quoted for it.
	const result =
		process.platform === 'win32' && command === 'vp'
			? NodeChildProcess.spawnSync([command, ...args.map((arg) => `"${arg}"`)].join(' '), {
					...options,
					shell: true,
				})
			: NodeChildProcess.spawnSync(command, args, options)
	if (result.error) console.error(`[check] ${command}: ${result.error.message}`)
	return result.status === 0
}

function onPath(command: string): boolean {
	const finder = process.platform === 'win32' ? 'where' : 'which'
	return NodeChildProcess.spawnSync(finder, [command], { stdio: 'ignore' }).status === 0
}

function git(args: ReadonlyArray<string>): string {
	const result = NodeChildProcess.spawnSync('git', args, {
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
	})
	if (result.status !== 0) throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`)
	return result.stdout.trim()
}
