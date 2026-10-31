import { describe, expect, it } from 'vite-plus/test'

import {
	bridgeUrlFromLine,
	createProcessSpecs,
	createRuntimeEnvironment,
	findFirstAvailableOffset,
	isBrowserAllowedPort,
	parseInput,
	portsForOffset,
	processLabelsForMode,
	programKey,
	resolveStartOffset,
	scenarioUsername,
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
			backend: 8787,
			accountWeb: 3100,
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
		).toEqual({ offset: 42, source: 'PORT_OFFSET=42' })

		const named = resolveStartOffset({
			devInstance: 'design-review',
			primaryPath: '/repo',
			worktreePath: '/repo',
		})
		expect(named.offset).toBeGreaterThan(0)
		expect(named.source).toBe('hashed DEV_INSTANCE=design-review')
	})

	it('rejects invalid explicit offsets', () => {
		expect(() =>
			resolveStartOffset({
				explicitOffset: '-1',
				primaryPath: '/repo',
				worktreePath: '/repo',
			}),
		).toThrow('PORT_OFFSET must be a non-negative integer')
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
			backend: 8788,
			accountWeb: 3101,
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
				HOST: 'example.com',
				PORT: '9999',
				SHARED: 'kept',
			},
			ports: portsForOffset(3),
		})

		expect(env.HOST).toBeUndefined()
		expect(env.PORT).toBeUndefined()
		expect(env.SHARED).toBe('kept')
		expect(env.ACCOUNT_API_URL).toBe('http://127.0.0.1:8790')
	})
})

describe('dev runner modes', () => {
	it('runs the complete product in dev mode', () => {
		expect(processLabelsForMode('dev')).toEqual(['backend', 'account-web', 'core', 'app-frontend'])
	})

	it('keeps partial modes focused', () => {
		expect(processLabelsForMode('dev:app')).toEqual(['app-frontend'])
		expect(processLabelsForMode('dev:backend')).toEqual(['backend', 'account-web'])
		expect(processLabelsForMode('dev:core')).toEqual(['core'])
	})

	it('leaves out the account sign-in website when asked', async () => {
		expect(processLabelsForMode('dev', false)).toEqual(['backend', 'core', 'app-frontend'])
		const checked: number[] = []
		await findFirstAvailableOffset({
			accountWeb: false,
			checkPort: async (port) => {
				checked.push(port)
				return true
			},
			mode: 'dev',
			startOffset: 0,
		})
		expect(checked).toEqual([1420, 8787, 16662])
	})
})

describe('dev runner scenarios', () => {
	it('uses dev.json defaults when no scenario is passed', () => {
		expect(parseInput(['dev'], [2, 4])).toEqual({
			accountWeb: true,
			dryRun: false,
			hideWindows: false,
			mode: 'dev',
			scenarios: [2, 4],
		})
	})

	it('reads the options a background start passes', () => {
		expect(parseInput(['dev', '2', '--no-account-web', '--hide-windows'], [1])).toEqual({
			accountWeb: false,
			dryRun: false,
			hideWindows: true,
			mode: 'dev',
			scenarios: [2],
		})
		expect(() => parseInput(['dev', '--nope'], [1])).toThrow('Unknown option --nope')
	})

	it('accepts several scenarios and removes duplicates', () => {
		expect(parseInput(['dev', '1', '2', '1'], [4]).scenarios).toEqual([1, 2])
		expect(parseInput(['dev:app', '3'], [1]).scenarios).toEqual([3])
	})

	it('does not attach scenarios to backend-only modes', () => {
		expect(parseInput(['dev:core'], [1])).toEqual({
			accountWeb: true,
			dryRun: false,
			hideWindows: false,
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
			coreData: `${process.cwd()}/.data/core`,
			data: `${process.cwd()}/.data`,
			primary: process.cwd(),
			runtime: `${process.cwd()}/.data/runtime.json`,
			scenariosData: `${process.cwd()}/.data/scenarios`,
			worktree: process.cwd(),
		}
		const specs = createProcessSpecs({
			accountWeb: true,
			branch: 'feature',
			env: {},
			hideWindows: true,
			mode: 'dev',
			paths,
			ports: portsForOffset(0),
			scenarios: [1, 3],
		})

		expect(specs.map((spec) => spec.label)).toEqual([
			'backend',
			'account-web',
			'core',
			'app-frontend',
			'app:1',
			'app:3',
		])
		const apps = specs.filter((spec) => spec.label.startsWith('app:'))
		for (const app of apps) {
			// Ports reach the App when it starts, so one build serves every checkout.
			expect(JSON.parse(app.args.at(-1)!)).toMatchObject({
				backendUrl: 'http://127.0.0.1:8787',
				devUrl: 'http://localhost:1420',
				hideWindow: true,
			})
			expect(JSON.stringify(app.build)).not.toContain('1420')
			expect(app.build?.inputs).toEqual(
				expect.arrayContaining(['Cargo.lock', 'apps/app', 'packages/browser-bridge']),
			)
		}
		expect(apps[0]!.build).toEqual(apps[1]!.build)
		// Core compiles this crate from outside its own folder.
		expect(specs.find((spec) => spec.label === 'core')?.build?.inputs).toEqual(
			expect.arrayContaining(['apps/core', 'Cargo.lock', 'packages/minecraft-meta-manifest']),
		)
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

describe('dev runner program cache', () => {
	const build = {
		cargoArgs: ['build'],
		cwd: process.cwd(),
		env: {},
		ignoredInputs: [],
		inputs: ['scripts/dev-runner.ts'],
		program: 'program',
		targetDir: `${process.cwd()}/target`,
	}

	it('keys a program by its inputs, not by where it is built', () => {
		const key = programKey(process.cwd(), build)
		expect(programKey(process.cwd(), { ...build, targetDir: '/elsewhere' })).toBe(key)
		expect(
			programKey(process.cwd(), { ...build, inputs: ['scripts/dev-runner.test.ts'] }),
		).not.toBe(key)
		expect(programKey(process.cwd(), { ...build, env: { TAURI_CONFIG: '{}' } })).not.toBe(key)
		expect(programKey(process.cwd(), { ...build, cargoArgs: ['build', '--release'] })).not.toBe(key)
	})
})

describe('dev runner readiness', () => {
	it('reads the browser bridge URL from the line the App prints', () => {
		expect(
			bridgeUrlFromLine(
				'2026-10-11T15:33:26.147268Z  INFO browser_bridge: Amberite browser bridge ready url=http://127.0.0.1:64463',
			),
		).toBe('http://127.0.0.1:64463')
		expect(bridgeUrlFromLine('Browser bridge Vite WebSocket proxy failed')).toBeUndefined()
	})
})
