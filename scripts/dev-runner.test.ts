import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'
import { describe, expect, it } from 'vite-plus/test'

import {
	appTabUrl,
	createCommandInput,
	createRuntimeEnvironment,
	createSharedSpecs,
	createStackSpecs,
	findFirstAvailableOffset,
	isBrowserAllowedPort,
	parseInput,
	portsForOffset,
	prepareCheckoutCrates,
	processLabelsForMode,
	programKey,
	readStackShape,
	resolveStartOffset,
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
	it('runs everything but the Cores and Apps of the stack in dev mode', () => {
		expect(processLabelsForMode('dev')).toEqual(['backend', 'account-web', 'app-frontend'])
	})

	it('keeps partial modes focused', () => {
		expect(processLabelsForMode('dev:app')).toEqual(['app-frontend'])
		expect(processLabelsForMode('dev:backend')).toEqual(['backend', 'account-web'])
		expect(processLabelsForMode('dev:core')).toEqual([])
	})

	it('leaves out the account sign-in website when asked', async () => {
		expect(processLabelsForMode('dev', false)).toEqual(['backend', 'app-frontend'])
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

describe('dev runner stack', () => {
	const paths = {
		appsData: `${process.cwd()}/.data/apps`,
		coresData: `${process.cwd()}/.data/cores`,
		data: `${process.cwd()}/.data`,
		primary: process.cwd(),
		runtime: `${process.cwd()}/.data/runtime.json`,
		stack: `${process.cwd()}/.data/stack.json`,
		worktree: process.cwd(),
	}

	it('reads the options a background start passes', () => {
		expect(parseInput(['dev'])).toEqual({
			accountWeb: true,
			dryRun: false,
			held: false,
			hideWindows: false,
			mode: 'dev',
		})
		expect(parseInput(['dev', '--no-account-web', '--hide-windows', '--held'])).toEqual({
			accountWeb: false,
			dryRun: false,
			held: true,
			hideWindows: true,
			mode: 'dev',
		})
		expect(() => parseInput(['dev', '--nope'])).toThrow('Unknown option --nope')
	})

	it('leaves the Apps and Cores to a fixture', () => {
		expect(() => parseInput(['dev', '2'])).toThrow('A fixture names the Apps and Cores')
	})

	it('reads the stack a fixture wrote, and only the part a mode runs', () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const file = NodePath.join(scratch, 'stack.json')
		try {
			expect(readStackShape(file, 'dev')).toBeUndefined()
			const shape = { apps: [{ name: 'alice' }, { name: 'bob' }], cores: 2, id: 'one' }
			NodeFS.writeFileSync(file, JSON.stringify(shape))
			expect(readStackShape(file, 'dev')).toEqual(shape)
			expect(readStackShape(file, 'dev:core')).toEqual({ ...shape, apps: [] })
			expect(readStackShape(file, 'dev:app')).toEqual({ ...shape, cores: 0 })
			expect(readStackShape(file, 'dev:backend')).toEqual({ apps: [], cores: 0, id: 'one' })
			NodeFS.writeFileSync(file, JSON.stringify({ apps: ['alice'], cores: 1, id: 'two' }))
			expect(() => readStackShape(file, 'dev')).toThrow('is not a stack file')
		} finally {
			NodeFS.rmSync(scratch, { force: true, recursive: true })
		}
	})

	it('gives each App and Core isolated persistent state', () => {
		const specs = createStackSpecs({
			appAddresses: [
				{ port: 50001, secret: 'one' },
				{ port: 50002, secret: 'two' },
			],
			branch: 'feature',
			corePorts: [16662, 50010],
			env: { MODRINTH_API_URL: 'https://api.example/v2/' },
			hideWindows: true,
			paths,
			ports: portsForOffset(0),
			shape: { apps: [{ name: 'alice' }, { name: 'bob' }], cores: 2, id: 'one' },
		})

		expect(specs.map((spec) => spec.label)).toEqual(['core:1', 'core:2', 'app:1', 'app:2'])
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
				expect.arrayContaining(['Cargo.lock', 'apps/app', 'packages/engine']),
			)
		}
		expect(apps[0]!.build).toEqual(apps[1]!.build)
		const cores = specs.filter((spec) => spec.label.startsWith('core:'))
		// Core compiles this crate from outside its own folder.
		expect(cores[0]!.build?.inputs).toEqual(
			expect.arrayContaining(['apps/core', 'Cargo.lock', 'packages/minecraft-meta-manifest']),
		)
		expect(cores[0]!.build?.env).toEqual({ MODRINTH_API_URL: 'https://api.example/v2/' })
		expect(cores[0]!.build?.ignoredInputs).toEqual(apps[0]!.build?.ignoredInputs)
		expect(cores[0]!.build).toEqual(cores[1]!.build)
		const cwd = process.cwd().replaceAll('\\', '/')
		// A folder is numbered by place in the stack, whatever the fixture calls its account.
		expect(apps.map((spec) => spec.env.THESEUS_CONFIG_DIR?.replaceAll('\\', '/'))).toEqual([
			`${cwd}/.data/apps/1`,
			`${cwd}/.data/apps/2`,
		])
		expect(cores.map((spec) => spec.env.CORE_DATA_DIR?.replaceAll('\\', '/'))).toEqual([
			`${cwd}/.data/cores/1`,
			`${cwd}/.data/cores/2`,
		])
		expect(cores.map((spec) => spec.env.PORT)).toEqual(['16662', '50010'])
		expect(apps.map((spec) => JSON.parse(spec.args.at(-1)!).username)).toEqual(['alice', 'bob'])
		// `rs` restarts an App from this spec, so it keeps its local address.
		expect(apps.map((spec) => JSON.parse(spec.args.at(-1)!))).toMatchObject([
			{ localPort: 50001, localSecret: 'one' },
			{ localPort: 50002, localSecret: 'two' },
		])
	})

	it('runs the programs a mode shares beside the stack', () => {
		const specs = createSharedSpecs({
			accountWeb: false,
			env: {},
			mode: 'dev',
			paths,
			ports: portsForOffset(0),
		})
		expect(specs.map((spec) => spec.label)).toEqual(['backend', 'app-frontend'])
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

	it('trusts shared output per build, and a crate tests only where they were compiled', () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const library = (checkout: string) => NodePath.join(checkout, 'packages', 'a', 'src', 'lib.rs')
		const checkout = (name: string) => {
			const dir = NodePath.join(scratch, name)
			for (const crate of ['app', 'a']) {
				NodeFS.mkdirSync(NodePath.join(dir, 'packages', crate, 'src'), { recursive: true })
				NodeFS.writeFileSync(NodePath.join(dir, 'packages', crate, 'Cargo.toml'), crate)
			}
			NodeFS.writeFileSync(NodePath.join(dir, 'packages', 'app', 'src', 'main.rs'), 'fn main() {}')
			NodeFS.writeFileSync(library(dir), 'pub fn a() {}')
			NodeFS.writeFileSync(NodePath.join(dir, 'Cargo.lock'), 'lock')
			NodeChildProcess.execFileSync('git', ['init', '-q'], { cwd: dir })
			return dir
		}
		const shared = {
			cacheDir: NodePath.join(scratch, 'target', 'dev-bin'),
			cargoArgs: [],
			cwd: scratch,
			env: {},
			ignoredInputs: [],
			targetDir: NodePath.join(scratch, 'target'),
		}
		const program = {
			...shared,
			inputs: ['Cargo.lock', 'packages/app', 'packages/a'],
			program: 'app',
		}
		const tests = {
			...shared,
			inputs: ['Cargo.lock', 'packages/a'],
			programInputs: {},
			tested: ['packages/a'],
		}
		const both = {
			...tests,
			inputs: program.inputs,
			programInputs: { 'packages/app': program.inputs },
			tested: ['packages/a', 'packages/app'],
		}
		/** How many crates the build compiles, recorded as if it succeeded. */
		const compiles = (worktree: string, build: typeof program | typeof tests) => {
			const prepared = prepareCheckoutCrates(worktree, build)
			prepared.recordBuilt()
			return prepared.changed
		}
		try {
			const here = checkout('here')
			const there = checkout('there')
			expect(compiles(here, program)).toBe(2)
			expect(compiles(here, tests)).toBe(1)
			// The tests compiled the content the program was compiled from.
			expect(compiles(here, program)).toBe(0)
			expect(NodeFS.statSync(library(here)).mtime.getUTCFullYear()).toBe(2000)
			expect(compiles(here, tests)).toBe(0)
			// Same content elsewhere: the program serves it, but the tests hold this checkout's path.
			expect(compiles(there, program)).toBe(0)
			expect(compiles(there, tests)).toBe(1)
			expect(NodeFS.statSync(library(there)).mtime.getUTCFullYear()).not.toBe(2000)
			expect(compiles(here, tests)).toBe(1)
			expect(compiles(here, program)).toBe(0)
			NodeFS.writeFileSync(library(there), 'pub fn b() {}')
			expect(compiles(there, tests)).toBe(1)
			expect(compiles(here, program)).toBe(1)
			expect(compiles(here, tests)).toBe(1)
			// A program's own crate hashes with the program's loose inputs in its tests too.
			expect(compiles(here, both)).toBe(2)
			expect(compiles(here, tests)).toBe(0)
			expect(compiles(here, program)).toBe(0)
			// Compiling a crate's tests elsewhere ends every test run's trust in them here.
			expect(compiles(there, both)).toBe(2)
			expect(compiles(here, tests)).toBe(1)
		} finally {
			NodeFS.rmSync(scratch, { force: true, recursive: true })
		}
	})
})

describe('dev runner tab URL', () => {
	it("names the tab after the App's account and carries its local API base", () => {
		expect(appTabUrl('alice', 1420, { port: 50001, secret: 'ab12' })).toBe(
			'http://alice.localhost:1420/#local=http%3A%2F%2F127.0.0.1%3A50001%2Fab12',
		)
		// A host name cannot hold an underscore.
		expect(new URL(appTabUrl('big_bob', 1420, { port: 1, secret: 'a' })).hostname).toBe(
			'big-bob.localhost',
		)
	})
})

describe('dev runner commands', () => {
	it('answers a request once that very request has finished, in the order sent', async () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const file = NodePath.join(scratch, 'dev-commands')
		const answered: string[] = []
		const calls: string[] = []
		let finishStop = () => {}
		let finishStart = () => {}
		const commands = createCommandInput({
			answered: (request, name) => answered.push(`${request} ${name}`),
			commandFile: file,
			getCore: () => undefined,
			halt: async () => {},
			restart: async (name) => {
				calls.push(`rs ${name}`)
				return { started: new Promise<void>((resolve) => (finishStart = resolve)) }
			},
			startStack: async () => {},
			stop: async () => {},
			stopProgram: (name) => {
				calls.push(`stop ${name}`)
				return new Promise<void>((resolve) => (finishStop = resolve))
			},
		})
		try {
			NodeFS.appendFileSync(file, 'stop backend #one\nrs core:1 #two\n')
			await expect.poll(() => calls, { timeout: 5_000 }).toEqual(['stop backend'])
			// The restart waits its turn, and nothing is answered while the stop still works.
			expect(answered).toEqual([])
			finishStop()
			await expect.poll(() => calls, { timeout: 5_000 }).toEqual(['stop backend', 'rs core:1'])
			// The old process is gone, but a restart is only done once the new one is up.
			expect(answered).toEqual(['one backend'])
			finishStart()
			await expect.poll(() => answered, { timeout: 5_000 }).toEqual(['one backend', 'two core:1'])
		} finally {
			commands.close()
			NodeFS.rmSync(scratch, { force: true, recursive: true })
		}
	})
})
