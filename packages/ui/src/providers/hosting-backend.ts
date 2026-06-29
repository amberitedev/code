import {
	CoreApiClient,
	type CoreApiClient as CoreApiClientType,
	type CoreCreateInstanceBody,
	type CoreInstance,
	type CoreInstanceSummary,
	type PlatformAdapter,
} from '@amberite/amberite-api'

import { createContext } from './create-context'

export interface HostingUserSearchResult {
	id: string
	username: string
	avatarUrl?: string | null
}

export interface CoreHostingBackendOptions {
	searchUsers?: (query: string) => Promise<HostingUserSearchResult[]>
}

export interface HostingBackend {
	readonly core: CoreApiClientType
	listServers: () => Promise<CoreInstanceSummary[]>
	getServer: (id: string) => Promise<CoreInstance>
	createServer: (body: CoreCreateInstanceBody) => Promise<CoreInstance>
	searchUsers: (query: string) => Promise<HostingUserSearchResult[]>
}

export class CoreHostingBackend implements HostingBackend {
	constructor(
		readonly core: CoreApiClientType,
		private readonly options: CoreHostingBackendOptions = {},
	) {}

	listServers(): Promise<CoreInstanceSummary[]> {
		return this.core.listInstances()
	}

	getServer(id: string): Promise<CoreInstance> {
		return this.core.getInstance(id)
	}

	createServer(body: CoreCreateInstanceBody): Promise<CoreInstance> {
		return this.core.createInstance(body)
	}

	searchUsers(query: string): Promise<HostingUserSearchResult[]> {
		return this.options.searchUsers?.(query) ?? Promise.resolve([])
	}
}

export interface BrowserCoreHostingBackendOptions extends CoreHostingBackendOptions {
	coreUrl: string | (() => string | null | undefined)
	jwt?: string | (() => string | null | undefined | Promise<string | null | undefined>)
	connectedCoreId?: string | (() => string | null | undefined | Promise<string | null | undefined>)
	convexUrl?: string
}

function resolveRequiredCoreUrl(value: string | (() => string | null | undefined)): string {
	const coreUrl = typeof value === 'function' ? value() : value
	if (!coreUrl?.trim()) {
		throw new Error(
			'NUXT_PUBLIC_AMBERITE_CORE_URL or AMBERITE_CORE_URL must be set to use Amberite hosting.',
		)
	}
	return coreUrl
}

async function resolveOptionalString(
	value: string | (() => string | null | undefined | Promise<string | null | undefined>) | undefined,
): Promise<string | null> {
	const resolved = typeof value === 'function' ? await value() : value
	return resolved?.trim() ? resolved : null
}

export function createBrowserCoreHostingBackend(
	options: BrowserCoreHostingBackendOptions,
): CoreHostingBackend {
	const adapter: PlatformAdapter = {
		fetchFn: globalThis.fetch.bind(globalThis) as typeof fetch,
		convexUrl: options.convexUrl ?? '',
		getCoreUrl: async () => resolveRequiredCoreUrl(options.coreUrl),
		getCurrentJwt: async () => resolveOptionalString(options.jwt),
		getConnectedCoreId: async () => resolveOptionalString(options.connectedCoreId),
		openExternalAuth: (url) => {
			globalThis.location.href = url
		},
	}

	return new CoreHostingBackend(new CoreApiClient(adapter), options)
}

export const [injectHostingBackend, provideHostingBackend] = createContext<HostingBackend>(
	'HostingBackend',
	'hostingBackend',
)

export const injectCoreHostingBackend = injectHostingBackend
export const provideCoreHostingBackend = provideHostingBackend
