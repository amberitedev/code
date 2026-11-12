import type { IconConfig } from '../../types/icons'

export namespace SharedInstances {
	export namespace Invites {
		export namespace v1 {
			export type Inviter = {
				id: string
				name: string
				avatar: string | null
			}

			export type UserManager = {
				id: string
				name: string
				type: 'user'
				avatar?: string | null
			}

			export type ServerManager = {
				name: string
				type: 'server'
				icon?: string | null
				icon_config?: IconConfig | null
			}

			export type Manager = UserManager | ServerManager

			export type InviteUser = {
				id: string
				name: string
				avatar?: string | null
				joined_at: string | null
			}

			export type InviteLink = {
				id: string
				expiration: string
				max_uses: number
				uses: number
			}

			export type Invite = {
				instance_id: string
				instance_name: string
				instance_icon?: string | null
				instance_icon_config?: IconConfig | null
				game_version: string
				loader_version: string
				inviter: Inviter | null
				managers: Manager[]
				instance_users?: InviteUser[]
				version?: Instances.v1.InstanceVersion
			}
		}
	}

	export namespace Instances {
		export namespace v1 {
			export type OnlineStatus = 'unknown' | 'running' | 'stopped' | null

			export type Instance = {
				name: string
				icon: string | null
				icon_config: IconConfig | null
				quarantine: boolean
				installed_once: boolean
				linked_server: { domain: string; region: string; online_status: OnlineStatus } | null
			}

			export type InstanceLimits = {
				current: number
				max: number
			}

			export type JoinType = 'owner' | 'invite' | 'link'

			/** A server someone else hosts for a shared instance and shows to its members. */
			export type HostedServer = {
				core_id: string
				server_id: string
				name: string
				/** The Minecraft address to ping and join. Not shown to the member. */
				address: string
				host: { id: string; username: string; avatar_url: string | null }
			}

			export type InstanceUser = {
				id: string
				joined_at: string | null
				join_type: JoinType
				last_played: string | null
			}

			export type InstanceUsers = {
				users: InstanceUser[]
			}

			export type FileMetadata = {
				path: string
				hash: string
			}

			export type ExternalFile = {
				file_name: string
				file_type: string
				url: string | null
				file_size?: number
				metadata?: FileMetadata[]
			}

			export type InstanceVersion = {
				version: number
				modrinth_ids?: string[]
				ready: boolean
				external_files: ExternalFile[]
				modpack_id: string | null
				removed_files: (
					| { type: 'version'; version_id: string }
					| { type: 'path'; parent: string; filename: string }
				)[]
				game_version: string
				loader: string
				loader_version: string
			}
		}
	}

	export namespace Storage {
		export namespace v1 {
			export type StoredVersion = {
				version: number
				created: string
				bytes: number
				ready: boolean
				/** The latest ready version. It cannot be deleted. */
				current: boolean
			}

			export type StoredShare = {
				id: string
				name: string
				/** False when the share uses none of the allowance: it is private or has no members. */
				counted: boolean
				/** Bytes this share counts against the allowance, 0 when not counted. */
				used: number
				/** Bytes its kept versions take, counted or not. */
				size: number
				/** The account's own Cores holding it. */
				cores: string[]
				/** Newest first. */
				versions: StoredVersion[]
			}

			/** Only the shares the signed-in account owns. Sizes are in bytes. */
			export type Storage = {
				used: number
				allowance: number
				/** How many versions each share keeps, files included. Null keeps all of them. */
				kept_versions: number | null
				shares: StoredShare[]
			}
		}
	}

	export namespace Moderation {
		export namespace v1 {
			export type BlacklistUserRequest = {
				user_ids: string[]
			}
		}
	}

	export namespace Users {
		export namespace v1 {
			export type BlacklistStatus = {
				blacklisted: boolean
			}
		}
	}
}
