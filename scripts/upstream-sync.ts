#!/usr/bin/env node

// Merges Modrinth into the current branch as one real merge commit and prints a JSON summary for
// the agent that resolves it. Run by .github/workflows/upstream-sync.yml on a fresh
// sync/upstream/N branch; the /upstream-sync skill reads the summary.
//
// Usage: node scripts/upstream-sync.ts [upstream-ref] > summary.json   (default: modrinth/main)
//
// upstream.json's keys are described in docs/internals/upstream.md. Conflict markers are committed
// as they are; resolving them is the skill's job.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'

import {
	type Bucket,
	type UpstreamConfig,
	bucketOf,
	changedPaths,
	lines,
	parseNameStatus,
	removedPaths,
} from './upstream-paths.ts'

type Commit = { sha: string; title: string }

// Taken from Modrinth, then pruned and topped up to match our manifests. pnpm freezes the lockfile
// in CI unless told not to.
const LOCKFILES = {
	'Cargo.lock': ['cargo', 'metadata', '--format-version', '1'],
	'pnpm-lock.yaml': [
		'pnpm',
		'install',
		'--lockfile-only',
		'--no-frozen-lockfile',
		'--ignore-scripts',
	],
} as const

const upstream = process.argv[2] ?? 'modrinth/main'
const config: UpstreamConfig = JSON.parse(NodeFS.readFileSync('upstream.json', 'utf8'))
// Git quotes non-ASCII paths by default, and a quoted path matches no key in upstream.json.
const unquoted = ['-c', 'core.quotePath=false'] as const

const base = git('merge-base', 'HEAD', upstream)
const upstreamSha = git('rev-parse', upstream)
const ours = changedPaths(
	parseNameStatus(git(...unquoted, 'diff', '--find-renames', '--name-status', base, 'HEAD')),
)
const commits = lines(git('rev-list', '--reverse', '--no-merges', `${base}..${upstream}`))
const removed = removedPaths(config, (path) =>
	Boolean(git('ls-tree', '-r', '--name-only', 'HEAD', '--', path)),
)

// --no-commit stops before committing even on a clean merge; it exits 1 when there are conflicts.
// directoryRenames puts upstream's new files under our renamed folders.
run('git', ['-c', 'merge.directoryRenames=true', 'merge', '--no-commit', '--no-ff', upstream])
if (!run('git', ['rev-parse', '-q', '--verify', 'MERGE_HEAD'])) {
	throw new Error(`git merge ${upstream} did not start a merge`)
}

// Before the conflict list, so a modify/delete conflict on a ported path is not added back.
for (const path of removed) {
	if (git('ls-files', '--', path)) git('rm', '-r', '-q', '--force', '--', path)
}

const lockfiles: Record<string, 'regenerated' | 'failed'> = {}
for (const [file, command] of Object.entries(LOCKFILES)) {
	git('checkout', upstream, '--', file)
	const [bin, ...args] = command
	lockfiles[file] = run(bin, args) ? 'regenerated' : 'failed'
	git('add', '--', file)
}

const conflicts = lines(git('diff', '--name-only', '--diff-filter=U'))
if (conflicts.length > 0) git('add', '--', ...conflicts)
git(
	'commit',
	'--no-verify',
	'-m',
	`chore: merge ${upstream} (${commits.length} commits)`,
	'-m',
	conflicts.length > 0 ? `Unresolved conflicts:\n${conflicts.join('\n')}` : 'Merged cleanly.',
)

const buckets: Record<Bucket, Commit[]> = {
	ignored: [],
	port: [],
	new: [],
	overlap: [],
	other: [],
}
for (const sha of commits) {
	// --find-renames shows a move as one change, so a moved page is not counted as a new one.
	const changes = parseNameStatus(
		git(...unquoted, 'show', '--find-renames', '--format=', '--name-status', sha),
	)
	buckets[bucketOf(changes, config, ours)].push({
		sha,
		title: git('log', '-1', '--format=%s', sha),
	})
}

const summary = {
	upstream: upstreamSha,
	base,
	clean:
		conflicts.length === 0 && Object.values(lockfiles).every((state) => state === 'regenerated'),
	conflicts,
	lockfiles,
	commits: buckets,
}
console.log(JSON.stringify(summary, null, 2))

function git(...args: string[]) {
	return NodeChildProcess.execFileSync('git', args, {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'inherit'],
	}).trim()
}

// Runs a command with its output on stderr, so stdout stays the JSON summary. Returns success.
function run(bin: string, args: readonly string[]) {
	return NodeChildProcess.spawnSync(bin, args, { stdio: ['ignore', 2, 2] }).status === 0
}
