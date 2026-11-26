#!/usr/bin/env node

// Merges Modrinth into the current branch. The /upstream-sync skill runs it and works from the
// report it prints.
//
// Usage: node scripts/upstream-sync.ts [ref]        Start the merge and print the report.
//                                                   Without a ref, fetches Modrinth's main.
//        node scripts/upstream-sync.ts --continue   Once nothing is unmerged: regenerate the
//                                                   lockfiles and make the merge commit.
//        node scripts/upstream-sync.ts --ports      List the Modrinth changes still owed to code
//                                                   we rewrote. Fails while one has no trailer.
//
// `git merge --abort` undoes a started merge. What the merge settles without asking is in
// scripts/upstream-merge.ts and scripts/upstream-map.ts; the reasons are in docs/internals/upstream.md.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'

import { MOVED, PORTED, SIDES } from './upstream-map.ts'
import { type Merged, lookup, matches, mergeUpstream, ourPath } from './upstream-merge.ts'
import { type Port, owing, readPorts } from './upstream-ports.ts'

const MODRINTH = 'https://github.com/modrinth/code.git'

// Regenerated against our manifests after the merge. Starting from Modrinth's lockfile keeps its
// versions; each command prunes and tops it up. pnpm freezes the lockfile in CI unless told not to.
const LOCKFILES = {
	'Cargo.lock': 'cargo metadata --format-version 1',
	'pnpm-lock.yaml': 'pnpm install --lockfile-only --no-frozen-lockfile --ignore-scripts',
}

const MARKERS = ['grep', '-I', '-l', '-E', '^(<<<<<<<|>>>>>>>) ']

const [argument] = process.argv.slice(2)
try {
	if (argument === '--continue') finish()
	else if (argument === '--ports') ports()
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
	const map = { ...MOVED, ...PORTED }
	const mapErrors = [
		...Object.keys(map)
			.filter((path) => !git('ls-tree', upstream, '--', path))
			.map((path) => `${path} is gone from Modrinth`),
		...Object.values(map)
			.filter((path) => !git('ls-tree', 'HEAD', '--', path))
			.map((path) => `${path} is gone from our tree`),
	]

	const owed = history().filter((port) => port.state === 'open' || port.state === 'later')
	const merged = mergeUpstream({ base, upstream, moved: MOVED, sides: SIDES, cwd: process.cwd() })
	const { text, toPort } = report({ base, upstream, commits, paths, merged, mapErrors, owed })
	NodeFS.writeFileSync(
		git('rev-parse', '--git-path', 'MERGE_MSG'),
		`chore: merge Modrinth (${commits.length} commits)\n${owing(toPort)}`,
	)
	console.log(text)
}

function ports() {
	const all = history()
	const inState = (state: Port['state']) => all.filter((port) => port.state === state)
	const [open, later] = [inState('open'), inState('later')]
	console.log(`${inState('ported').length} ported, ${inState('skipped').length} skipped.`)
	if (later.length > 0) console.log(['', 'Later:', ...later.map(describe)].join('\n'))
	if (open.length > 0) {
		throw new Error(
			[
				'',
				'No trailer yet. Port each one in a commit that ends with `Upstream: <sha>`, or put',
				'`Upstream-skip: <sha> <why>` or `Upstream-later: <sha> <why>` on any commit:',
				...open.map(describe),
			].join('\n'),
		)
	}
}

// Every change a sync in this branch's history owed, read from the commits that mention one.
function history() {
	const mentions = '^(To port:|Upstream(-skip|-later)?: )'
	return readPorts(git('log', '-E', `--grep=${mentions}`, '--format=%B%x01').split('\x01'))
}

function describe(port: Port) {
	return `${port.sha} ${port.title}${port.note && `: ${port.note}`}`
}

function finish() {
	if (!probe('rev-parse', '--verify', '--quiet', 'MERGE_HEAD')) {
		throw new Error('No merge to continue. Start one with: node scripts/upstream-sync.ts')
	}
	const unmerged = git('diff', '--name-only', '--diff-filter=U')
	if (unmerged) throw new Error(`Still unmerged:\n${unmerged}`)
	// A file that holds markers at HEAD, such as a merge fixture, is not ours to flag.
	const known = new Set(lines(probe(...MARKERS, 'HEAD')).map((path) => path.replace(/^HEAD:/, '')))
	const marked = lines(probe(...MARKERS)).filter((path) => !known.has(path))
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
	mapErrors: string[]
	// Changes earlier syncs left unported.
	owed: Port[]
}

function report({ base, upstream, commits, paths, merged, mapErrors, owed }: Report) {
	const index = new Set(lines(git('ls-files')))
	const dropped = new Map(merged.dropped.map((entry) => [entry.path, entry.folder]))
	const folders = [...new Set(dropped.values())].filter((folder) => folder !== undefined)
	const isCut = (path: string) =>
		dropped.has(path) || folders.some((folder) => matches(path, folder))
	// Where a change to Modrinth's `path` has to be carried by hand. A ported file we only moved is
	// neither dropped nor at its own path: git merged the change into our file.
	const target = (path: string) => {
		const ours = ourPath(path, MOVED)
		return dropped.has(ours) || index.has(ours) ? lookup(path, PORTED)?.[1] : undefined
	}

	const toPort: string[] = []
	const porting: string[] = []
	const log: string[] = []
	// Our paths that Modrinth touched, with the commits that did, minus the ones listed for porting.
	const touching = new Map<string, string[]>()
	for (const commit of commits) {
		const [sha = '', title = ''] = commit.split('\t')
		const touched = (paths.get(sha) ?? []).map((path) => ({
			path,
			ours: ourPath(path, MOVED),
			target: target(path),
		}))
		const ports = touched.filter((change) => change.target !== undefined)
		for (const change of touched) {
			if (change.target === undefined) {
				touching.set(change.ours, [...(touching.get(change.ours) ?? []), sha])
			}
		}
		if (ports.length > 0) {
			toPort.push(`${sha} ${title}`)
			porting.push(
				`${sha} ${title}`,
				...ports.map((change) => `    ${change.path} -> ${change.target}`),
			)
		}
		const kind =
			ports.length > 0 ? 'port' : touched.every((change) => isCut(change.ours)) ? 'cut ' : '    '
		log.push(`${kind} ${sha} ${title}`)
	}

	const generated = (path: string) => SIDES.some(([pattern]) => pattern.test(path))
	const loose = merged.dropped
		.filter((entry) => entry.folder === undefined && !generated(entry.path))
		.flatMap((entry) => {
			const shas = touching.get(entry.path)
			return shas ? [`${entry.path} (${shas.join(', ')})`] : []
		})
	const cut = folders
		.map((folder) => ({
			folder,
			files: merged.dropped.filter((entry) => entry.folder === folder).length,
		}))
		.sort((a, b) => b.files - a.files)
		.map(({ folder, files }) => `${folder} (${files} files)`)

	const section = (title: string, hint: string, items: string[]) =>
		items.length > 0 ? ['', `## ${title}`, hint, '', ...items] : []
	const text = [
		`# Modrinth sync: ${commits.length} commits`,
		'',
		`base ${base}`,
		`upstream ${upstream}`,
		...section('Fix scripts/upstream-map.ts', 'A path it names has moved or is gone.', mapErrors),
		...section(
			`Conflicts (${merged.conflicts.length})`,
			'`git status` shows each one. Resolve, then `git add`.',
			merged.conflicts,
		),
		...section('Git also reported', 'These have no unmerged file.', merged.notes),
		...section(
			`To port (${toPort.length})`,
			'Modrinth changed code we rewrote, so nothing merged. `git show <sha> -- <path>` is the change. Each commit here needs a trailer before the sync is done: see scripts/upstream-ports.ts.',
			porting,
		),
		...section(
			`Still owed from earlier syncs (${owed.length})`,
			'Put off before. Port it now, or leave it for the next sync to list again.',
			owed.map(describe),
		),
		...section(
			`Dropped (${loose.length})`,
			'Modrinth changed a file we deleted, so the change was left out. Nothing to do for one we cut. If we only moved it, add it to MOVED and start over. If we rewrote it, port the change and add it to PORTED.',
			loose,
		),
		...section(
			'Dropped with their folder',
			'We deleted these folders, so changes and new files inside them were left out. One we moved belongs in MOVED, one we rewrote in PORTED.',
			cut,
		),
		...section(
			`New files (${merged.added.length})`,
			'Modrinth added these and they came in.',
			merged.added,
		),
		...section('Commits', '`port` is listed above. Nothing of a `cut` commit is in our tree.', log),
	].join('\n')
	return { text, toPort }
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
