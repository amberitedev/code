// The merge step of scripts/upstream-sync.ts. Git pairs renamed files by similarity, so a file we
// moved and rewrote, or a hash-named file upstream replaced, turns into a tree conflict. We know
// every rename, so the base and upstream trees are rewritten to our paths first and the three-way
// merge has no rename of ours left to guess.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

import { type UpstreamConfig, ourPath } from './upstream-paths.ts'

type MergeOptions = {
	// Merge base of HEAD and `upstream`, both commit shas.
	base: string
	upstream: string
	renamed: UpstreamConfig['renamed']
	cwd: string
}

// Leaves the merged tree in the index and working tree with MERGE_HEAD set to upstream, so the next
// `git commit` makes the merge commit. Returns the conflicted paths; their markers are in the files,
// and git's explanation of each is printed to stderr. Needs git 2.45 or newer (merge-tree on trees).
export function mergeMapped({ base, upstream, renamed, cwd }: MergeOptions) {
	const git = (args: string[], options: { env?: NodeJS.ProcessEnv; input?: string } = {}) =>
		NodeChildProcess.spawnSync('git', args, {
			cwd,
			encoding: 'utf8',
			env: { ...process.env, ...options.env },
			input: options.input,
			maxBuffer: 1 << 28,
		})
	const ok = (...params: Parameters<typeof git>) => {
		const result = git(...params)
		if (result.status !== 0) throw new Error(`git ${params[0][0]} failed:\n${result.stderr}`)
		return result.stdout.trim()
	}

	if (base === upstream) throw new Error(`Already up to date with ${upstream}`)
	// read-tree would carry staged changes into the merge commit, where `git merge` refused them.
	if (git(['diff-index', '--quiet', '--cached', 'HEAD']).status !== 0) {
		throw new Error('The index has staged changes; commit or unstage them first')
	}

	// A fresh index takes `ls-tree` lines as they are, so modes, symlinks and submodules carry over.
	const mappedTree = (commit: string) => {
		const dir = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'upstream-merge-'))
		try {
			const env = { GIT_INDEX_FILE: NodePath.join(dir, 'index') }
			const input = ok(['ls-tree', '-r', '-z', '--full-tree', commit])
				.split('\0')
				.filter(Boolean)
				.map((entry) => {
					const tab = entry.indexOf('\t')
					return `${entry.slice(0, tab)}\t${ourPath(entry.slice(tab + 1), renamed)}\0`
				})
				.join('')
			ok(['update-index', '-z', '--index-info'], { env, input })
			return ok(['write-tree'], { env })
		} finally {
			NodeFS.rmSync(dir, { recursive: true, force: true })
		}
	}

	// Our mapped prefixes leave no rename to detect; directoryRenames still places files we added
	// inside a folder upstream moved on its own.
	const merge = git([
		'-c',
		'merge.directoryRenames=true',
		'merge-tree',
		'--write-tree',
		'-z',
		'--name-only',
		`--merge-base=${mappedTree(base)}`,
		'HEAD',
		mappedTree(upstream),
	])
	// Exit 1 means conflicts; anything else is an error.
	if (merge.status !== 0 && merge.status !== 1) {
		throw new Error(`git merge-tree failed:\n${merge.stderr}`)
	}
	// `<tree>\0<conflicted path>\0...\0\0` then messages as `<count>\0<paths>\0<type>\0<text>\0`.
	const end = merge.stdout.indexOf('\0\0')
	const [tree = '', ...conflicts] = merge.stdout
		.slice(0, end === -1 ? undefined : end)
		.split('\0')
		.filter(Boolean)
	const messages = end === -1 ? [] : merge.stdout.slice(end + 2).split('\0')
	// Some conflicts, such as a folder upstream split two ways, have a message and no conflicted file.
	const flagged: string[] = []
	for (let index = 0; index + 2 < messages.length;) {
		const count = Number(messages[index])
		const [type = '', text = ''] = messages.slice(index + 1 + count)
		if (type.startsWith('CONFLICT')) flagged.push(...messages.slice(index + 1, index + 1 + count))
		process.stderr.write(`${text.trimEnd()}\n`)
		index += count + 3
	}

	// MERGE_HEAD goes first: if a later step fails, git merge --abort undoes it.
	NodeFS.writeFileSync(
		NodePath.resolve(cwd, ok(['rev-parse', '--git-path', 'MERGE_HEAD'])),
		`${upstream}\n`,
	)
	ok(['read-tree', '-u', '-m', 'HEAD', tree])
	return [...new Set([...conflicts, ...flagged])].sort()
}
