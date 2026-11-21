// One typed endpoint, sent to whichever program the first argument names.

import * as NodeCrypto from 'node:crypto'
import * as NodeFS from 'node:fs'
import * as NodeHttp from 'node:http'
import * as NodeHttps from 'node:https'
import * as NodePath from 'node:path'
import { core as coreApi, cores as coresApi, dev, type Endpoint } from '@amberite/contracts'

import {
	type Account,
	type App,
	type Core,
	current,
	FixtureError,
	runtime,
	sleep,
	state,
} from './context.ts'

/** Who a call goes to: an App's own API, the backend as an account, or a Core as its owner. */
export type Who = App | Account | Core

/**
 * Calls `endpoint` and returns what it answers. An App is reached on its local API, the way its
 * own frontend reaches it; an account goes straight to the backend; a Core is called as its
 * owner. An error answer throws with the program's own message.
 */
export async function call<P, Q, B, R>(
	who: Who,
	endpoint: Endpoint<P, Q, B, R>,
	params?: NoInfer<P> | null,
	body?: NoInfer<B>,
	query?: NoInfer<Q> | null,
): Promise<R> {
	const route = endpoint.path.replace(/\{(\w+)\}/g, (_, key: string) => {
		const value = (params as Record<string, unknown> | null | undefined)?.[key]
		if (value === undefined || value === null) {
			throw new FixtureError(`${endpoint.method} ${endpoint.path} needs \`${key}\` in its params.`)
		}
		return encodeURIComponent(String(value))
	})
	const search = query
		? `?${new URLSearchParams(Object.entries(query).map(([key, value]): [string, string] => [key, String(value)]))}`
		: ''
	const target = await reach(who, endpoint)
	const answer = await send(`${target.base}${route}${search}`, {
		method: endpoint.method,
		headers: target.headers,
		body: endpoint.body === 'json' ? JSON.stringify(body ?? null) : undefined,
	})
	if (answer.status >= 400) {
		throw new FixtureError(
			`${endpoint.method} ${route} on ${target.name} answered ${answer.status}: ${describe(answer.text)}`,
		)
	}
	return (endpoint.response === 'json' && answer.text ? JSON.parse(answer.text) : undefined) as R
}

/** The program's `error` code and description, or its raw answer when it sent neither. */
function describe(text: string): string {
	try {
		const { error, description } = JSON.parse(text) as { error?: string; description?: string }
		if (error) return description ? `${error} (${description})` : error
	} catch {
		// Not JSON.
	}
	return text.slice(0, 400) || 'no body'
}

interface Target {
	readonly base: string
	readonly headers: Record<string, string>
	readonly name: string
}

async function reach(who: Who, endpoint: Endpoint): Promise<Target> {
	if (who.kind === 'app') {
		await restartIfPatched(`app:${who.index}`)
		return { base: appUrl(who), headers: {}, name: `${who.name}'s App` }
	}
	if (who.kind === 'account') {
		const { session } = await backendSession(who.name)
		return {
			base: runtime().urls.backend,
			headers: { authorization: session },
			name: `the backend as ${who.name}`,
		}
	}
	const account = who.owner
	await restartIfPatched(`core:${who.index}`)
	const base = coreUrl(who.index)
	const name = `Core ${who.index}${account ? ` as ${account.name}` : ''}`
	if (endpoint.auth === 'none') return { base, headers: {}, name }
	if (!account) throw new FixtureError(`Core ${who.index} has no owner, so it answers nobody.`)
	const headers: Record<string, string> = {
		authorization: `Bearer ${await coreToken(who, account)}`,
	}
	// A dangerous action proves the account again with a ticket made for this one call.
	if (endpoint.dangerous) headers['x-amberite-ticket'] = await ticket(who, account)
	return { base, headers, name }
}

/** The App's local API base, which its tab URL carries. */
export function appUrl(app: Pick<App, 'index' | 'name'>): string {
	const tabUrl = runtime().apps.find((entry) => entry.index === app.index)?.tabUrl
	const base = tabUrl && new URLSearchParams(new URL(tabUrl).hash.slice(1)).get('local')
	if (!base) throw new FixtureError(`${app.name}'s App is not part of the running stack.`)
	return base
}

export function coreUrl(index: number): string {
	const url = runtime().cores.find((entry) => entry.index === index)?.url
	if (!url) throw new FixtureError(`Core ${index} is not part of the running stack.`)
	return url
}

/** A backend session for the account of that name, which the backend creates on first use. */
export async function backendSession(
	name: string,
): Promise<{ readonly session: string; readonly userId: string }> {
	const known = state.backendSessions.get(name)
	if (known) return known
	const { session, user_id } = await devCall(dev.Session, { username: name })
	const made = { session, userId: user_id }
	state.backendSessions.set(name, made)
	return made
}

/**
 * Calls one of the backend's `/_dev` routes, which ask for the dev runner's secret instead of an
 * account.
 */
export async function devCall<B, R>(
	endpoint: Endpoint<null, null, B, R>,
	body: NoInfer<B>,
): Promise<R> {
	const secret = NodePath.join(current().data, 'backend', 'dev-secret')
	const answer = await send(`${runtime().urls.backend}${endpoint.path}`, {
		method: endpoint.method,
		headers: { 'x-local-dev-secret': NodeFS.readFileSync(secret, 'utf8').trim() },
		body: JSON.stringify(body),
	})
	if (answer.status >= 400) {
		throw new FixtureError(
			`${endpoint.method} ${endpoint.path} on the backend answered ${answer.status}: ${describe(answer.text)}`,
		)
	}
	return JSON.parse(answer.text) as R
}

async function ticket(core: Core, account: Account): Promise<string> {
	return (await call(account, coresApi.IssueTicket, { id: core.id })).ticket
}

async function coreToken(core: Core, account: Account): Promise<string> {
	const key = `${core.index}:${account.name}`
	const known = state.coreTokens.get(key)
	if (known) return known
	const signedInAs = await call(core, coreApi.SignIn, null, {
		ticket: await ticket(core, account),
		device_name: 'fixture',
	})
	state.coreTokens.set(key, signedInAs.token)
	return signedInAs.token
}

/**
 * Restarts a Core or App whose rows a fixture wrote while it ran, so it never answers from what it
 * read before. See `patch`.
 */
export function restartIfPatched(label: string): Promise<void> {
	// Calls made at the same time wait on the one restart rather than reaching the old process.
	const pending = restarts.get(label)
	if (pending) return pending
	if (!state.dirty.delete(label)) return Promise.resolve()
	const attempt = restart(label)
		.catch((error: unknown) => {
			state.dirty.add(label)
			throw error
		})
		.finally(() => restarts.delete(label))
	restarts.set(label, attempt)
	return attempt
}

const restarts = new Map<string, Promise<void>>()

/** Restarts one program through the dev runner and waits until it answers again. */
export async function restart(label: string): Promise<void> {
	// The id marks this request: the program's state alone could be left from before it, or come
	// from a restart the runner made on its own.
	const request = NodeCrypto.randomUUID()
	current().send(`rs ${label} #${request}`)
	const deadline = Date.now() + 9 * 60_000
	for (;;) {
		const now = runtime()
		const answer = now.requests[request]
		// Once answered, a failure is this start's: the new process can exit before it ever answers.
		if (answer && (answer !== 'running' || now.programs[label] === 'failed')) {
			throw new FixtureError(`${label} did not start again.`)
		}
		if (answer && (await answers(label))) return
		if (Date.now() > deadline) throw new FixtureError(`${label} did not come back.`)
		await sleep(100)
	}
}

/** Whether a program of the stack answers on its API. */
export async function answers(label: string): Promise<boolean> {
	const [kind, index] = label.split(':')
	const url =
		kind === 'backend'
			? `${runtime().urls.backend}/`
			: kind === 'core'
				? `${coreUrl(Number(index))}${coreApi.CheckHealth.path}`
				: `${appUrl({ index: Number(index), name: label })}/health`
	try {
		const answer = await send(url, { method: 'GET', headers: {} })
		return kind === 'backend' || answer.status < 400
	} catch {
		return false
	}
}

/** One request. A Core serves its own certificate, which only the App pins, so it is not checked. */
export function send(
	url: string,
	init: {
		readonly method: string
		readonly headers: Record<string, string>
		readonly body?: string
	},
): Promise<{ readonly status: number; readonly text: string }> {
	return new Promise((resolve, reject) => {
		const target = new URL(url)
		const headers =
			init.body === undefined
				? init.headers
				: { ...init.headers, 'content-type': 'application/json' }
		const options = { method: init.method, headers, timeout: 120_000 }
		const request =
			target.protocol === 'https:'
				? NodeHttps.request(target, { ...options, rejectUnauthorized: false })
				: NodeHttp.request(target, options)
		request.once('timeout', () => request.destroy(new Error(`${init.method} ${url} timed out`)))
		request.once('error', reject)
		request.once('response', (response) => {
			const chunks: Buffer[] = []
			response.on('data', (chunk: Buffer) => chunks.push(chunk))
			response.once('error', reject)
			response.once('end', () =>
				resolve({ status: response.statusCode ?? 0, text: Buffer.concat(chunks).toString('utf8') }),
			)
		})
		request.end(init.body)
	})
}
