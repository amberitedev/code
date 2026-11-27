import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeModule from 'node:module'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

import { expect, it } from 'vitest'

const WAIT = 'no-wait-in-tests'
const LIMIT = 'no-test-timeout-or-unquarantined-skip'

// Each source is linted as its own test file; the rule it breaks, or none.
const CASES: ReadonlyArray<readonly [source: string, rule?: string]> = [
	['await new Promise((resolve) => setTimeout(resolve, 100))', WAIT],
	["import { setTimeout as delay } from 'node:timers/promises'\nawait delay(10)", WAIT],
	["import * as Timers from 'node:timers/promises'\nawait Timers.setTimeout(10)", WAIT],
	['await sleep(50)', WAIT],
	['while (!done) await Promise.resolve()', WAIT],
	['for (;;) {\n\tif (ready()) break\n\tawait Promise.resolve()\n}', WAIT],
	['while (!done) await new Promise(setImmediate)', WAIT],
	['for (let i = 0; i < 3; i++) await Promise.resolve()'],
	["await new Promise((resolve) => socket.once('open', resolve))"],
	["setTimeout(() => emitter.emit('tick'))"],
	['await expect.poll(() => calls, { timeout: 5_000 }).toEqual([])'],
	["it('slow', () => {}, 5_000)", LIMIT],
	["test('slow', { timeout: 1_000 }, () => {})", LIMIT],
	["it('flaky', { retry: 2 }, () => {})", LIMIT],
	["it.each([1])('case %s', () => {}, 5_000)", LIMIT],
	["it('slow', () => {}, LONG)", LIMIT],
	["it('slow', () => {}, 30 * 1000)", LIMIT],
	['beforeAll(() => {}, 10_000)', LIMIT],
	['beforeEach(() => {}, LONG)', LIMIT],
	["test('fast', { concurrent: true }, () => {})"],
	["test('shared', options, () => {})"],
	["test('fast', () => {}, { concurrent: true, fails: false, sequential: false })"],
	["test('slow', options, () => {}, 5_000)", LIMIT],
	["test('dynamic', { [timeout]: 1 }, () => {})"],
	["test[only]('dynamic', () => {})"],
	['vi.setConfig({ testTimeout: 1 })', LIMIT],
	["it.skip('later', () => {})", LIMIT],
	["it.todo('later')", LIMIT],
	["xit('later', () => {})", LIMIT],
	["test('later', { skip: 'broken' }, () => {})", LIMIT],
	["it('later', () => {}, { skip: true })", LIMIT],
	["// quarantine: #12 flakes on Windows\nit('later', () => {}, { skip: true })"],
	["// quarantine: #12 flakes on Windows until the pty fix lands\nit.skip('later', () => {})"],
	["// quarantine: #12\nit.skip('later', () => {})", LIMIT],
	["describe.only('focused', () => {})", LIMIT],
	["// quarantine: #12 debugging\nit.only('focused', () => {})", LIMIT],
	["test('focused', { only: true }, () => {})", LIMIT],
	["it('focused', () => {}, { only: true })", LIMIT],
	["it.skipIf(process.platform === 'win32')('gated', () => {})"],
	["test('gated', { skip: windows }, () => {})"],
]

it('reports the waits, limits, skips and focus the testing doc forbids', () => {
	const folder = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'oxlint-plugin-'))
	try {
		const config = NodePath.join(folder, 'config.json')
		NodeFS.writeFileSync(
			config,
			JSON.stringify({
				jsPlugins: [NodePath.join(import.meta.dirname, 'oxlint-plugin.ts')],
				rules: { [`amberite/${WAIT}`]: 'error', [`amberite/${LIMIT}`]: 'error' },
			}),
		)
		const files = CASES.map(([source], index) => {
			const file = NodePath.join(folder, `case-${index}.test.ts`)
			NodeFS.writeFileSync(file, `${source}\n`)
			return file
		})
		// The oxlint that Vite+ runs for `vp lint`.
		const vitePlus = NodeModule.createRequire(import.meta.url).resolve('vite-plus/package.json')
		const oxlint = NodePath.join(
			NodePath.dirname(NodeModule.createRequire(vitePlus).resolve('oxlint/package.json')),
			'bin',
			'oxlint',
		)
		const result = NodeChildProcess.spawnSync(
			process.execPath,
			[oxlint, '-c', config, '--format', 'json', ...files],
			{ encoding: 'utf8' },
		)
		const { diagnostics } = JSON.parse(result.stdout) as {
			diagnostics: ReadonlyArray<{ code: string; filename: string }>
		}
		const found = files.map((file) =>
			diagnostics
				.filter(
					(diagnostic) =>
						NodePath.resolve(diagnostic.filename) === file &&
						diagnostic.code.startsWith('amberite('),
				)
				.map((diagnostic) => diagnostic.code.slice('amberite('.length, -1)),
		)
		expect(found.map((rules, index) => [CASES[index]![0], [...new Set(rules)]])).toEqual(
			CASES.map(([source, rule]) => [source, rule ? [rule] : []]),
		)
	} finally {
		NodeFS.rmSync(folder, { recursive: true, force: true })
	}
})
