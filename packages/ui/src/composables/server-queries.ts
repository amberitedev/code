import type { AbstractModrinthClient } from '@modrinth/api-client'

/** Query keys for servers on Cores. Shorter forms are prefixes of longer ones for invalidation. */
export const serverKeys = {
	list: () => ['servers'] as const,
	detail: (serverId: string) => ['servers', 'detail', serverId] as const,
	content: (serverId: string) => ['content', 'list', 'v1', serverId] as const,
	startup: (serverId: string) => ['servers', 'startup', 'v1', serverId] as const,
	shareDiff: (serverId: string, ...scope: [userId?: string]) =>
		['servers', 'share-diff', serverId, ...scope] as const,
	/** The listings of the server's files, by folder. */
	files: (serverId: string) => ['servers', 'files', serverId] as const,
	shareFiles: (serverId: string, userId: string | undefined) =>
		['servers', 'share-files', serverId, userId] as const,
	sharingUpdateTarget: (userId: string, serverId: string | undefined) =>
		['servers', 'sharing-update-target', userId, serverId ?? null] as const,
}

/** The share history kept with an instance on this computer, or with a server on its Core. */
export const shareHistoryKey = (instanceId: string) => ['share-history', instanceId] as const

/** Query keys for shared instance data. Shorter forms are prefixes of longer ones for invalidation. */
export const sharedInstanceKeys = {
	instance: (instanceId: string, ...scope: [userId?: string]) =>
		['shared-instances', instanceId, 'instance', ...scope] as const,
	// Players and their links belong to one server: on a Core, two servers following the same
	// shared instance have their own.
	players: (instanceId: string, ...scope: [serverId?: string, userId?: string]) =>
		['shared-instances', instanceId, 'players', ...scope] as const,
	invites: (instanceId: string, ...scope: [serverId?: string, userId?: string]) =>
		['shared-instances', instanceId, 'invites', ...scope] as const,
	friends: (userId: string | undefined) => ['shared-instances', 'friends', userId] as const,
}

export function serverListQueryOptions(client: AbstractModrinthClient) {
	return {
		queryKey: serverKeys.list(),
		staleTime: 30_000,
		queryFn: () => client.cores.servers(),
	}
}
