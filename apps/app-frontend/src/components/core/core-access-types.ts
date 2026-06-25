import type { ServerAccessMember } from '@modrinth/ui'

export interface CoreAccessMember extends ServerAccessMember {
	inviteCandidate?: boolean
}
