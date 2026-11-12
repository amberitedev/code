import { accounts, social } from '@amberite/contracts'

import type { ContractTransport } from '../contracts/client'
import { AbstractFeature, type FeatureConfig } from '../core/abstract-feature'
import { SharedInstancesApiError } from '../sharing/errors'
import type { RequestContext } from '../types/request'

export interface SelfHostedConfig extends FeatureConfig {
	baseUrl: string
	token: () => Promise<string | undefined>
}

/**
 * Routes private accounts and sharing without sending their sessions to public content services.
 * Every other request, including those to a Core, goes without an account token: a Core gets
 * this device's own session for it from the platform instead (see `archon.cores_v1.signIn`).
 */
export class SelfHostedFeature extends AbstractFeature {
	declare protected config: SelfHostedConfig

	constructor(config: SelfHostedConfig) {
		super(config)
	}

	/** A refused sharing request throws `SharedInstancesApiError`, with its status and code. */
	async execute<T>(next: () => Promise<T>, context: RequestContext): Promise<T> {
		await routeSelfHostedRequest(context, this.config)
		if (context.options.api !== 'sharedinstances') return next()
		try {
			return await next()
		} catch (error) {
			throw SharedInstancesApiError.from(error)
		}
	}

	/** `client.backend`'s way to the backend: the same URL and account session. */
	transport(fetch: typeof globalThis.fetch): ContractTransport {
		return {
			base: this.config.baseUrl,
			headers: async (endpoint) => {
				const token = endpoint.auth === 'none' ? undefined : await this.config.token()
				const headers: Record<string, string> = {}
				if (token) headers.Authorization = `Bearer ${token}`
				return headers
			},
			fetch,
		}
	}
}

/** Shared by the typed client and the website's legacy fetch adapter. */
export async function routeSelfHostedRequest(
	context: Pick<RequestContext, 'url' | 'path' | 'options'>,
	config: SelfHostedConfig,
): Promise<void> {
	if (
		context.options.api === 'labrinth' &&
		context.path.split('?')[0] === '/image' &&
		(context.options.params?.context ?? new URL(context.url).searchParams.get('context')) ===
			'report'
	) {
		throw new Error('Report attachments are not supported by this account service yet.')
	}
	const privateRequest =
		context.options.api === 'sharedinstances' ||
		(context.options.api === 'labrinth' &&
			context.options.accountSource !== 'modrinth' &&
			isAccountPath(context.path))
	const skipAuth = context.options.skipAuth
	context.options.skipAuth = true
	const headers = new Headers(context.options.headers)
	if (privateRequest) {
		const source = new URL(context.url)
		context.url = new URL(source.pathname + source.search, config.baseUrl).toString()
		if (!headers.has('authorization') && !skipAuth) {
			const token = await config.token()
			if (token) headers.set('authorization', `Bearer ${token}`)
		}
	} else {
		headers.delete('authorization')
	}
	const normalizedHeaders: Record<string, string> = {}
	headers.forEach((value, key) => {
		normalizedHeaders[key] = value
	})
	context.options.headers = normalizedHeaders
}

/** The segments of every path the backend serves for upstream's labrinth modules. */
const accountPaths = [...Object.values(accounts), ...Object.values(social)].map(({ path }) =>
	path.split('/'),
)

function isAccountPath(path: string): boolean {
	const segments = (path.split('?')[0] ?? path).split('/')
	// The backend refuses every other sign-in method itself, so none reaches Modrinth.
	if (segments[1] === 'auth') return true
	return accountPaths.some(
		(template) =>
			template.length === segments.length &&
			template.every((part, index) =>
				part.startsWith('{') ? segments[index] !== '' : part === segments[index],
			),
	)
}
