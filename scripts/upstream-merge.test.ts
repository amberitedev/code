import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

import { afterEach, expect, it } from 'vite-plus/test'

import { mergeUpstream } from './upstream-merge.ts'

const repos: string[] = []
afterEach(() => {
	for (const repo of repos.splice(0)) NodeFS.rmSync(repo, { recursive: true, force: true })
})

const lines = (prefix: string, count: number) =>
	Array.from({ length: count }, (_, index) => `${prefix} ${index}\n`).join('')

it('merges upstream into our tree and settles what the tree already answers', () => {
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
		'AGENTS.md': 'base\n',
		'lib/main.rs': `fn main() {}\n${lines('base', 10)}`,
		'lib/old.rs': lines('old', 5),
		'lib/gradlew': lines('gradle', 5),
		'lib/both.rs': 'base\n',
		'lib/cut.rs': lines('cut', 5),
		'lib/desktop.rs': `fn desktop() {}\n${lines('base', 10)}`,
		'lib/.sqlx/query-aaaa.json': `{"query": "SELECT 1",\n${lines('"describe"', 10)}}\n`,
		'site/page.rs': lines('page', 5),
		'split/one.rs': lines('one', 5),
		'split/two.rs': lines('two', 5),
	})
	git('add', '-A')
	git('update-index', '--chmod=+x', 'lib/gradlew')
	git('commit', '-q', '-m', 'base')

	git('switch', '-q', '-c', 'upstream')
	NodeFS.rmSync(NodePath.join(repo, 'lib/old.rs'))
	NodeFS.rmSync(NodePath.join(repo, 'lib/.sqlx/query-aaaa.json'))
	write({
		'AGENTS.md': 'upstream\n',
		'lib/main.rs': `fn main() { upstream() }\n${lines('base', 10)}`,
		'lib/new.rs': lines('new', 5),
		'lib/gradlew': lines('gradle', 6),
		'lib/both.rs': 'upstream\n',
		'lib/cut.rs': lines('cut', 6),
		'lib/desktop.rs': `fn desktop() { upstream() }\n${lines('base', 10)}`,
		'lib/.sqlx/query-bbbb.json': `{"query": "SELECT 2",\n${lines('"describe"', 10)}}\n`,
		'site/page.rs': lines('page', 6),
		'site/new.rs': lines('new', 5),
	})
	// Half of split/ goes each way, so git cannot say where our new file in it belongs. It reports
	// that as a message with no conflicted file.
	NodeFS.mkdirSync(NodePath.join(repo, 'left'))
	NodeFS.mkdirSync(NodePath.join(repo, 'right'))
	git('mv', 'split/one.rs', 'left/one.rs')
	git('mv', 'split/two.rs', 'right/two.rs')
	commit('upstream')
	const upstream = git('rev-parse', 'HEAD')

	git('switch', '-q', 'ours')
	git('mv', 'lib', 'engine')
	NodeFS.mkdirSync(NodePath.join(repo, 'app'))
	git('mv', 'engine/desktop.rs', 'app/desktop.rs')
	git('rm', '-q', '-r', '-f', 'site', 'engine/cut.rs')
	write({
		'AGENTS.md': 'ours\n',
		'engine/main.rs': `fn main() {}\nbase 0\n${lines('ours', 30)}`,
		'engine/both.rs': 'ours\n',
		// Too far from upstream's file for git to pair them by similarity.
		'app/desktop.rs': `fn desktop() {}\nbase 0\n${lines('ours', 30)}`,
		'split/ours.rs': lines('ours', 5),
	})
	commit('ours')

	const base = git('merge-base', 'ours', upstream)
	const merged = mergeUpstream({
		base,
		upstream,
		moved: { lib: 'engine', 'lib/desktop.rs': 'app/desktop.rs' },
		sides: [[/^AGENTS\.md$/, 'ours']],
		cwd: repo,
	})
	expect(merged).toEqual({
		conflicts: ['engine/both.rs'],
		dropped: [
			{ path: 'engine/cut.rs', folder: undefined },
			{ path: 'site/new.rs', folder: 'site' },
			{ path: 'site/page.rs', folder: 'site' },
		],
		// The hash-named file replaced another, and split/ only moved: neither is new.
		added: ['engine/new.rs'],
		notes: [expect.stringMatching(/^CONFLICT \(directory rename split\): .* split /)],
	})
	// The conflict is a real unmerged path, so either side can be checked out.
	expect(git('status', '--porcelain')).toContain('UU engine/both.rs')
	git('checkout', '--theirs', 'engine/both.rs')
	git('add', 'engine/both.rs')
	git('commit', '-q', '-m', 'merge')

	expect(git('rev-parse', 'HEAD^2')).toBe(upstream)
	expect(git('ls-tree', '-r', '--name-only', 'HEAD').split('\n')).toEqual([
		'AGENTS.md',
		'app/desktop.rs',
		'engine/.sqlx/query-bbbb.json',
		'engine/both.rs',
		'engine/gradlew',
		'engine/main.rs',
		'engine/new.rs',
		'left/one.rs',
		'right/two.rs',
		'split/ours.rs',
	])
	expect(git('show', 'HEAD:AGENTS.md')).toBe('ours')
	expect(git('show', 'HEAD:engine/both.rs')).toBe('upstream')
	expect(git('show', 'HEAD:engine/main.rs')).toBe(
		`fn main() { upstream() }\nbase 0\n${lines('ours', 30)}`.trim(),
	)
	expect(git('show', 'HEAD:app/desktop.rs')).toBe(
		`fn desktop() { upstream() }\nbase 0\n${lines('ours', 30)}`.trim(),
	)
	expect(git('ls-tree', 'HEAD', 'engine/gradlew')).toMatch(/^100755 /)
	expect(git('status', '--porcelain')).toBe('')
})
