// The merge step of scripts/upstream-sync.ts. It merges Modrinth into HEAD and settles everything
// our tree already answers, so only real decisions are left unmerged.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

type MergeOptions = {
	// Merge base of HEAD and `upstream`, both commit shas.
	base: string
	upstream: string
	// Modrinth's path prefix to ours, for folders git cannot pair by similarity.
	moved: Record<string, string>
	// Files a side wins without a conflict, when both changed one.
	sides: readonly (readonly [RegExp, 'ours' | 'theirs'])[]
	cwd: string
}

export type Merged = {
	// Our paths still unmerged. `git status` shows each one's kind.
	conflicts: string[]
	// Modrinth's changes left out because we deleted the file or, with `folder`, the whole folder.
	dropped: { path: string; folder?: string }[]
	// Modrinth's new files that came in.
	added: string[]
	// Conflicts git reports with no unmerged file, such as a folder Modrinth split two ways.
	notes: string[]
}

export function matches(path: string, prefix: string) {
	return path === prefix || path.startsWith(`${prefix}/`)
}

// The value of the longest matching key, and that key.
export function lookup(path: string, map: Record<string, string>) {
	return Object.entries(map)
		.filter(([prefix]) => matches(path, prefix))
		.sort(([a], [b]) => b.length - a.length)[0]
}

export function ourPath(path: string, moved: Record<string, string>) {
	const [from, to] = lookup(path, moved) ?? [path, path]
	return to + path.slice(from.length)
}

// Leaves the merge in the index and working tree as `git merge` would, with MERGE_HEAD set to
// upstream: the next `git commit` makes the merge commit and `git merge --abort` undoes it. Needs a
// clean index.
export function mergeUpstream({ base, upstream, moved, sides, cwd }: MergeOptions): Merged {
	const git = (args: string[], options: { env?: NodeJS.ProcessEnv; input?: string } = {}) =>
		NodeChildProcess.spawnSync('git', ['-c', 'core.quotePath=false', ...args], {
			cwd,
			encoding: 'utf8',
			env: { ...process.env, ...options.env },
			input: options.input,
			maxBuffer: 1 << 28,
		})
	const ok = (...params: Parameters<typeof git>) => {
		const result = git(...params)
		if (result.status !== 0) throw new Error(`git ${params[0][0]} failed:\n${result.stderr}`)
		return result.stdout
	}
	const list = (...params: Parameters<typeof git>) =>
		ok(...params)
			.split('\0')
			.filter(Boolean)
	// A path list on stdin, since hundreds of paths overflow a Windows command line.
	const each = (command: string[], paths: string[]) => {
		if (paths.length === 0) return
		ok([...command, '--pathspec-from-file=-', '--pathspec-file-nul'], { input: paths.join('\0') })
	}

	// Git pairs renamed files by similarity, which fails for a folder we moved and rewrote and for
	// its hash-named files. We know those renames, so each tree is rewritten to our paths first. A
	// fresh index takes `ls-tree` lines as they are, so modes, symlinks and submodules carry over.
	const mappedTree = (commit: string) => {
		const dir = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'upstream-merge-'))
		try {
			const env = { GIT_INDEX_FILE: NodePath.join(dir, 'index') }
			const input = list(['ls-tree', '-r', '-z', '--full-tree', commit])
				.map((entry) => {
					const tab = entry.indexOf('\t')
					return `${entry.slice(0, tab)}\t${ourPath(entry.slice(tab + 1), moved)}\0`
				})
				.join('')
			ok(['update-index', '-z', '--index-info'], { env, input })
			return ok(['write-tree'], { env }).trim()
		} finally {
			NodeFS.rmSync(dir, { recursive: true, force: true })
		}
	}
	const folders = (tree: string) =>
		new Set(list(['ls-tree', '-r', '-d', '-z', '--name-only', tree]))

	const [baseTree, upstreamTree] = [mappedTree(base), mappedTree(upstream)]
	const [baseFolders, ourFolders] = [folders(baseTree), folders('HEAD')]
	// The topmost folder of `path` that Modrinth had at the merge base and we no longer have.
	const deletedFolder = (path: string) => {
		const parts = path.split('/')
		for (let depth = 1; depth < parts.length; depth++) {
			const folder = parts.slice(0, depth).join('/')
			if (baseFolders.has(folder) && !ourFolders.has(folder)) return folder
		}
	}

	// MERGE_HEAD goes first: if a later step fails, git merge --abort undoes it.
	NodeFS.writeFileSync(
		NodePath.resolve(cwd, ok(['rev-parse', '--git-path', 'MERGE_HEAD']).trim()),
		`${upstream}\n`,
	)
	// directoryRenames places Modrinth's new files inside a folder we moved; zdiff3 puts the merge
	// base in each conflict, which shows what Modrinth changed.
	const merge = git(
		[
			'-c',
			'merge.directoryRenames=true',
			'-c',
			'merge.conflictStyle=zdiff3',
			'merge-recursive',
			baseTree,
			'--',
			'HEAD',
			upstreamTree,
		],
		{ env: { [`GITHEAD_${upstreamTree}`]: 'modrinth' } },
	)
	// Exit 1 means conflicts; anything else is an error.
	if (merge.status !== 0 && merge.status !== 1) {
		throw new Error(`git merge-recursive failed:\n${merge.stdout}${merge.stderr}`)
	}

	// `<mode> <object> <stage>\t<path>` for each side an unmerged path has: 1 base, 2 ours, 3 theirs.
	const stages = new Map<string, Set<string>>()
	for (const entry of list(['ls-files', '-u', '-z'])) {
		const tab = entry.indexOf('\t')
		const path = entry.slice(tab + 1)
		stages.set(path, (stages.get(path) ?? new Set()).add(entry.slice(tab - 1, tab)))
	}
	const settled = { remove: [] as string[], ours: [] as string[], theirs: [] as string[] }
	const merged: Merged = { conflicts: [], dropped: [], added: [], notes: [] }
	for (const [path, has] of stages) {
		const side = sides.find(([pattern]) => pattern.test(path))?.[1]
		if (has.has('1') && !has.has('2')) {
			// We deleted it and Modrinth changed it. It stays deleted.
			settled.remove.push(path)
			merged.dropped.push({ path, folder: deletedFolder(path) })
		} else if (side && has.has('2')) {
			if (side === 'ours') settled.ours.push(path)
			else if (has.has('3')) settled.theirs.push(path)
			else settled.remove.push(path)
		} else {
			merged.conflicts.push(path)
		}
	}
	each(['checkout', '--ours'], settled.ours)
	each(['checkout', '--theirs'], settled.theirs)
	each(['add'], [...settled.ours, ...settled.theirs])
	each(['rm', '-q', '-f'], settled.remove)

	// Modrinth's new and moved files. One that lands in a folder we deleted is not ours to take;
	// only a new one is reported as added. `-z` lists `A\0<path>` or `R<score>\0<from>\0<to>`.
	const unwanted: string[] = []
	const arrived = list([
		'diff',
		'--cached',
		'--find-renames',
		'--name-status',
		'--diff-filter=AR',
		'-z',
		'HEAD',
	])
	for (let index = 0; index < arrived.length;) {
		const isNew = arrived[index]?.startsWith('A')
		index += isNew ? 1 : 2
		const path = arrived[index++] ?? ''
		const folder = deletedFolder(path)
		if (folder !== undefined) {
			unwanted.push(path)
			merged.dropped.push({ path, folder })
		} else if (isNew) merged.added.push(path)
	}
	each(['rm', '-q', '-f'], unwanted)

	merged.notes = merge.stdout
		.split('\n')
		.filter((line) => /^CONFLICT \((?!content|modify\/delete|rename\/delete)/.test(line))
	merged.conflicts.sort()
	merged.dropped.sort((a, b) => (a.path < b.path ? -1 : 1))
	return merged
}
