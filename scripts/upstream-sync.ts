#!/usr/bin/env node

// Merges Modrinth into the current branch and prints a report of what is left to settle.
//
// Usage: node scripts/upstream-sync.ts [ref]        Start the merge and print the report.
//                                                   Without a ref, fetches Modrinth's main.
//        node scripts/upstream-sync.ts --continue   Once nothing is unmerged: regenerate the
//                                                   lockfiles and make the merge commit.
//
// `git merge --abort` undoes a started merge. A file or folder we deleted stays deleted, and the
// report lists every Modrinth change that left out.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'

import { type Merged, matches, mergeUpstream, ourPath } from './upstream-merge.ts'

const MODRINTH = 'https://github.com/modrinth/code.git'

// The folder we renamed. Its files are rewritten and hash-named, so git cannot pair them by
// similarity; a file we moved on its own still looks like Modrinth's, and git pairs it.
const MOVED = { 'packages/app-lib': 'packages/engine' }

// Files nobody merges by hand. When both sides changed one, this side wins.
const SIDES = [
	[/(^|\/)(AGENTS|CLAUDE)\.md$/, 'ours'],
	[/^README\.md$/, 'ours'],
	// Keyed by a hash of the query, so ours describes our schema.
	[/\/\.sqlx\//, 'ours'],
	// Modrinth's translators own every language but the source one.
	[/\/locales\/(?!en-US\/)[^/]+\/index\.json$/, 'theirs'],
	// `--continue` regenerates these against our manifests.
	[/^(Cargo\.lock|pnpm-lock\.yaml)$/, 'theirs'],
] as const

// Regenerated against our manifests after the merge. Starting from Modrinth's lockfile keeps its
// versions; each command prunes and tops it up. pnpm freezes the lockfile in CI unless told not to.
const LOCKFILES = {
	'Cargo.lock': 'cargo metadata --format-version 1',
	'pnpm-lock.yaml': 'pnpm install --lockfile-only --no-frozen-lockfile --ignore-scripts',
}

const MARKER = '^(<<<<<<<|>>>>>>>) '

// Files with conflict markers in `commit`, or in the index without one.
function markers(commit?: string) {
	// Git wants `--cached` before the pattern and a commit after it. A commit prefixes each path
	// with `<commit>:`.
	return commit
		? lines(probe('grep', '-I', '-l', '-E', '-e', MARKER, commit)).map((path) =>
				path.slice(commit.length + 1),
			)
		: lines(probe('grep', '-I', '-l', '-E', '--cached', '-e', MARKER))
}

const [argument] = process.argv.slice(2)
try {
	if (argument === '--continue') finish()
	else start(argument)
} catch (error) {
	// The message is the instruction; a stack trace buries it.
	console.error(error instanceof Error ? error.message : error)
	process.exitCode = 1
}

function start(ref: string | undefined) {
	if (git('status', '--porcelain', '--untracked-files=no')) {
		throw new Error('Commit or discard your changes first.')
	}
	const upstream = git('rev-parse', '--verify', `${ref ?? fetchModrinth()}^{commit}`)
	const [base, ...otherBases] = lines(git('merge-base', '--all', 'HEAD', upstream))
	if (!base || otherBases.length > 0) throw new Error('HEAD and Modrinth need one merge base.')
	if (base === upstream) {
		console.log('Already up to date with Modrinth.')
		return
	}

	const range = `${base}..${upstream}`
	const commits = lines(git('log', '--reverse', '--no-merges', '--format=%h\t%s', range))
	// `sha\n\npath\npath\n` per commit. Without rename detection a moved file names both its paths.
	const paths = new Map(
		git('log', '--no-merges', '--no-renames', '--name-only', '--format=%x01%h', range)
			.split('\x01')
			.filter(Boolean)
			.map((entry) => {
				const [sha = '', ...touched] = lines(entry)
				return [sha, touched] as const
			}),
	)

	const merged = mergeUpstream({ base, upstream, moved: MOVED, sides: SIDES, cwd: process.cwd() })
	NodeFS.writeFileSync(
		git('rev-parse', '--git-path', 'MERGE_MSG'),
		`chore: merge Modrinth (${commits.length} commits)\n`,
	)
	console.log(report({ base, upstream, commits, paths, merged }))
}

function finish() {
	if (!probe('rev-parse', '--verify', '--quiet', 'MERGE_HEAD')) {
		throw new Error('No merge to continue. Start one with: node scripts/upstream-sync.ts')
	}
	const unmerged = git('diff', '--name-only', '--diff-filter=U')
	if (unmerged) throw new Error(`Still unmerged:\n${unmerged}`)
	// A file that holds markers at HEAD, such as a merge fixture, is not ours to flag.
	const known = new Set(markers('HEAD'))
	// The index, since that is what the commit records.
	const marked = markers().filter((path) => !known.has(path))
	if (marked.length > 0) throw new Error(`Conflict markers left in:\n${marked.join('\n')}`)

	for (const [file, command] of Object.entries(LOCKFILES)) {
		// Shell, because pnpm is a .cmd on Windows. cargo metadata prints the whole graph to stdout.
		const { status } = NodeChildProcess.spawnSync(command, {
			shell: true,
			stdio: ['ignore', 'ignore', 'inherit'],
		})
		if (status !== 0) throw new Error(`Could not regenerate ${file} with \`${command}\`.`)
		git('add', '--', file)
	}
	// The hooks format and lint staged files, which here is all of Modrinth's changes.
	git('commit', '--quiet', '--no-verify', '--no-edit')
	console.log(git('log', '-1', '--format=Committed %h %s'))
	// The repo allows squashing, and a squash drops Modrinth as a parent.
	console.log('Land it with a merge commit. A squash or rebase makes the next sync start over.')
}

// Modrinth's main, with full history: a shallow one has no merge base with us.
function fetchModrinth() {
	const shallow = git('rev-parse', '--is-shallow-repository') === 'true'
	const depth = shallow ? ['--unshallow'] : []
	git('fetch', '--quiet', '--no-tags', ...depth, MODRINTH, '+main:refs/remotes/modrinth/main')
	return 'modrinth/main'
}

type Report = {
	base: string
	upstream: string
	// `sha\ttitle`, oldest first.
	commits: string[]
	// Modrinth's paths each commit touched, by short sha.
	paths: Map<string, string[]>
	merged: Merged
}

function report({ base, upstream, commits, paths, merged }: Report) {
	const dropped = new Map(merged.dropped.map((entry) => [entry.path, entry.folder]))
	const folders = [...new Set(dropped.values())].filter((folder) => folder !== undefined)
	const isDropped = (path: string) =>
		dropped.has(path) || folders.some((folder) => matches(path, folder))

	const log: string[] = []
	// Our paths that Modrinth touched, with the commits that did.
	const touching = new Map<string, string[]>()
	for (const commit of commits) {
		const [sha = '', title = ''] = commit.split('\t')
		const touched = (paths.get(sha) ?? []).map((path) => ourPath(path, MOVED))
		for (const path of touched) touching.set(path, [...(touching.get(path) ?? []), sha])
		log.push(`${touched.every(isDropped) ? 'dropped' : '       '} ${sha} ${title}`)
	}

	const generated = (path: string) => SIDES.some(([pattern]) => pattern.test(path))
	const files = merged.dropped
		.filter((entry) => entry.folder === undefined && !generated(entry.path))
		.map((entry) => `${entry.path} (${(touching.get(entry.path) ?? []).join(', ')})`)
	const cut = folders
		.map((folder) => ({
			folder,
			count: merged.dropped.filter((entry) => entry.folder === folder).length,
		}))
		.sort((a, b) => b.count - a.count)
		.map(({ folder, count }) => `${folder} (${count} files)`)

	const section = (title: string, hint: string, items: string[]) =>
		items.length > 0 ? ['', `## ${title}`, hint, '', ...items] : []
	return [
		`# Modrinth sync: ${commits.length} commits`,
		'',
		`base ${base}`,
		`upstream ${upstream}`,
		...section(
			`Conflicts (${merged.conflicts.length})`,
			'`git status` shows each one. Resolve, then `git add`.',
			merged.conflicts,
		),
		...section('Git also reported', 'These have no unmerged file.', merged.notes),
		...section(
			`Dropped (${files.length})`,
			'Modrinth changed a file we deleted, so the change was left out. Where our code does that job now, `git show <sha> -- <path>` is the change to carry over by hand.',
			files,
		),
		...section(
			'Dropped with their folder',
			'We deleted these folders, so changes and new files inside them were left out.',
			cut,
		),
		...section(
			`New files (${merged.added.length})`,
			'Modrinth added these and they came in.',
			merged.added,
		),
		...section(
			'Commits',
			'Nothing of a `dropped` commit merged. Read its title: a fix to code we rewrote still needs carrying over.',
			log,
		),
	].join('\n')
}

function git(...args: string[]) {
	const result = run(args)
	if (result.status !== 0) throw new Error(`git ${args[0]} failed:\n${result.stderr}`)
	return result.stdout.trim()
}

// For a command that answers "no" with a failing status, such as grep with no match.
function probe(...args: string[]) {
	return run(args).stdout.trim()
}

function run(args: string[]) {
	return NodeChildProcess.spawnSync('git', ['-c', 'core.quotePath=false', ...args], {
		encoding: 'utf8',
		maxBuffer: 1 << 28,
	})
}

function lines(text: string) {
	return text.split('\n').filter(Boolean)
}
