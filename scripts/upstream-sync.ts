#!/usr/bin/env node

// Merges Modrinth into the current branch as one real merge commit and prints a JSON summary for
// the agent that resolves it. Run by .github/workflows/upstream-sync.yml on a fresh
// sync/upstream/N branch; the /upstream-sync skill reads the summary.
//
// Usage: node scripts/upstream-sync.ts [upstream-ref] > summary.json   (default: modrinth/main)
//
// Paths listed under `ignore` in upstream.json are deleted from the merge, so they stay gone on
// every sync. Conflict markers are committed as they are; resolving them is the skill's job.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'

type UpstreamConfig = {
	// Upstream paths we never take. Deleted from every merge.
	ignore: string[]
	// Upstream labrinth paths that apps/backend ports. Read from modrinth/main, never merged.
	watch: string[]
}

// Each new upstream commit lands in one bucket. See bucketOf for the order they are checked in.
type Bucket = 'ignored' | 'watch' | 'new' | 'overlap' | 'other'
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

const base = git('merge-base', 'HEAD', upstream)
const upstreamSha = git('rev-parse', upstream)
const ours = new Set(lines(git('diff', '--name-only', base, 'HEAD')))
const commits = lines(git('rev-list', '--reverse', '--no-merges', `${base}..${upstream}`))

// --no-commit stops before committing even on a clean merge; it exits 1 when there are conflicts.
run('git', ['merge', '--no-commit', '--no-ff', upstream])
if (!run('git', ['rev-parse', '-q', '--verify', 'MERGE_HEAD'])) {
	throw new Error(`git merge ${upstream} did not start a merge`)
}

for (const path of config.ignore) {
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
	watch: [],
	new: [],
	overlap: [],
	other: [],
}
for (const sha of commits) {
	buckets[bucketOf(sha)].push({ sha, title: git('log', '-1', '--format=%s', sha) })
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

// Watch comes first because watch paths sit inside the ignored labrinth folder.
function bucketOf(sha: string): Bucket {
	// Renames show as `R<score>\told\tnew`, so a moved page is not counted as a new one.
	const changes = lines(git('show', '--find-renames', '--format=', '--name-status', sha)).map(
		(line) => {
			const [status, ...paths] = line.split('\t')
			return { added: status === 'A', paths }
		},
	)
	const paths = changes.flatMap((change) => change.paths)
	if (paths.some((path) => matches(path, config.watch))) return 'watch'
	if (paths.every((path) => matches(path, config.ignore))) return 'ignored'
	if (changes.some((change) => change.added && isNewSurface(change.paths[0]))) return 'new'
	if (paths.some((path) => ours.has(path))) return 'overlap'
	return 'other'
}

// A new app or package manifest, a new `*-page.vue`, or a new file under a pages or routes folder.
function isNewSurface(path: string) {
	return (
		/^(apps|packages)\/[^/]+\/(package\.json|Cargo\.toml)$/.test(path) ||
		/(^|\/)(pages|routes)\//.test(path) ||
		path.endsWith('-page.vue')
	)
}

function matches(path: string, prefixes: string[]) {
	return prefixes.some((prefix) => path === prefix || path.startsWith(`${prefix}/`))
}

function lines(text: string) {
	return text.split('\n').filter(Boolean)
}

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
