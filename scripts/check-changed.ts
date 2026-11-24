#!/usr/bin/env node

// CI's fast checks for this branch's changes, so they fail here instead of after a push:
// lint, typos and TOML on the changed files, typecheck and tests of the affected packages, and
// for Rust, `cargo fmt`, a build through the dev runner (the same compile check a start does,
// sharing its build folder), and the tests of every workspace crate with a changed file, also
// through the runner and in that folder.
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

// The packages CI typechecks and tests (ci.yml, js job), their folder, and what reaches each.
const PACKAGES = [
	{
		name: '@amberite/backend',
		folder: 'apps/backend',
		test: true,
		reachedBy: ['apps/backend', 'packages/contracts'],
	},
	// A fixture helper calls the generated endpoints, so a changed contract shows up here.
	{
		name: '@amberite/fixtures',
		folder: 'packages/fixtures',
		test: false,
		reachedBy: ['packages/fixtures', 'packages/contracts'],
	},
	{
		name: '@modrinth/app-frontend',
		folder: 'apps/app-frontend',
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
	/** A step that starts once this one has finished, whether or not it passed. */
	readonly next?: Step
	/** TypeScript's incremental state, which it reads and writes as `--tsBuildInfoFile`. */
	readonly buildInfo?: string
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
		// Where the package's typecheck script keeps it.
		buildInfo: `${pack.folder}/node_modules/.cache/typecheck.tsbuildinfo`,
	})
	if (pack.test) {
		steps.push({
			name: `test ${pack.name}`,
			command: 'vp',
			args: ['run', '--filter', pack.name, 'test'],
		})
	}
}
// The root test run (vite.config.ts) covers scripts/ and the unit tests of api-client and fixtures.
const unitTests = changed.flatMap((file) => {
	if (!/^(?:scripts|packages\/(?:api-client|fixtures)\/src)\/.+\.ts$/.test(file)) return []
	const test = file.replace(/(?<!\.test)\.ts$/, '.test.ts')
	return NodeFS.existsSync(NodePath.join(root, test)) ? [test] : []
})
if (unitTests.length > 0) {
	steps.push({ name: 'unit tests', command: 'vp', args: ['test', 'run', ...new Set(unitTests)] })
}
const rust = matching(/\.rs$/)
if (rust.length > 0) {
	steps.push({ name: 'rustfmt', command: 'rustfmt', args: ['--check'], files: rust })
}
// Builds go one at a time anyway, so the Rust tests follow the build in one chain.
let rustTests: Step | undefined
const crates = under('apps', 'packages') ? workspaceCrates() : []
if (crates === 'no cargo') skipped.push('rust tests (cargo is not installed)')
else if (crates === 'failed') failed.push('rust tests (cargo metadata failed)')
else {
	// The crates CI leaves out of the workspace tests (ci.yml, rust job).
	const untested = new Set(['toasty'])
	const tested = crates.filter((crate) => !untested.has(crate.name) && under(crate.folder))
	if (tested.some((crate) => crate.name === 'amberite-app')) {
		// Tauri only checks that the frontend build folder exists, as in CI.
		NodeFS.mkdirSync(NodePath.join(root, 'apps', 'app-frontend', 'dist'), { recursive: true })
	}
	if (tested.length > 0) {
		rustTests = {
			name: 'rust tests',
			command: 'node',
			args: ['scripts/dev-runner.ts', 'test', ...tested.map((crate) => crate.name)],
		}
	}
}
if (touched.some((file) => /\.rs$|(?:^|\/)Cargo\.(?:toml|lock)$/.test(file))) {
	steps.push({
		name: 'rust build',
		command: 'node',
		args: ['scripts/dev-runner.ts', 'build'],
		next: rustTests,
	})
} else if (rustTests) steps.push(rustTests)

console.log(`[check] ${changed.length} changed files since ${base.slice(0, 9)}`)
// Every step runs at once; each one's output is printed whole when it finishes.
const results = (
	await Promise.all(
		steps.map(async (first) => {
			const done: Array<{ readonly name: string; readonly ok: boolean }> = []
			for (let step: Step | undefined = first; step; step = step.next) {
				let ok = true
				let output = ''
				if (step.buildInfo) shareBuildInfo(step.buildInfo, 'in')
				for (const files of batches(step.files ?? [])) {
					const result = await run(step.command, [...step.args, ...files])
					ok &&= result.ok
					output += result.output
				}
				if (ok && step.buildInfo) shareBuildInfo(step.buildInfo, 'out')
				console.log(`\n[check] ${step.name}\n${output.trimEnd()}`)
				done.push({ name: step.name, ok })
			}
			return done
		}),
	)
).flat()
console.log('\n[check] summary')
for (const { name, ok } of results) console.log(`  ${ok ? 'pass' : 'FAIL'}  ${name}`)
for (const name of failed) console.log(`  FAIL  ${name}`)
for (const name of skipped) console.log(`  skip  ${name}`)
if (failed.length > 0 || results.some((result) => !result.ok)) process.exitCode = 1

/**
 * A typecheck's incremental state, shared by every checkout like the Rust build output, so a new
 * worktree checks only what differs from the last check anywhere: about 15 seconds instead of 50
 * for the App. TypeScript stores paths relative to the file and checks each source by its content,
 * so any checkout's state is a safe start. `in` seeds a checkout that has none of its own; `out`
 * shares a passing check's state.
 */
function shareBuildInfo(file: string, direction: 'in' | 'out'): void {
	const primary = git(['worktree', 'list', '--porcelain'])
		.split(/\r?\n/)[0]!
		.slice('worktree '.length)
	const shared = NodePath.join(
		primary,
		'target',
		'dev-shared',
		'typecheck',
		file.replaceAll('/', '_'),
	)
	const local = NodePath.join(root, file)
	try {
		if (direction === 'in') {
			if (!NodeFS.existsSync(local) && NodeFS.existsSync(shared)) {
				NodeFS.mkdirSync(NodePath.dirname(local), { recursive: true })
				NodeFS.copyFileSync(shared, local)
			}
		} else if (NodeFS.existsSync(local)) {
			NodeFS.mkdirSync(NodePath.dirname(shared), { recursive: true })
			const partial = `${shared}.${process.pid}.partial`
			NodeFS.copyFileSync(local, partial)
			NodeFS.renameSync(partial, shared)
		}
	} catch {
		// Another checkout is sharing its own at this moment; this one's check is no slower.
	}
}

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

/** Whether the command succeeded, and what it printed. */
function run(
	command: string,
	args: ReadonlyArray<string>,
): Promise<{ readonly ok: boolean; readonly output: string }> {
	// Windows runs vp through its .cmd shim, which needs the shell. File names are quoted for it.
	const child =
		process.platform === 'win32' && command === 'vp'
			? NodeChildProcess.spawn([command, ...args.map((arg) => `"${arg}"`)].join(' '), {
					cwd: root,
					shell: true,
				})
			: NodeChildProcess.spawn(command, args, { cwd: root })
	let output = ''
	const collect = (chunk: Buffer) => {
		output += chunk.toString()
	}
	child.stdout?.on('data', collect)
	child.stderr?.on('data', collect)
	return new Promise((resolve) => {
		child.once('error', (error) => resolve({ ok: false, output: `${command}: ${error.message}` }))
		child.once('close', (code) => resolve({ ok: code === 0, output }))
	})
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
