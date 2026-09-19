import type { Archon } from '../../api-client/src/modules/archon/types.ts'
import type { Labrinth } from '../../api-client/src/modules/labrinth/types.ts'
import type { SharedInstances } from '../../api-client/src/modules/shared-instances/types.ts'

export type PersonaId = 'owner' | 'recipient'

export type Persona = {
	id: PersonaId
	token: string
	user: Labrinth.Users.v3.User
}

export type StoredExternalFile = SharedInstances.Instances.v1.ExternalFile & {
	uploaded: boolean
}

export type SharedVersion = Omit<SharedInstances.Instances.v1.InstanceVersion, 'external_files'> & {
	external_files: StoredExternalFile[]
}

export type SharedInvite = {
	id: string
	expiration: string
	max_uses: number
	uses: number
}

export type SharedInstanceState = SharedInstances.Instances.v1.Instance & {
	id: string
	owner_id: string
	users: SharedInstances.Instances.v1.InstanceUser[]
	tokens: number
	versions: SharedVersion[]
	invites: SharedInvite[]
	pending_user_ids: string[]
}

export type ArchonState = {
	server_v0: Archon.Servers.v0.Server
	server_v1: Archon.Servers.v1.ServerFull
	power_state: Archon.Websocket.v0.PowerState
	started_at: string | null
	content: Archon.Content.v1.Addons
	properties: Archon.Content.v1.PropertiesFields
	startup: Archon.Content.v1.RuntimeOptions
	allocations: Archon.Servers.v0.Allocation[]
	users: Archon.ServerUsers.v1.ServerUser[]
	backups: Archon.Backups.v1.Backup[]
	backup_queue: Archon.BackupsQueue.v1.BackupsQueueResponse
	actions: Archon.Actions.v1.ActionEntry[]
}

export type LabState = {
	schema_version: 1
	next_backup: number
	next_invite: number
	next_shared_version: number
	archon: ArchonState
	shared_instances: SharedInstanceState[]
	preferences: Record<string, Labrinth.Users.v3.UserPreferences>
}

export type TraceEntry = {
	time: string
	request_id: string
	method: string
	path: string
	status: number
	persona: PersonaId | null
	request_headers: Record<string, string>
	duration_ms: number
}
