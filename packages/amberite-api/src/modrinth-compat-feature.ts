import { AmberiteFeature } from '@modrinth/api-client'
import type { PlatformAdapter } from './adapter'
import type { ConvexApiClient } from './convex-api'

type CompatTransport = Pick<ConvexApiClient, 'rawAction' | 'rawMutation' | 'rawQuery'>

export interface AmberiteModrinthCompatFeatureOptions {
	adapter: PlatformAdapter
	client: CompatTransport
	/** Convex HTTP Actions origin, for example https://example.convex.site. */
	sharedClientsSiteUrl?: string
	social?: boolean
	sharedClients?: boolean
}

/** @deprecated Configure `AmberiteFeature` from `@modrinth/api-client` directly. */
export class AmberiteModrinthCompatFeature extends AmberiteFeature {
	constructor(options: AmberiteModrinthCompatFeatureOptions) {
		super({
			transport: {
				query: (path, args = {}) => options.client.rawQuery(path, args),
				mutation: (path, args = {}) => options.client.rawMutation(path, args),
				action: (path, args = {}) => options.client.rawAction(path, args),
			},
			getAccessToken: options.adapter.getCurrentJwt.bind(options.adapter),
			fetch: options.adapter.fetchFn,
			siteUrl: options.sharedClientsSiteUrl ?? options.adapter.convexSiteUrl,
			refreshSession: options.adapter.refreshAmberiteSession?.bind(options.adapter),
			nativeAuth:
				options.adapter.signInWithMinecraft &&
				options.adapter.restoreAmberiteSession &&
				options.adapter.signOutAmberiteSession
					? {
							signIn: options.adapter.signInWithMinecraft.bind(options.adapter),
							restore: options.adapter.restoreAmberiteSession.bind(options.adapter),
							signOut: options.adapter.signOutAmberiteSession.bind(options.adapter),
						}
					: undefined,
			social: options.social,
			sharedInstances: options.sharedClients,
		})
	}
}
