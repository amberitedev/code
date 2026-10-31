import type { AbstractFeature } from '../core/abstract-feature'
import type { Copal } from '../modules/copal/types'
import type { RequestContext } from './request'
import type { LocalFileUploader } from './upload'

export type MaybePromise<T> = T | Promise<T>
export type UserAgentProvider = string | (() => MaybePromise<string | undefined>)
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
export type CoreCallError = { status: number | null; error: string; description: string }

/** See `ClientConfig.coreSessions`. Each call rejects with a `CoreCallError`. */
export interface CoreSessions {
	/** Trade `ticket` for a session on the Core `coreId` at `address`, replacing any held. */
	signIn(
		coreId: string,
		address: string,
		fingerprint: string,
		ticket: string,
	): Promise<Copal.Session>
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
	 * Hosting is served by the account's own Cores instead of Modrinth Hosting. Hosting requests
	 * go to those Cores (see `archon.cores_v1`), and there are no purchase or subscription flows.
	 */
	selfHostedHosting?: boolean

	/**
	 * Opens a connection to the Core `coreId` at `address`, which serves HTTPS with its own
	 * certificate, trusting only the certificate with this SHA-256 fingerprint. Returns the base URL
	 * requests to that Core go to; requests there carry this device's session for it (see
	 * `coreSessions`). Without it, a Core is reached at its address directly.
	 */
	connectCore?: (coreId: string, address: string, fingerprint: string) => Promise<string>

	/**
	 * This device's sessions on Cores, held by the platform outside the page: the page never sees
	 * a session token, and `connectCore` adds it to each request. See `copal.access_v1`.
	 */
	coreSessions?: CoreSessions

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
	 * Base URL for Archon API (Modrinth Hosting API)
	 * Can be a callback so apps can drive this from runtime feature flags.
	 *
	 * @default 'https://archon.modrinth.com'
	 */
	archonBaseUrl?: BaseUrlConfig

	/**
	 * Base URL for the Shared Instances API
	 * @default 'https://shared-instances.modrinth.com'
	 */
	sharedInstancesBaseUrl?: BaseUrlConfig

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
	 * Whether to attach `modrinth-sentry-capture: 1` to Archon requests.
	 * Can be a callback so apps can drive this from runtime feature flags.
	 *
	 * @default false
	 */
	archonSentryCapture?: boolean | (() => boolean)

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
