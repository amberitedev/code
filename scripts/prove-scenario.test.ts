import { describe, expect, it } from 'vite-plus/test'

import { readSide, renderProof, type Side, upload, verdict } from './prove-scenario.ts'

const report = (...attempts: ReadonlyArray<{ status: string; code?: string }>) => ({
	run: {
		results: attempts.map(({ status, code }) => ({
			titlePath: ['a scenario'],
			attempts: [{ status, ...(code ? { error: { code, message: 'why' } } : {}) }],
		})),
	},
})

describe('readSide', () => {
	it.each([
		['a pass', report({ status: 'passed' }), 'passed', undefined],
		[
			'a failed expectation',
			report({ status: 'failed', code: 'ASSERTION_FAILED' }),
			'failed',
			undefined,
		],
		// A control the scenario did not find says nothing about the bug.
		[
			'any other error',
			report({ status: 'failed', code: 'LOCATOR_NOT_FOUND' }),
			'error',
			'scenario',
		],
		['no report', undefined, 'error', 'app'],
		[
			'two tests in the file',
			report({ status: 'passed' }, { status: 'failed' }),
			'error',
			'scenario',
		],
	])('reads %s', (_, input, status, stopped) => {
		expect(readSide('results', input)).toMatchObject({ status, ...(stopped ? { stopped } : {}) })
	})
})

describe('verdict', () => {
	const side = (status: Side['status'], stopped?: Side['stopped']): Side => ({
		status,
		title: 'a scenario',
		captures: [],
		...(stopped ? { stopped } : {}),
	})

	it.each([
		['fails on base, passes here', side('passed'), side('failed'), 0],
		['passes on both', side('passed'), side('passed'), 1],
		['fails here', side('failed'), undefined, 1],
		['the App did not run on base', side('passed'), side('error', 'app'), 2],
		['the scenario stopped early here', side('error', 'scenario'), undefined, 3],
	])('%s exits %i', (_, head, base, exit) => {
		expect(verdict(head, base).exit).toBe(exit)
	})
})

describe('upload', () => {
	const print = (...lines: string[]) =>
		`node -e "${lines.map((line) => `console.log('${line}')`).join(';')}"`

	it('pairs each capture with the URL on its line', () => {
		expect([...upload(print('https://a', 'https://b'), ['one.png', 'two.png'])]).toEqual([
			['one.png', 'https://a'],
			['two.png', 'https://b'],
		])
	})

	it('links nothing when the lines do not match the captures', () => {
		expect(upload(print('https://a'), ['one.png', 'two.png']).size).toBe(0)
		expect(upload(print(''), ['one.png']).size).toBe(0)
	})
})

describe('renderProof', () => {
	it('puts each capture beside the one base took under the same name', () => {
		const proof = renderProof({
			title: 'a server page names its server',
			result: { text: 'Proven', exit: 0 },
			base: {
				status: 'failed',
				title: '',
				captures: ['base/001-page.png', 'base/002-failure.png'],
			},
			head: { status: 'passed', title: '', captures: ['head/001-page.png'] },
			baseName: 'main',
			link: (capture) => `./${capture}`,
		})
		expect(proof).toContain('### A server page names its server')
		expect(proof).toContain('| ![Before](./base/001-page.png) | ![After](./head/001-page.png) |')
		expect(proof).not.toContain('002-failure')
	})
})
