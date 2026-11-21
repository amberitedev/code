import type { AuditActor, AuditWorld, ParsedAuditEvent } from './events/types'

export interface ServerAuditLogEntry {
	id: string
	actor: AuditActor
	world: AuditWorld | null
	event: ParsedAuditEvent
	timestamp: string
}

export interface ServerAuditLogFilters {
	userId: string | null
	worldId: string | null
}

export interface ServerListingOwner {
	username: string
	avatarUrl?: string
}
