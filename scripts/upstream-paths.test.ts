import { describe, expect, it } from 'vite-plus/test'

import {
	type UpstreamConfig,
	bucketOf,
	changedPaths,
	ourPath,
	parseNameStatus,
	removedPaths,
} from './upstream-paths.ts'

const config: UpstreamConfig = {
	ignore: ['apps/labrinth', 'packages/app-lib/AGENTS.md'],
	renamed: {
		'packages/app-lib': 'packages/engine',
		'packages/app-lib/src/api/friends.rs': 'apps/app/src/friends/api.rs',
	},
	ported: {
		'apps/labrinth/src/routes/v3/friends.rs': 'apps/backend/src/social/index.ts',
		'packages/app-lib/src/api/users.rs': 'apps/app/src/accounts/users.rs',
		'packages/ariadne/src/networking': 'apps/backend/src/social/socket.ts',
	},
}

describe('ourPath', () => {
	it.each([
		['packages/app-lib/src/state/mod.rs', 'packages/engine/src/state/mod.rs'],
		['packages/app-lib/src/api/friends.rs', 'apps/app/src/friends/api.rs'],
		['packages/app-lib-extra/src/lib.rs', 'packages/app-lib-extra/src/lib.rs'],
		['apps/app/src/main.rs', 'apps/app/src/main.rs'],
	])('maps %s to %s', (upstream, ours) => {
		expect(ourPath(upstream, config.renamed)).toBe(ours)
	})
})

describe('removedPaths', () => {
	it('deletes ignored paths and ported paths we no longer have, under both names', () => {
		const have = new Set(['packages/ariadne/src/networking'])
		expect(removedPaths(config, (path) => have.has(path))).toEqual([
			'apps/labrinth',
			'packages/app-lib/AGENTS.md',
			'packages/engine/AGENTS.md',
			'apps/labrinth/src/routes/v3/friends.rs',
			'packages/app-lib/src/api/users.rs',
			'packages/engine/src/api/users.rs',
		])
	})
})

describe('bucketOf', () => {
	const ours = changedPaths(
		parseNameStatus(
			'M\tpackages/engine/src/state/mod.rs\nR100\tpackages/app-lib/src/state/dirs.rs\tpackages/engine/src/state/dirs.rs',
		),
	)

	it.each([
		[
			'ported beats ignored',
			'M\tapps/labrinth/src/routes/v3/friends.rs\nM\tapps/labrinth/Cargo.toml',
			'port',
		],
		['a ported folder', 'M\tpackages/ariadne/src/networking/message.rs', 'port'],
		[
			'a moved ported file',
			'R097\tpackages/app-lib/src/api/users.rs\tpackages/app-lib/src/api/user.rs',
			'port',
		],
		[
			'only ignored paths',
			'M\tapps/labrinth/src/main.rs\nD\tpackages/app-lib/AGENTS.md',
			'ignored',
		],
		['a new page', 'A\tapps/app-frontend/src/pages/Worlds.vue', 'new'],
		[
			'a moved page',
			'R100\tapps/app-frontend/src/pages/A.vue\tapps/app-frontend/src/pages/B.vue',
			'other',
		],
		['a renamed path we changed', 'M\tpackages/app-lib/src/state/mod.rs', 'overlap'],
		['a renamed path we only moved', 'M\tpackages/app-lib/src/state/dirs.rs', 'other'],
		['a path we did not change', 'M\tpackages/app-lib/src/state/fs.rs', 'other'],
	] as const)('%s', (_, nameStatus, bucket) => {
		expect(bucketOf(parseNameStatus(nameStatus), config, ours)).toBe(bucket)
	})
})
