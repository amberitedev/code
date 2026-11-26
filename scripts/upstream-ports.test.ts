import { expect, it } from 'vite-plus/test'

import { owing, readPorts } from './upstream-ports.ts'

it('reads what each sync owed and what became of it', () => {
	const merge = `chore: merge Modrinth (4 commits)\n${owing([
		'aaaaaaaaaa fix: sessions expire (#1)',
		'bbbbbbbbbb chore: bump stripe (#2)',
		'cccccccccc feat: user bans (#3)',
		'dddddddddd fix: friend requests (#4)',
	])}\nUpstream-skip: bbbbbbbbbb we have no Stripe\nUpstream-later: cccccccccc needs a ban table\n`
	const port = 'fix(backend): port session expiry from upstream\n\nUpstream: aaaaaaa\n'

	expect(readPorts([merge, port])).toEqual([
		{ sha: 'aaaaaaaaaa', title: 'fix: sessions expire (#1)', state: 'ported', note: '' },
		{
			sha: 'bbbbbbbbbb',
			title: 'chore: bump stripe (#2)',
			state: 'skipped',
			note: 'we have no Stripe',
		},
		{ sha: 'cccccccccc', title: 'feat: user bans (#3)', state: 'later', note: 'needs a ban table' },
		{ sha: 'dddddddddd', title: 'fix: friend requests (#4)', state: 'open', note: '' },
	])

	// A change put off in one sync is settled by a port in a later one, whatever order they are read in.
	const late = 'feat(backend): port user bans from upstream\n\nUpstream: cccccccccccccccccccc\n'
	expect(readPorts([late, merge]).find((entry) => entry.sha === 'cccccccccc')?.state).toBe('ported')
})
