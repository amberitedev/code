import { describe, expect, it } from 'vite-plus/test'

import {
	createProcessSpecs,
	createRuntimeEnvironment,
	findFirstAvailableOffset,
	formatGitDiffSummary,
	isBrowserAllowedPort,
	parseGitNumstat,
	parseInput,
	portsForOffset,
	processLabelsForMode,
	resolveConvexMode,
	resolveStartOffset,
	scenarioUsername,
	shouldPromptForCloudConvexPush,
} from './dev-runner.ts'

describe('dev runner ports', () => {
	it('keeps the documented ports in the primary checkout', () => {
		expect(
			resolveStartOffset({
				primaryPath: '/repo',
				worktreePath: '/repo',
			}),
		).toEqual({ offset: 0, source: 'primary checkout' })
		expect(portsForOffset(0)).toEqual({
			app: 1420,
			convexCloud: 3210,
			convexSite: 3211,
			core: 16662,
		})
	})

	it('gives a worktree a stable non-zero offset', () => {
		const first = resolveStartOffset({
			primaryPath: '/repo',
			worktreePath: '/repo-worktrees/feature',
		})
		const second = resolveStartOffset({
			primaryPath: '/repo',
			worktreePath: '/repo-worktrees/feature',
		})

		expect(first).toEqual(second)
		expect(first.offset).toBeGreaterThan(0)
	})

	it('honours explicit offsets and named development instances', () => {
		expect(
			resolveStartOffset({
				explicitOffset: '42',
				primaryPath: '/repo',
				worktreePath: '/repo',
			}),
		).toEqual({ offset: 42, source: 'AMBERITE_PORT_OFFSET=42' })

		const named = resolveStartOffset({
			devInstance: 'design-review',
			primaryPath: '/repo',
			worktreePath: '/repo',
		})
		expect(named.offset).toBeGreaterThan(0)
		expect(named.source).toBe('hashed AMBERITE_DEV_INSTANCE=design-review')
	})

	it('rejects invalid explicit offsets', () => {
		expect(() =>
			resolveStartOffset({
				explicitOffset: '-1',
				primaryPath: '/repo',
				worktreePath: '/repo',
			}),
		).toThrow('AMBERITE_PORT_OFFSET must be a non-negative integer')
	})

	it('moves the complete stack together when a port is occupied', async () => {
		const checked: number[] = []
		const offset = await findFirstAvailableOffset({
			checkPort: async (port) => {
				checked.push(port)
				return port !== 1420
			},
			mode: 'dev',
			startOffset: 0,
		})

		expect(offset).toBe(1)
		expect(checked).toContain(1420)
		expect(portsForOffset(offset)).toEqual({
			app: 1421,
			convexCloud: 3211,
			convexSite: 3212,
			core: 16663,
		})
	})

	it('checks only the ports started by a partial mode', async () => {
		const checked: number[] = []
		await findFirstAvailableOffset({
			checkPort: async (port) => {
				checked.push(port)
				return true
			},
			mode: 'dev:core',
			startOffset: 0,
		})

		expect(checked).toEqual([16662])
	})

	it('skips ports blocked by browsers', () => {
		expect(isBrowserAllowedPort(6000)).toBe(false)
		expect(isBrowserAllowedPort(16662)).toBe(true)
	})
})

describe('dev runner environment', () => {
	it('owns runtime URLs and removes ambient host and port overrides', () => {
		const env = createRuntimeEnvironment({
			baseEnv: {
				CONVEX_DEPLOYMENT: 'dev:must-not-escape',
				HOST: 'example.com',
				PORT: '9999',
				SHARED: 'kept',
			},
			convexMode: 'local',
			localDeployment: 'local-test',
			paths: {
				convexData: '/repo/.data/convex',
				coreData: '/repo/.data/core',
				data: '/repo/.data',
				primary: '/repo',
				runtime: '/repo/.data/runtime.json',
				scenariosData: '/repo/.data/scenarios',
				worktree: '/repo',
			},
			ports: portsForOffset(3),
		})

		expect(env.HOST).toBeUndefined()
		expect(env.PORT).toBeUndefined()
		expect(env.SHARED).toBe('kept')
		expect(env.AMBERITE_DEV_MODE).toBe('true')
		expect(env.CONVEX_DEPLOYMENT).toBe('local:local-test')
		expect(env.VITE_CONVEX_URL).toBe('http://127.0.0.1:3213')
		expect(env.VITE_CORE_URL).toBe('http://127.0.0.1:16665')
	})

	it('uses the configured cloud deployment in the primary checkout', () => {
		const env = createRuntimeEnvironment({
			baseEnv: {
				CONVEX_DEPLOYMENT: 'dev:example',
				CONVEX_SITE_URL: 'https://example.convex.site',
				CONVEX_URL: 'https://example.convex.cloud',
			},
			convexMode: 'cloud',
			paths: {
				convexData: '/repo/.data/convex',
				coreData: '/repo/.data/core',
				data: '/repo/.data',
				primary: '/repo',
				runtime: '/repo/.data/runtime.json',
				scenariosData: '/repo/.data/scenarios',
				worktree: '/repo',
			},
			ports: portsForOffset(0),
		})

		expect(env.VITE_CONVEX_URL).toBe('https://example.convex.cloud')
		expect(env.VITE_CONVEX_SITE_URL).toBe('https://example.convex.site')
	})

	it('rejects a production deployment in the primary checkout', () => {
		expect(() =>
			createRuntimeEnvironment({
				baseEnv: {
					CONVEX_DEPLOYMENT: 'prod:example',
					CONVEX_SITE_URL: 'https://example.convex.site',
					CONVEX_URL: 'https://example.convex.cloud',
				},
				convexMode: 'cloud',
				paths: {
					convexData: '/repo/.data/convex',
					coreData: '/repo/.data/core',
					data: '/repo/.data',
					primary: '/repo',
					runtime: '/repo/.data/runtime.json',
					scenariosData: '/repo/.data/scenarios',
					worktree: '/repo',
				},
				ports: portsForOffset(0),
			}),
		).toThrow('requires a cloud Convex development deployment')
	})

	it('chooses cloud only for the primary checkout', () => {
		expect(resolveConvexMode({ primary: '/repo', worktree: '/repo' })).toBe('cloud')
		expect(resolveConvexMode({ primary: '/repo', worktree: '/worktrees/main' })).toBe('local')
	})
})

describe('dev runner modes', () => {
	it('runs the complete product in dev mode', () => {
		expect(processLabelsForMode('dev')).toEqual(['convex', 'core', 'app-frontend'])
	})

	it('keeps partial modes focused', () => {
		expect(processLabelsForMode('dev:app')).toEqual(['app-frontend'])
		expect(processLabelsForMode('dev:core')).toEqual(['core'])
		expect(processLabelsForMode('dev:convex')).toEqual(['convex'])
	})
})

describe('Convex cloud push confirmation', () => {
	const changes = { additions: 18, deletions: 7, files: 3 }

	it('summarizes Git numstat output', () => {
		expect(
			parseGitNumstat('12\t4\tconvex/schema.ts\n6\t3\tconvex/friends.ts\n-\t-\tconvex/icon.png'),
		).toEqual(changes)
		expect(formatGitDiffSummary(changes, false)).toBe('+18 -7 in 3 files')
	})

	it('prompts only when the primary checkout will start Convex with changes', () => {
		expect(shouldPromptForCloudConvexPush('cloud', 'dev', changes)).toBe(true)
		expect(shouldPromptForCloudConvexPush('cloud', 'dev:convex', changes)).toBe(true)
		expect(shouldPromptForCloudConvexPush('cloud', 'dev:app', changes)).toBe(false)
		expect(shouldPromptForCloudConvexPush('local', 'dev', changes)).toBe(false)
		expect(
			shouldPromptForCloudConvexPush('cloud', 'dev', {
				additions: 0,
				deletions: 0,
				files: 0,
			}),
		).toBe(false)
	})
})

describe('dev runner scenarios', () => {
	it('uses dev.json defaults when no scenario is passed', () => {
		expect(parseInput(['dev'], [2, 4])).toEqual({
			dryRun: false,
			mode: 'dev',
			scenarios: [2, 4],
		})
	})

	it('accepts several scenarios and removes duplicates', () => {
		expect(parseInput(['dev', '1', '2', '1'], [4]).scenarios).toEqual([1, 2])
		expect(parseInput(['dev:app', '3'], [1]).scenarios).toEqual([3])
	})

	it('does not attach scenarios to backend-only modes', () => {
		expect(parseInput(['dev:core'], [1])).toEqual({
			dryRun: false,
			mode: 'dev:core',
			scenarios: [],
		})
		expect(() => parseInput(['dev:core', '2'], [1])).toThrow(
			'dev:core does not start App scenarios',
		)
	})

	it('derives the fake account from the scenario number', () => {
		expect(scenarioUsername(12)).toBe('scenario_12')
	})

	it('gives each App isolated persistent state', () => {
		const paths = {
			convexData: `${process.cwd()}/.data/convex`,
			coreData: `${process.cwd()}/.data/core`,
			data: `${process.cwd()}/.data`,
			primary: process.cwd(),
			runtime: `${process.cwd()}/.data/runtime.json`,
			scenariosData: `${process.cwd()}/.data/scenarios`,
			worktree: process.cwd(),
		}
		const specs = createProcessSpecs({
			branch: 'feature',
			convexMode: 'local',
			env: {
				VITE_CONVEX_SITE_URL: 'http://127.0.0.1:3211',
				VITE_CONVEX_URL: 'http://127.0.0.1:3210',
			},
			mode: 'dev',
			paths,
			ports: portsForOffset(0),
			scenarios: [1, 3],
		})

		expect(specs.map((spec) => spec.label)).toEqual([
			'convex',
			'core',
			'app-frontend',
			'app:1',
			'app:3',
		])
		const apps = specs.filter((spec) => spec.label.startsWith('app:'))
		const cwd = process.cwd().replaceAll('\\', '/')
		expect(apps.map((spec) => spec.env.THESEUS_CONFIG_DIR?.replaceAll('\\', '/'))).toEqual([
			`${cwd}/.data/scenarios/1`,
			`${cwd}/.data/scenarios/3`,
		])
		expect(apps.map((spec) => JSON.parse(spec.args.at(-1)!).username)).toEqual([
			'scenario_1',
			'scenario_3',
		])
	})
})
