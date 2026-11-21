#!/usr/bin/env node

// Fetches the latest of each reference repo into .repos/<id>, for agents to read. A shallow clone
// with no history; .repos/ is gitignored, so nothing is committed. .repos/<id>/SYNCED.json names the
// commit it holds.
//
// Usage: node scripts/sync-repos.ts [id]   (default: every repo)

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

const REPOS = [
	{ id: 'modrinth', repository: 'https://github.com/modrinth/code.git', ref: 'main' },
] as const

const root = git(['rev-parse', '--show-toplevel'], process.cwd())
const only = process.argv[2]
const selected = only ? REPOS.filter((repo) => repo.id === only) : REPOS
if (selected.length === 0)
	throw new Error(`Unknown repo "${only}". Expected one of: ${REPOS.map((r) => r.id).join(', ')}`)

for (const repo of selected) {
	const target = NodePath.join(root, '.repos', repo.id)
	NodeFS.mkdirSync(NodePath.dirname(target), { recursive: true })
	// Cloned beside the target and renamed into place, so a failed sync leaves the old copy whole.
	const temp = NodeFS.mkdtempSync(NodePath.join(NodePath.dirname(target), `.sync-${repo.id}-`))
	try {
		const checkout = NodePath.join(temp, 'checkout')
		console.log(`Syncing ${repo.id} from ${repo.ref} (shallow clone, no history).`)
		git(
			[
				'clone',
				'--quiet',
				'--depth=1',
				'--no-tags',
				'--single-branch',
				`--branch=${repo.ref}`,
				repo.repository,
				checkout,
			],
			root,
		)
		const commit = git(['rev-parse', 'HEAD'], checkout)
		NodeFS.rmSync(NodePath.join(checkout, '.git'), { recursive: true, force: true })
		const synced = { repository: repo.repository, ref: repo.ref, commit }
		NodeFS.writeFileSync(
			NodePath.join(checkout, 'SYNCED.json'),
			`${JSON.stringify(synced, null, '\t')}\n`,
		)
		NodeFS.rmSync(target, { recursive: true, force: true })
		NodeFS.renameSync(checkout, target)
		console.log(`.repos/${repo.id} is at ${repo.ref} (${commit.slice(0, 12)}).`)
	} finally {
		NodeFS.rmSync(temp, { recursive: true, force: true })
	}
}

function git(args: string[], cwd: string): string {
	return NodeChildProcess.execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}
