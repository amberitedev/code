// What a fixture's helpers share while it runs: the dev runner it talks to, and what `stack` made.

/** The part of the dev runner's `.data/runtime.json` a fixture reads. */
export interface Runtime {
	readonly programs: Readonly<Record<string, string>>
	/** Finished `stop` and `rs` requests by id, with the state each left its program in. */
	readonly requests: Readonly<Record<string, string>>
	readonly stackId?: string
	readonly apps: ReadonlyArray<{
		readonly dataDir: string
		readonly index: number
		readonly name: string
		readonly tabUrl?: string
	}>
	readonly cores: ReadonlyArray<{
		readonly dataDir: string
		readonly index: number
		readonly url: string
	}>
	readonly urls: { readonly backend: string }
}

/** How the fixture command lets a fixture reach the running dev stack. */
export interface Runner {
	readonly worktree: string
	readonly primary: string
	readonly data: string
	/** The runner's state, read again each time. Throws once the runner can no longer be used. */
	readonly runtime: () => Runtime | undefined
	/** One line for the runner, such as `rs core:1`. */
	readonly send: (command: string) => void
}

export interface Account {
	readonly kind: 'account'
	readonly name: string
	readonly id: string
}

export interface App {
	readonly kind: 'app'
	readonly name: string
	/** Its place in the stack, from 1. The runner calls it `app:<index>`. */
	readonly index: number
	/** Its data folder: databases, `profiles/` with one folder per instance, and the rest. */
	readonly dir: string
	readonly account: Account
}

export interface Core {
	readonly kind: 'core'
	/** Its place in the stack, from 1. The runner calls it `core:<index>`. */
	readonly index: number
	/** Its data folder: `db/`, `servers/` with one folder per server, and the rest. */
	readonly dir: string
	readonly id: string
	/** `null` while nobody has paired it. */
	readonly owner: Account | null
}

export class FixtureError extends Error {
	constructor(message: string) {
		super(message)
		this.name = 'FixtureError'
	}
}

let runner: Runner | undefined

export function begin(value: Runner): void {
	runner = value
	state.stack = undefined
	state.dirty.clear()
	state.backendSessions.clear()
	state.coreTokens.clear()
}

export function current(): Runner {
	if (!runner) {
		throw new FixtureError(
			'Fixture helpers only work in a file the fixture command runs: vp run fixture <file>',
		)
	}
	return runner
}

export const state = {
	stack: undefined as
		| { readonly apps: ReadonlyArray<App>; readonly cores: ReadonlyArray<Core> }
		| undefined,
	/** Programs whose rows were written while they ran, by runner label. */
	dirty: new Set<string>(),
	/** Backend sessions by account name. */
	backendSessions: new Map<string, { readonly session: string; readonly userId: string }>(),
	/** Core sessions by `<core index>:<account name>`. */
	coreTokens: new Map<string, string>(),
}

/** The runner's state, which has to exist while a fixture runs. */
export function runtime(): Runtime {
	// The runner rewrites its file as programs start and stop, so one read can land mid-write.
	for (let attempt = 0; attempt < 50; attempt += 1) {
		const value = current().runtime()
		if (value) return value
		Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10)
	}
	throw new FixtureError('The dev runner is not running.')
}

/** Runs one named part of a fixture and prints how long it took, so a slow apply shows where. */
export async function step<T>(name: string, work: () => Promise<T>): Promise<T> {
	const started = performance.now()
	const result = await work()
	console.log(`[fixture]   ${name} · ${((performance.now() - started) / 1000).toFixed(1)} s`)
	return result
}

export function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms))
}

/**
 * Asks `check` until it returns something, and returns that. For a helper to wait on work a
 * program finishes in the background, so the fixture's next line finds it done.
 */
export async function until<T>(
	what: string,
	check: () => Promise<T | undefined | null | false> | T | undefined | null | false,
	options: { readonly timeoutMs?: number; readonly everyMs?: number } = {},
): Promise<T> {
	const deadline = Date.now() + (options.timeoutMs ?? 120_000)
	for (;;) {
		const value = await check()
		if (value !== undefined && value !== null && value !== false) return value
		if (Date.now() > deadline) throw new FixtureError(`Gave up waiting until ${what}.`)
		await sleep(options.everyMs ?? 100)
	}
}
