export namespace Amberite {
	export namespace Auth {
		export namespace v1 {
			export type SignInRequest = {
				mode: 'continue' | 'use_another_account'
				expectedMinecraftUuid?: string
			}
			export type Session = {
				accessToken: string
				expiresAt?: string
				user?: unknown
			}
		}
	}

	export namespace Accounts {
		export namespace v1 {
			export type LinkedModrinthAccount = {
				id: string
				userId: string
				modrinthUserId: string
				username: string
				avatar_url: string | null
				scopes: string[]
				expiresAt: number | null
				status: 'active' | 'needs_reconnect' | 'revoked'
				needsReconnect: boolean
				reconnectReason?: string | null
				linkedAt: number
				updatedAt: number
			}
			export type StoreModrinthTokensRequest = {
				accessToken: string
				refreshToken?: string
				scopes: string[]
				expiresAt?: number
			}
		}
	}

	export namespace Friends {
		export namespace v1 {
			export type AddByCodeRequest = { code: string }
		}
	}

	export namespace Sessions {
		export namespace v1 {
			export type DeviceMetadata = {
				os?: string
				platform?: string
				userAgent: string
				city?: string
				country?: string
				ip?: string
			}
		}
	}

	export namespace Cores {
		export namespace v1 {
			export type PairingMetadata = { bindHost?: string; port?: number }
			export type CoreListEntry = {
				coreId: string
				ownerUserId: string
				linkState: 'unlinked' | 'linked'
				connectionUrl?: string
				setupMode?: 'remote' | 'local'
				createdAt: number
				lastSeenAt: number
				projectionRevision: number
				syncedAt: number
				isOwner: boolean
				memberUserIds: string[]
			}
			export type PairingClaim = {
				coreId: string
				connectionUrl?: string
				metadata?: PairingMetadata
				syncCredential: string
				/** @deprecated Compatibility alias for Core builds created during the migration. */
				realtimeCredential: string
			}
		}
	}
}
