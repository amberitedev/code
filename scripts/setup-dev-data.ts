#!/usr/bin/env node

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

const worktree = git(['rev-parse', '--show-toplevel'], process.cwd())
const primary = primaryWorktree(worktree)
const destination = NodePath.join(worktree, '.data')
const source = NodePath.join(primary, '.data')

if (!samePath(worktree, primary) && NodeFS.existsSync(source)) {
	NodeFS.mkdirSync(destination, { recursive: true })
	let copied = false
	for (const entry of NodeFS.readdirSync(source)) {
		if (entry === 'runtime.json') continue
		const target = NodePath.join(destination, entry)
		if (NodeFS.existsSync(target)) continue
		NodeFS.cpSync(NodePath.join(source, entry), target, { recursive: true })
		copied = true
	}
	if (copied) console.log(`[setup] copied development data from ${source}`)
}

const convexData = NodePath.join(destination, 'convex')
const legacyConvexData = NodePath.join(primary, '.convex')
if (
	!samePath(worktree, primary) &&
	!NodeFS.existsSync(convexData) &&
	NodeFS.existsSync(legacyConvexData)
) {
	NodeFS.cpSync(legacyConvexData, convexData, { recursive: true })
}

const usesLegacyConvexDirectory =
	samePath(worktree, primary) &&
	!NodeFS.existsSync(convexData) &&
	NodeFS.existsSync(legacyConvexData)
for (const path of [
	...(usesLegacyConvexDirectory ? [] : [convexData]),
	NodePath.join(destination, 'core'),
	...([1, 2, 3, 4] as const).map((scenario) =>
		NodePath.join(destination, 'scenarios', String(scenario)),
	),
]) {
	NodeFS.mkdirSync(path, { recursive: true })
}

function primaryWorktree(cwd: string): string {
	const output = git(['worktree', 'list', '--porcelain'], cwd)
	const first = output
		.split(/\r?\n/)
		.find((line) => line.startsWith('worktree '))
		?.slice('worktree '.length)
	return NodePath.resolve(first ?? cwd)
}

function git(args: ReadonlyArray<string>, cwd: string): string {
	const result = NodeChildProcess.spawnSync('git', args, { cwd, encoding: 'utf8' })
	if (result.status !== 0) throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`)
	return result.stdout.trim()
}

function samePath(left: string, right: string): boolean {
	const normalize = (path: string) => {
		const resolved = NodePath.resolve(path)
		return process.platform === 'win32' ? resolved.toLowerCase() : resolved
	}
	return normalize(left) === normalize(right)
}
