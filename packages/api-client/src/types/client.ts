import type { CoreRoute } from '@amberite/contracts/app'
import type { ApiError } from '@amberite/contracts/protocol'
import type { CoreSession } from '@amberite/contracts/core'

import type { ContractTransport } from '../contracts/client'
import type { AbstractFeature } from '../core/abstract-feature'
import type { RequestContext } from './request'
import type { LocalFileUploader } from './upload'

type MaybePromise<T> = T | Promise<T>
type UserAgentProvider = string | (() => MaybePromise<string | undefined>)
export type BaseUrlConfig = string | (() => string)

/**
 * Request lifecycle hooks
 */
export type RequestHooks = {
	/**
	 * Called before request is sent (after all features have processed)
	 */
	onRequest?: (context: RequestContext) => void | Promise<void>

	/**
	 * Called after successful response (before features process response)
	 */
	onResponse?: <T>(data: T, context: RequestContext) => void | Promise<void>

	/**
	 * Called when request fails (after all features have processed error)
	 */
	onError?: (error: Error, context: RequestContext) => void | Promise<void>
}

/**
 * Why the platform could not sign in to a Core: the error Core answered with, or `status` null
 * when it did not answer.
 */
export type CoreCallError = ApiError & { status: number | null }

/** See `ClientConfig.coreSessions`. Each call rejects with a `CoreCallError` or an `ApiCallError`. */
export interface CoreSessions {
	/** Trade `ticket` for a session on the connected Core `coreId`, replacing any held. */
	signIn(coreId: string, fingerprint: string, ticket: string): Promise<CoreSession>
	/** End this device's session on the Core, when it answers, and forget it either way. */
	signOut(coreId: string): Promise<void>
	/** Whether this device holds a session for the Core. It may have ended there since. */
	exists(coreId: string): Promise<boolean>
}

/**
 * Client configuration
 */
export interface ClientConfig {
	/**
	 * Connects to the Core `coreId`, trusting only the program that holds the certificate with this
	 * SHA-256 fingerprint: at `address` when that is on this computer's network, or at `introduced`,
	 * where the backend's introduction says the Core is. Answers with the base URL requests to that
	 * Core go to, which carry this device's session for it (see `coreSessions`), how the Core is
	 * reached, and when it is not and nothing was `introduced`, what to ask the backend to
	 * introduce. Without it, a Core is reached at its address directly.
	 */
	connectCore?: (
		coreId: string,
		address: string,
		fingerprint: string,
		introduced?: string[],
	) => Promise<CoreRoute>

	/**
	 * This device's sessions on Cores, held by the platform outside the page: the page never sees
	 * a session token, and `connectCore` adds it to each request. See `cores.signIn`.
	 */
	coreSessions?: CoreSessions

	/** How `client.local` reaches the App's local server. */
	localTransport?: ContractTransport

	/** How `client.core(id)` reaches the Core `coreId`, in place of today's Core routing. */
	coreTransport?: (coreId: string) => ContractTransport

	/**
	 * User agent string or provider for requests
	 * Should identify your application (e.g., 'my-app/1.0.0')
	 * If not provided, the platform's default user agent will be used
	 */
	userAgent?: UserAgentProvider

	/**
	 * Base URL for Labrinth API (main Modrinth API)
	 * @default 'https://api.modrinth.com'
	 */
	labrinthBaseUrl?: BaseUrlConfig

	/**
	 * Default request timeout in milliseconds
	 * @default 10000
	 */
	timeout?: number

	/**
	 * Additional default headers to include in all requests
	 */
	headers?: Record<string, string>

	/**
	 * Sends uploads of `LocalFile`s, which the platform reads straight from disk.
	 * Without it, uploading a `LocalFile` fails.
	 */
	localFileUploader?: LocalFileUploader

	/**
	 * Features to enable for this client
	 * Features are applied in the order they appear in this array
	 */
	features?: AbstractFeature[]

	/**
	 * Request lifecycle hooks
	 */
	hooks?: RequestHooks
}
