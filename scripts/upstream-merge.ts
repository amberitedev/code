// The merge step of scripts/upstream-sync.ts. Git pairs renamed files by similarity, so a file we
// moved and rewrote, or a hash-named file upstream replaced, turns into a tree conflict. We know
// every rename, so the base and upstream trees are rewritten to our paths first and the three-way
// merge has no rename of ours left to guess.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

import { type UpstreamConfig, ourPath } from './upstream-paths.ts'

// Leaves the merged tree in the index and working tree with MERGE_HEAD set to upstream, so the next
// `git commit` makes the merge commit. Returns the conflicted paths; their markers are in the files.
// Needs git 2.45 or newer (merge-tree on trees) and a clean working tree.
export function mergeMapped(upstream: string, renamed: UpstreamConfig['renamed'], cwd: string) {
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

	// A fresh index takes `ls-tree` lines as they are, so modes, symlinks and submodules carry over.
	const mappedTree = (commit: string) => {
		const dir = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'upstream-merge-'))
		try {
			const env = { GIT_INDEX_FILE: NodePath.join(dir, 'index') }
			const input = ok(['ls-tree', '-r', '-z', '--full-tree', commit])
				.split('\0')
				.filter(Boolean)
				.map((entry) => {
					const [info, path = ''] = entry.split('\t')
					return `${info}\t${ourPath(path, renamed)}\0`
				})
				.join('')
			ok(['update-index', '-z', '--index-info'], { env, input })
			return ok(['write-tree'], { env })
		} finally {
			NodeFS.rmSync(dir, { recursive: true, force: true })
		}
	}

	const upstreamSha = ok(['rev-parse', '--verify', `${upstream}^{commit}`])
	const base = ok(['merge-base', 'HEAD', upstreamSha])
	const merge = git([
		'merge-tree',
		'--write-tree',
		'-z',
		'--name-only',
		'--no-messages',
		`--merge-base=${mappedTree(base)}`,
		'HEAD',
		mappedTree(upstreamSha),
	])
	// Exit 1 means conflicts; anything else is an error.
	if (merge.status !== 0 && merge.status !== 1) {
		throw new Error(`git merge-tree failed:\n${merge.stderr}`)
	}
	const [tree = '', ...conflicts] = merge.stdout.split('\0').filter(Boolean)

	ok(['read-tree', '-u', '-m', 'HEAD', tree])
	NodeFS.writeFileSync(
		NodePath.resolve(cwd, ok(['rev-parse', '--git-path', 'MERGE_HEAD'])),
		`${upstreamSha}\n`,
	)
	return conflicts
}
