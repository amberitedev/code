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

const root = NodePath.join(import.meta.dirname, '..')
const repos = NodePath.join(root, '.repos')
const only = process.argv[2]
const selected = only ? REPOS.filter((repo) => repo.id === only) : REPOS
if (selected.length === 0)
	throw new Error(`Unknown repo "${only}". Expected one of: ${REPOS.map((r) => r.id).join(', ')}`)

NodeFS.mkdirSync(repos, { recursive: true })
// An interrupted run leaves its temp folder, a second full copy for every grep to find. A recent one
// may belong to a sync still running in this checkout, so only old ones go.
for (const name of NodeFS.readdirSync(repos)) {
	const leftover = NodePath.join(repos, name)
	if (name.startsWith('.sync-') && Date.now() - NodeFS.statSync(leftover).mtimeMs > 3_600_000)
		remove(leftover)
}

for (const repo of selected) {
	const target = NodePath.join(repos, repo.id)
	// Cloned beside the target and swapped in by renames, so a failed sync leaves the old copy whole.
	// Deleting the old copy in place could stop halfway on a locked file and leave a copy that looks
	// complete.
	const temp = NodeFS.mkdtempSync(NodePath.join(repos, `.sync-${repo.id}-`))
	let keepTemp = false
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
		remove(NodePath.join(checkout, '.git'))
		const synced = { repository: repo.repository, ref: repo.ref, commit }
		NodeFS.writeFileSync(
			NodePath.join(checkout, 'SYNCED.json'),
			`${JSON.stringify(synced, null, '\t')}\n`,
		)
		const old = NodePath.join(temp, 'old')
		if (NodeFS.existsSync(target)) NodeFS.renameSync(target, old)
		try {
			NodeFS.renameSync(checkout, target)
		} catch (error) {
			// If the old copy cannot go back either, it stays in the temp folder rather than being lost.
			keepTemp = NodeFS.existsSync(old)
			if (keepTemp) NodeFS.renameSync(old, target)
			keepTemp = false
			throw error
		}
		console.log(`.repos/${repo.id} is at ${repo.ref} (${commit.slice(0, 12)}).`)
	} finally {
		if (!keepTemp) remove(temp)
	}
}

// Retries ride out the brief locks antivirus and indexers take on Windows.
function remove(path: string) {
	NodeFS.rmSync(path, { recursive: true, force: true, maxRetries: 5 })
}

function git(args: string[], cwd: string): string {
	return NodeChildProcess.execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}
