export namespace SharedInstances {
	/**
	 * Sent live to the members of a shared instance when its member list (`shared_instance_users`)
	 * or the servers other members host for it (`shared_instance_servers`) changed, so an open page
	 * fetches it again. It arrives over the friends socket shaped like a notification, but is not
	 * stored and has no id.
	 */
	export type LiveEvent = {
		body: { type: 'shared_instance_users' | 'shared_instance_servers'; shared_instance_id: string }
	}

	export namespace Invites {
		export namespace v1 {
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
			}

			export type Manager = UserManager | ServerManager

			export type InviteUser = {
				id: string
				name: string
				avatar?: string | null
				joined_at: string | null
			}

			export type Invite = {
				instance_id: string
				instance_name: string
				instance_icon?: string | null
				game_version: string
				loader_version: string
				managers: Manager[]
				instance_users?: InviteUser[]
			}
		}
	}

	export namespace Instances {
		export namespace v1 {
			export type Instance = {
				name: string
				icon: string | null
				quarantine: boolean
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
				tokens: number
			}

			export type FileMetadata = {
				path: string
				hash: string
			}

			export type ExternalFile = {
				file_name: string
				file_type: string
				url: string
				file_size?: number
				metadata?: FileMetadata[]
			}

			export type InstanceVersion = {
				version: number
				modrinth_ids?: string[]
				ready: boolean
				external_files: ExternalFile[]
				modpack_id: string | null
				game_version: string
				loader: string
				loader_version: string
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
