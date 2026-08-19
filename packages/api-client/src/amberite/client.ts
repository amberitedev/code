import type { Amberite } from '../modules/amberite/types'
import type { AmberiteProfilePatch } from './profile'
import type { AmberitePlatformAdapter } from './platform'
import { ConvexAmberiteTransport, type AmberiteTransport } from './transport'

export interface AmberiteProfile {
	id: string
	userId: string
	username: string
	display_name: string
	displayName: string
	name: string
	minecraftUuid: string
	verifiedMinecraftHandle: string
	avatar_url: string | null
	bio: string | null
	created: string
	friendCode?: string
	allow_friend_requests: boolean
	[key: string]: unknown
}

/** Direct Convex operations that do not map to a Labrinth-compatible route. */
export class AmberiteApiClient {
	public readonly transport: AmberiteTransport

	constructor(source: AmberiteTransport | AmberitePlatformAdapter) {
		this.transport = 'query' in source ? source : new ConvexAmberiteTransport(source)
	}

	currentProfile(): Promise<AmberiteProfile> {
		return this.transport.query('profiles:current', {})
	}

	updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteProfile> {
		return this.transport.mutation('profiles:updateCurrent', patch)
	}

	linkedModrinthAccount(): Promise<Amberite.Accounts.v1.LinkedModrinthAccount | null> {
		return this.transport.query('modrinth:current', {})
	}

	storeModrinthOAuthTokens(
		request: Amberite.Accounts.v1.StoreModrinthTokensRequest,
	): Promise<Amberite.Accounts.v1.LinkedModrinthAccount> {
		return this.transport.action('modrinth:storeCurrentOAuthTokens', request)
	}

	disconnectModrinthAccount(): Promise<null> {
		return this.transport.mutation('modrinth:disconnectCurrent', {})
	}
}
