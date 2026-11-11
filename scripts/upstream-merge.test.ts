import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

import { afterEach, expect, it } from 'vite-plus/test'

import { mergeMapped } from './upstream-merge.ts'

const repos: string[] = []
afterEach(() => {
	for (const repo of repos.splice(0)) NodeFS.rmSync(repo, { recursive: true, force: true })
})

const lines = (prefix: string, count: number) =>
	Array.from({ length: count }, (_, index) => `${prefix} ${index}\n`).join('')

it('merges upstream into our renamed folder by path', () => {
	const repo = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'upstream-merge-test-'))
	repos.push(repo)
	const git = (...args: string[]) =>
		NodeChildProcess.execFileSync('git', args, { cwd: repo, encoding: 'utf8' }).trim()
	const write = (files: Record<string, string>) => {
		for (const [path, content] of Object.entries(files)) {
			NodeFS.mkdirSync(NodePath.dirname(NodePath.join(repo, path)), { recursive: true })
			NodeFS.writeFileSync(NodePath.join(repo, path), content)
		}
	}
	const commit = (message: string) => {
		git('add', '-A')
		git('commit', '-q', '-m', message)
	}

	git('init', '-q', '-b', 'ours')
	for (const [key, value] of [
		['user.name', 'test'],
		['user.email', 'test@example.com'],
		['core.autocrlf', 'false'],
		// The fixture sets the executable bit in the index only, which a later add would undo on Linux.
		['core.fileMode', 'false'],
	]) {
		git('config', key, value)
	}
	write({
		'lib/main.rs': `fn main() {}\n${lines('base', 10)}`,
		'lib/old.rs': lines('old', 5),
		'lib/gradlew': lines('gradle', 5),
		'lib/both.rs': 'base\n',
		'lib/.sqlx/query-aaaa.json': `{"query": "SELECT 1",\n${lines('"describe"', 10)}}\n`,
	})
	git('add', '-A')
	git('update-index', '--chmod=+x', 'lib/gradlew')
	git('commit', '-q', '-m', 'base')

	git('switch', '-q', '-c', 'upstream')
	NodeFS.rmSync(NodePath.join(repo, 'lib/old.rs'))
	NodeFS.rmSync(NodePath.join(repo, 'lib/.sqlx/query-aaaa.json'))
	write({
		'lib/main.rs': `fn main() { upstream() }\n${lines('base', 10)}`,
		'lib/new.rs': lines('new', 5),
		'lib/gradlew': lines('gradle', 6),
		'lib/both.rs': 'upstream\n',
		'lib/.sqlx/query-bbbb.json': `{"query": "SELECT 2",\n${lines('"describe"', 10)}}\n`,
	})
	commit('upstream')
	const upstream = git('rev-parse', 'HEAD')

	git('switch', '-q', 'ours')
	git('mv', 'lib', 'engine')
	write({
		'engine/main.rs': `fn main() {}\nbase 0\n${lines('ours', 30)}`,
		'engine/both.rs': 'ours\n',
	})
	commit('ours')

	const base = git('merge-base', 'ours', upstream)
	expect(mergeMapped({ base, upstream, renamed: { lib: 'engine' }, cwd: repo })).toEqual([
		'engine/both.rs',
	])
	git('commit', '-q', '-m', 'merge')

	expect(git('rev-parse', 'HEAD^2')).toBe(upstream)
	expect(git('ls-tree', '-r', '--name-only', 'HEAD').split('\n')).toEqual([
		'engine/.sqlx/query-bbbb.json',
		'engine/both.rs',
		'engine/gradlew',
		'engine/main.rs',
		'engine/new.rs',
	])
	expect(git('show', 'HEAD:engine/main.rs')).toBe(
		`fn main() { upstream() }\nbase 0\n${lines('ours', 30)}`.trim(),
	)
	expect(git('show', 'HEAD:engine/both.rs')).toMatch(
		/^<<<<<<< [^\n]*\nours\n=======\nupstream\n>>>>>>> /,
	)
	expect(git('ls-tree', 'HEAD', 'engine/gradlew')).toMatch(/^100755 /)
	expect(git('status', '--porcelain')).toBe('')
})
