// `stack`: a fixture's first line. It replaces the dev stack's data and starts exactly the Apps
// and Cores it names, each made ready the way a person would: the App signs in, the Core is paired
// with the code it shows.

import * as NodeChildProcess from 'node:child_process'
import * as NodeCrypto from 'node:crypto'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { core as coreApi, cores as coresApi, localAccounts } from '@amberite/contracts'

import { answers, appUrl, backendSession, call, send } from './call.ts'
import {
	type Account,
	type App,
	type Core,
	current,
	FixtureError,
	runtime,
	state,
	step,
	until,
} from './context.ts'
import { fillGameFiles } from './downloads.ts'
import { useKeptJava } from './java.ts'

/** A first Rust build of the App and the Core fits in this. */
const START_MS = 9 * 60_000
/** What an apply leaves in a program's folder: its game files and the App window's browser profile. */
const KEPT = new Set(['meta', 'webview2'])

type Stack<A extends string, U extends string> = { readonly [Name in A]: App } & {
	readonly [Name in U]: Account
} & { readonly cores: ReadonlyArray<Core> }

/**
 * Wipes the dev stack's data and starts this stack on it.
 *
 * - `apps`: one App each, signed in to the account of that name.
 * - `users`: accounts with no App.
 * - `cores`: one Core each, paired to `owner`, or left showing its pairing code when `owner` is
 *   `null`.
 *
 * Returns the Apps and accounts by name, and the Cores in order.
 */
export async function stack<const A extends string = never, const U extends string = never>(input: {
	readonly apps?: ReadonlyArray<A>
	readonly users?: ReadonlyArray<U>
	readonly cores?: ReadonlyArray<{ readonly owner: NoInfer<A | U> | null }>
}): Promise<Stack<A, U>> {
	const runner = current()
	if (state.stack) throw new FixtureError('A fixture declares its stack once, on its first line.')
	const apps: ReadonlyArray<string> = input.apps ?? []
	const users: ReadonlyArray<string> = input.users ?? []
	const cores = input.cores ?? []
	const names = [...apps, ...users]
	for (const name of names) {
		if (!/^[a-z][a-z0-9_]{1,30}$/.test(name) || name === 'cores') {
			throw new FixtureError(
				`"${name}" cannot name an account: use lowercase letters, digits and _.`,
			)
		}
	}
	if (new Set(names).size !== names.length) throw new FixtureError('Two accounts share a name.')

	const appDir = (index: number) => NodePath.join(runner.data, 'apps', String(index))
	const coreDir = (index: number) => NodePath.join(runner.data, 'cores', String(index))
	await step('stop the stack and clear its data', async () => {
		runner.send('halt')
		await until(
			'the backend, Cores and Apps have stopped',
			() =>
				Object.entries(runtime().programs).every(
					([label, program]) => !holdsData(label) || program === 'stopped' || program === 'failed',
				),
			{ timeoutMs: 60_000 },
		)
		clearData(runner.data)
		NodeFS.cpSync(
			emptyBackend(runner.worktree, runner.primary),
			NodePath.join(runner.data, 'backend', 'v3'),
			{ recursive: true },
		)
		apps.forEach((_, index) => fillGameFiles(runner.primary, 'app', appDir(index + 1)))
		cores.forEach((_, index) => fillGameFiles(runner.primary, 'core', coreDir(index + 1)))
	})

	const id = NodeCrypto.randomUUID()
	NodeFS.writeFileSync(
		NodePath.join(runner.data, 'stack.json'),
		`${JSON.stringify({ apps: apps.map((name) => ({ name })), cores: cores.length, id }, null, '\t')}\n`,
	)
	await step('start the backend', async () => {
		runner.send('stack')
		await until('the dev runner starts the stack', () => runtime().stackId === id, {
			timeoutMs: 60_000,
		})
		await until('the backend answers', () => answers('backend'), { timeoutMs: START_MS })
	})

	// An App creates its own account as it signs in, so those accounts are read only afterwards.
	if (apps.length > 0) {
		await step(`start and sign in ${apps.join(', ')}`, () =>
			Promise.all(apps.map((name, index) => appSignedIn(name, index + 1))),
		)
	}
	const accounts = new Map<string, Account>()
	for (const name of names) {
		accounts.set(name, { kind: 'account', name, id: (await backendSession(name)).userId })
	}
	const made: Record<string, App | Account | ReadonlyArray<Core>> = {}
	const madeApps = apps.map((name, index): App => {
		return {
			kind: 'app',
			name,
			index: index + 1,
			dir: appDir(index + 1),
			account: accounts.get(name)!,
		}
	})
	for (const app of madeApps) made[app.name] = app
	for (const name of users) made[name] = accounts.get(name)!
	const madeCores =
		cores.length === 0
			? []
			: await step(
					`start and pair ${cores.length === 1 ? 'the Core' : `${cores.length} Cores`}`,
					() =>
						Promise.all(
							cores.map(({ owner }, index) =>
								pair(index + 1, coreDir(index + 1), owner === null ? null : accounts.get(owner)!),
							),
						),
				)
	made.cores = madeCores
	if (madeApps.length > 0) {
		await step('point each App at the Java it kept', () => Promise.all(madeApps.map(useKeptJava)))
	}
	state.stack = { apps: madeApps, cores: madeCores }
	return made as Stack<A, U>
}

/** Whether a program keeps the data a fixture replaces. */
function holdsData(label: string): boolean {
	return label === 'backend' || label.startsWith('core:') || label.startsWith('app:')
}

/** Empties the backend's storage and every App's and Core's folder, down to what is kept. */
function clearData(data: string): void {
	// Data from before fixtures (`scenarios/`, `core/`) pairs with this backend database: keep it.
	const legacy = NodePath.join(data, 'backend', 'v3-before-fixtures')
	const storage = NodePath.join(data, 'backend', 'v3')
	const firstApply = !NodeFS.existsSync(NodePath.join(data, 'stack.json'))
	if (
		firstApply &&
		NodeFS.existsSync(NodePath.join(data, 'scenarios')) &&
		NodeFS.existsSync(storage) &&
		!NodeFS.existsSync(legacy)
	) {
		NodeFS.renameSync(storage, legacy)
	}
	// A program that just stopped can hold its files for a moment longer.
	const remove = (path: string) =>
		NodeFS.rmSync(path, { recursive: true, force: true, maxRetries: 40, retryDelay: 150 })
	remove(storage)
	for (const kind of ['apps', 'cores']) {
		const root = NodePath.join(data, kind)
		if (!NodeFS.existsSync(root)) continue
		for (const program of NodeFS.readdirSync(root)) {
			for (const entry of NodeFS.readdirSync(NodePath.join(root, program))) {
				if (!KEPT.has(entry)) remove(NodePath.join(root, program, entry))
			}
		}
	}
}

/**
 * The backend's storage with every migration applied and no rows, made by its own migration tool
 * once per schema and shared by every checkout.
 */
function emptyBackend(worktree: string, primary: string): string {
	const backend = NodePath.join(worktree, 'apps', 'backend')
	const migrations = NodePath.join(backend, 'migrations')
	const schema = NodeCrypto.createHash('sha256')
	for (const file of NodeFS.readdirSync(migrations).sort()) {
		schema.update(file).update(NodeFS.readFileSync(NodePath.join(migrations, file)))
	}
	const made = NodePath.join(
		primary,
		'target',
		'dev-shared',
		'fixtures',
		`backend-${schema.digest('hex').slice(0, 16)}`,
	)
	if (!NodeFS.existsSync(made)) {
		// Built beside its place and moved in, so a checkout applying at the same time never copies half.
		const partial = `${made}.${process.pid}`
		NodeFS.rmSync(partial, { recursive: true, force: true })
		const result = NodeChildProcess.spawnSync(
			process.execPath,
			[
				NodePath.join(worktree, 'node_modules', 'vite-plus', 'bin', 'vp'),
				...[
					'exec',
					'wrangler',
					'd1',
					'migrations',
					'apply',
					'accounts',
					'--local',
					'--persist-to',
					partial,
				],
			],
			{ cwd: backend, encoding: 'utf8', windowsHide: true },
		)
		if (result.status !== 0) {
			throw new FixtureError(
				`The backend's migrations did not apply:\n${result.stdout}${result.stderr}`,
			)
		}
		try {
			NodeFS.renameSync(partial, made)
		} catch (error) {
			if (!NodeFS.existsSync(made)) throw error
			NodeFS.rmSync(partial, { recursive: true, force: true })
		}
	}
	return NodePath.join(made, 'v3')
}

/**
 * Waits for a program the runner is starting. A Rust program is built first when its source
 * changed, which can outlast one command: the build goes on, and applying again waits for it.
 */
async function started<T>(
	label: string,
	what: string,
	check: () => Promise<T | undefined | false> | T | undefined | false,
): Promise<T> {
	try {
		return await until(what, check, { timeoutMs: START_MS })
	} catch (error) {
		if (runtime().programs[label] !== 'starting') throw error
		throw new FixtureError(
			`${label} is still being built. Run the same command again to keep waiting. Its progress is in ${NodePath.join(current().data, 'dev.log')}`,
		)
	}
}

async function appSignedIn(name: string, index: number): Promise<void> {
	const app = { index, name }
	await started(`app:${index}`, `${name}'s App starts`, () => answers(`app:${index}`))
	const active = localAccounts.getActiveModrinthAccount
	try {
		await until(
			`${name}'s App signs in`,
			async () => {
				const answer = await send(`${appUrl(app)}${active.path}`, {
					method: active.method,
					headers: {},
				})
				return answer.status < 400 && answer.text !== 'null' && answer.text !== ''
			},
			{ timeoutMs: 30_000 },
		)
	} catch {
		throw new FixtureError(
			`${name}'s App started but did not sign in. Its reason is in ${NodePath.join(current().data, 'dev.log')}`,
		)
	}
}

/** One value from a Core's own database, or `undefined` while the Core has not made it yet. */
function coreValue(dir: string, statement: string): string | undefined {
	const file = NodePath.join(dir, 'db', 'core.db')
	if (!NodeFS.existsSync(file)) return undefined
	try {
		const db = new DatabaseSync(file, { readOnly: true })
		try {
			const value = Object.values(db.prepare(statement).get() ?? {})[0]
			return value === null || value === undefined ? undefined : String(value)
		} finally {
			db.close()
		}
	} catch {
		// The Core is still creating its tables.
		return undefined
	}
}

/**
 * Waits for a new Core to show its pairing code and, for an owned Core, approves that code as the
 * owner and accepts the Minecraft EULA on it. This is the flow a person goes through in the App.
 */
async function pair(index: number, dir: string, owner: Account | null): Promise<Core> {
	const code = await started(`core:${index}`, `Core ${index} shows its pairing code`, () =>
		coreValue(dir, 'SELECT code FROM pairing WHERE id = 1'),
	)
	const id = await until(`Core ${index} has its id`, () =>
		coreValue(dir, 'SELECT core_id FROM core WHERE id = 1'),
	)
	const core: Core = { kind: 'core', index, dir, id, owner }
	if (!owner) return core
	await call(owner, coresApi.ApproveSignInCode, null, { code })
	await until(
		`Core ${index} learns it belongs to ${owner.name}`,
		() => coreValue(dir, 'SELECT owner_user_id FROM core WHERE id = 1'),
		{ timeoutMs: 60_000 },
	)
	await call(core, coreApi.AcceptEula)
	return core
}
