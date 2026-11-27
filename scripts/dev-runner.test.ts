import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeNet from 'node:net'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'
import { describe, expect, it } from 'vite-plus/test'

import {
	appLocalAddresses,
	appTabUrl,
	createCommandInput,
	createRuntimeEnvironment,
	createSharedSpecs,
	createStackSpecs,
	findFirstAvailableOffset,
	fixtureTurn,
	isBrowserAllowedPort,
	parseInput,
	portsForOffset,
	programKey,
	readStackShape,
	seedBuildOutput,
	resolveStartOffset,
	testBuild,
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
			startOffset: 0,
		})

		expect(offset).toBe(1)
		expect(checked).toContain(1420)
		expect(portsForOffset(offset)).toEqual({
			app: 1421,
			backend: 8788,
			core: 16663,
		})
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

describe('dev runner programs', () => {
	it('checks the ports of the frontend, the backend and the first Core', async () => {
		const checked: number[] = []
		await findFirstAvailableOffset({
			checkPort: async (port) => {
				checked.push(port)
				return true
			},
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

	it('tests several crates in one Cargo run', () => {
		const { cargoArgs } = testBuild(paths, {}, ['engine', 'contracts'])
		expect(cargoArgs.filter((arg) => arg === '--lib')).toHaveLength(1)
		expect(cargoArgs).toEqual(
			expect.arrayContaining(['--package', 'engine', '--package', 'contracts']),
		)
	})

	it("turns on only the tested crates' TypeScript, so the others stay as the programs compile them", () => {
		const { cargoArgs } = testBuild(paths, {}, ['engine', 'copal'])
		const features = cargoArgs.flatMap((arg, index) =>
			cargoArgs[index - 1] === '--features' ? [arg] : [],
		)
		expect(features).toEqual(['engine/export-ts'])
	})

	it('reads the fixture and options of dev', () => {
		expect(parseInput([])).toEqual({ fixture: undefined, held: false, hideWindows: false })
		expect(parseInput(['backend', '--hide-windows'])).toEqual({
			fixture: 'backend',
			held: false,
			hideWindows: true,
		})
		expect(parseInput(['--held', '--hide-windows'])).toMatchObject({ held: true })
		expect(() => parseInput(['--nope'])).toThrow('Unknown option --nope')
		expect(() => parseInput(['default', 'backend'])).toThrow('one fixture')
		expect(() => parseInput(['--held', 'default'])).toThrow('takes no fixture')
	})

	it('keeps each App at its local address across starts, by name', async () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const file = NodePath.join(scratch, 'local-api.json')
		const taken = NodeNet.createServer()
		try {
			const [alice, bob] = await appLocalAddresses(scratch, ['alice', 'bob'])
			expect(alice!.port).not.toBe(bob!.port)
			// Another stack, with the Apps in another order and one more, keeps each name's address.
			const [carol, bob2, alice2] = await appLocalAddresses(scratch, ['carol', 'bob', 'alice'])
			expect([bob2, alice2]).toEqual([bob, alice])
			expect([alice!.port, bob!.port]).not.toContain(carol!.port)
			// Unusable entries are replaced: a bad port, two Apps on one port, an empty secret.
			const kept = JSON.parse(NodeFS.readFileSync(file, 'utf8'))
			kept.alice = { port: 0, secret: alice!.secret }
			kept.bob = { port: carol!.port, secret: '' }
			NodeFS.writeFileSync(file, JSON.stringify(kept))
			const fixed = await appLocalAddresses(scratch, ['carol', 'bob', 'alice'])
			expect(fixed[0]).toEqual(carol)
			expect(fixed[1]!.port).not.toBe(carol!.port)
			expect(fixed[1]!.secret).toMatch(/^[0-9a-f]{64}$/)
			expect(fixed[2]!.port).toBeGreaterThan(0)
			expect(fixed[2]!.secret).toBe(alice!.secret)
			// A kept port something else holds is replaced; the secret stays.
			await new Promise<void>((resolve) => taken.listen(fixed[0]!.port, '127.0.0.1', resolve))
			const [moved] = await appLocalAddresses(scratch, ['carol'])
			expect(moved!.port).not.toBe(fixed[0]!.port)
			expect(moved!.secret).toBe(carol!.secret)
		} finally {
			taken.close()
			NodeFS.rmSync(scratch, { force: true, recursive: true })
		}
	})

	it('applies one fixture at a time', async () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const lock = NodePath.join(scratch, 'fixture.lock')
		try {
			const first = await fixtureTurn(scratch)
			const second = fixtureTurn(scratch)
			expect(NodeFS.readFileSync(lock, 'utf8')).toBe(String(process.pid))
			first()
			;(await second)()
			expect(NodeFS.existsSync(lock)).toBe(false)
			// A turn whose holder is gone is taken over.
			const gone = NodeChildProcess.spawnSync(process.execPath, ['-e', 'process.pid'])
			NodeFS.writeFileSync(lock, String(gone.pid))
			;(await fixtureTurn(scratch))()
		} finally {
			NodeFS.rmSync(scratch, { force: true, recursive: true })
		}
	})

	it('reads the stack a fixture wrote', () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const file = NodePath.join(scratch, 'stack.json')
		try {
			expect(readStackShape(file)).toBeUndefined()
			const shape = { apps: [{ name: 'alice' }, { name: 'bob' }], cores: 2, id: 'one' }
			NodeFS.writeFileSync(file, JSON.stringify(shape))
			expect(readStackShape(file)).toEqual(shape)
			NodeFS.writeFileSync(file, JSON.stringify({ apps: ['alice'], cores: 1, id: 'two' }))
			expect(() => readStackShape(file)).toThrow('is not a stack file')
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
			expect(JSON.stringify(app.program)).not.toContain('1420')
		}
		// One Cargo run builds both programs.
		const cores = specs.filter((spec) => spec.label.startsWith('core:'))
		for (const spec of [...apps, ...cores])
			expect(spec.program?.build).toBe(cores[0]!.program?.build)
		expect(apps[0]!.program?.file).toMatch(/^amberite-app/)
		expect(cores[0]!.program?.file).toMatch(/^copal/)
		// Core compiles this crate from outside its own folder.
		expect(cores[0]!.program?.build.inputs).toEqual(
			expect.arrayContaining([
				'apps/app',
				'apps/core',
				'Cargo.lock',
				'packages/engine',
				'packages/minecraft-meta-manifest',
			]),
		)
		expect(cores[0]!.program?.build.env).toMatchObject({
			MODRINTH_API_URL: 'https://api.example/v2/',
		})
		const cwd = process.cwd().replaceAll('\\', '/')
		// A folder is numbered by place in the stack, whatever the fixture calls its account.
		expect(apps.map((spec) => spec.env.AMBERITE_CONFIG_DIR?.replaceAll('\\', '/'))).toEqual([
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

	it('runs the backend and the frontend beside the stack', () => {
		const specs = createSharedSpecs({
			env: {},
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
		programs: ['program'],
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

	it("starts a build output from another checkout's third-party crates, never from a running one", async () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'dev-runner-'))
		const primary = NodePath.join(scratch, 'primary')
		const worktree = NodePath.join(scratch, 'worktree')
		// Ran Cargo last, but it failed before compiling anything.
		const broken = NodePath.join(scratch, 'broken')
		const git = (...args: ReadonlyArray<string>) =>
			NodeChildProcess.execFileSync('git', args, { cwd: primary })
		const output = (checkout: string) => NodePath.join(checkout, 'target', 'dev')
		const write = (file: string, content = '') => {
			NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
			NodeFS.writeFileSync(file, content)
		}
		try {
			NodeFS.mkdirSync(primary)
			git('init', '-q')
			git('-c', 'user.name=a', '-c', 'user.email=a@a', 'commit', '-q', '--allow-empty', '-m', 'a')
			git('worktree', 'add', '-q', worktree)
			git('worktree', 'add', '-q', broken)
			const source = NodePath.join(output(primary), 'debug')
			write(NodePath.join(source, 'deps', 'libtokio-1.rlib'), 'tokio')
			write(NodePath.join(source, '.fingerprint', 'tokio-0123456789abcdef', 'lib-tokio'))
			write(NodePath.join(source, '.fingerprint', 'engine-0123456789abcdef', 'lib-engine'))
			write(NodePath.join(source, '.fingerprint', 'tauri-0123456789abcdef', 'lib-tauri'))
			write(NodePath.join(source, 'build', 'tauri-0123456789abcdef', 'out', 'permission-files'))
			write(NodePath.join(output(primary), 'cargo-started'), 'run')
			const running = NodePath.join(output(primary), `cargo-running.${process.pid}.a`)
			write(running)
			const later = new Date(Date.now() + 60_000)
			write(NodePath.join(output(broken), 'cargo-started'), 'run')
			NodeFS.utimesSync(NodePath.join(output(broken), 'cargo-started'), later, later)

			await seedBuildOutput(worktree, output(worktree), new Set(['engine']), 'test')
			expect(NodeFS.existsSync(NodePath.join(output(worktree), 'debug'))).toBe(false)

			NodeFS.rmSync(running)
			await seedBuildOutput(worktree, output(worktree), new Set(['engine']), 'test')
			const seeded = NodePath.join(output(worktree), 'debug')
			expect(NodeFS.readFileSync(NodePath.join(seeded, 'deps', 'libtokio-1.rlib'), 'utf8')).toBe(
				'tokio',
			)
			// This repo's crates compile from the checkout itself, and tauri's build script runs again.
			expect(NodeFS.readdirSync(NodePath.join(seeded, '.fingerprint'))).toEqual([
				'tokio-0123456789abcdef',
			])
			expect(NodeFS.readdirSync(NodePath.join(seeded, 'build'))).toEqual([])
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
