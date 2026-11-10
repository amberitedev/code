#!/usr/bin/env node

// CI's fast checks for this branch's changes, so they fail here instead of after a push:
// lint, typos and TOML on the changed files, typecheck and tests of the affected packages,
// `dev:check` when the dev scripts change, and for Rust, `cargo fmt` and a build through the dev
// runner (the same compile check `up` does, sharing its build folder).
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
const changed = [
	...new Set([
		...git(['diff', '--name-only', '-z', base]).split('\0'),
		...git(['ls-files', '-o', '--exclude-standard', '-z']).split('\0'),
	]),
].filter((file) => file && NodeFS.existsSync(NodePath.join(root, file)))

const under = (...folders: ReadonlyArray<string>) =>
	changed.some((file) => folders.some((folder) => file.startsWith(`${folder}/`)))
const matching = (pattern: RegExp) => changed.filter((file) => pattern.test(file))

// The packages CI typechecks and tests (ci.yml, js job), and what reaches each.
const PACKAGES = [
	{ name: '@amberite/backend', test: true, reachedBy: ['apps/backend'] },
	{ name: '@amberite/browser-bridge', test: true, reachedBy: ['packages/browser-bridge'] },
	{
		name: '@modrinth/app-frontend',
		test: false,
		reachedBy: [
			'apps/app-frontend',
			'packages/ui',
			'packages/api-client',
			'packages/assets',
			'packages/utils',
		],
	},
] as const

type Step = {
	readonly name: string
	readonly command: string
	readonly args: ReadonlyArray<string>
}
const steps: Step[] = []
const skipped: string[] = []

const scripts = matching(/\.(?:[cm]?[jt]s|vue)$/)
if (scripts.length > 0) {
	steps.push({ name: 'lint', command: 'vp', args: ['lint', '--deny-warnings', ...scripts] })
}
if (onPath('typos')) steps.push({ name: 'typos', command: 'typos', args: changed })
else skipped.push('typos (not installed)')
const toml = matching(/\.toml$/)
if (toml.length > 0) {
	if (onPath('tombi')) {
		steps.push({ name: 'tombi lint', command: 'tombi', args: ['lint', ...toml] })
		steps.push({ name: 'tombi fmt', command: 'tombi', args: ['fmt', '--check', ...toml] })
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
if (under('scripts') || changed.includes('dev.json')) {
	steps.push({ name: 'dev:check', command: 'vp', args: ['run', 'dev:check'] })
}
const rust = matching(/\.rs$/)
if (rust.length > 0) steps.push({ name: 'rustfmt', command: 'rustfmt', args: ['--check', ...rust] })
if (rust.length > 0 || matching(/(?:^|\/)Cargo\.(?:toml|lock)$/).length > 0) {
	steps.push({ name: 'rust build', command: 'node', args: ['scripts/dev-runner.ts', 'build'] })
}

console.log(`[check] ${changed.length} changed files since ${base.slice(0, 9)}`)
const results = steps.map((step) => {
	console.log(`\n[check] ${step.name}`)
	// Windows runs vp through its .cmd shim, which needs the shell. File names are quoted for it.
	const result =
		process.platform === 'win32' && step.command === 'vp'
			? NodeChildProcess.spawnSync(
					[step.command, ...step.args.map((arg) => `"${arg}"`)].join(' '),
					{ cwd: root, shell: true, stdio: 'inherit' },
				)
			: NodeChildProcess.spawnSync(step.command, step.args, { cwd: root, stdio: 'inherit' })
	return { name: step.name, ok: result.status === 0 }
})
console.log('\n[check] summary')
for (const { name, ok } of results) console.log(`  ${ok ? 'pass' : 'FAIL'}  ${name}`)
for (const name of skipped) console.log(`  skip  ${name}`)
if (results.some((result) => !result.ok)) process.exitCode = 1

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
