import { type Archon, ModrinthApiError, type SharedInstances } from '@modrinth/api-client'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, ref, toValue, watch } from 'vue'

import { useSharedInstanceEvent } from '@/composables/use-shared-instance-event'
import { ensure_shared_instance, get_shared_instance_server_pack } from '@/helpers/instance'
import { apiClient } from '@/platform/modrinth-client'

export type LinkedServer = Archon.Servers.v0.Server & {
	linked_instance: Archon.Servers.v0.LinkedInstance
}

/** Shared with the Servers page, so both read the same cached list. */
export const serversQueryKey = ['servers'] as const

/**
 * The one query for the Core's server list. It never retries: a Core that is down refuses the
 * connection at once, and the next check is only seconds away.
 */
const serversQueryOptions = {
	queryKey: serversQueryKey,
	queryFn: () => apiClient.archon.servers_v0.list({ limit: 100 }),
	enabled: () => apiClient.selfHostedHosting,
	staleTime: 5_000,
	refetchInterval: 5_000,
	retry: false,
}

const failedChecks = ref(0)
const everAnswered = ref(false)

/**
 * Whether the Core counts as offline. One failed check is not enough once the Core has answered,
 * so a dropped request does not flip the screen; the first success brings it straight back.
 */
export const coreOffline = computed(
	() => failedChecks.value >= 2 || (failedChecks.value >= 1 && !everAnswered.value),
)

/**
 * Keeps the server list checked for as long as the App is open, so pages that show it already
 * know the answer when they open. Call once, from the App root.
 */
export function useCoreWatch() {
	const query = useQuery(serversQueryOptions)
	watch(query.dataUpdatedAt, (at) => {
		if (!at) return
		everAnswered.value = true
		failedChecks.value = 0
	})
	watch(query.errorUpdatedAt, (at) => {
		if (at) failedChecks.value += 1
	})
}

/**
 * What to tell the user when a request to a Core fails. The Core's own wording is for the log:
 * no answer means it is off or unreachable, a refusal means it is no longer signed in to the account.
 */
export function coreProblem(cause: unknown): string {
	const status = cause instanceof ModrinthApiError ? cause.statusCode : undefined
	if (status === undefined) return 'Check that your Core is running, then try again.'
	if (status === 401 || status === 403)
		return 'Your Core is not signed in to this account. Add it again from Core settings.'
	return 'Your Core ran into a problem. Try again in a moment.'
}

/** The Core's server list, kept fresh by {@link useCoreWatch}. */
export function useServers() {
	return useQuery(serversQueryOptions)
}

/**
 * The servers that follow an instance, on the Core this App is connected to. Empty when there
 * is no Core or it cannot be reached.
 */
export function useLinkedServers(instanceId: MaybeRefOrGetter<string | undefined>) {
	const query = useServers()
	const servers = computed(() => {
		const id = toValue(instanceId)
		return (query.data.value?.servers ?? []).filter(
			(server): server is LinkedServer => !!id && server.linked_instance?.id === id,
		)
	})
	return { servers, query }
}

export type HostedServer = SharedInstances.Instances.v1.HostedServer

/**
 * The servers other people host for a shared instance and show to its members. Empty when the
 * instance is not shared. Fetched again when the backend says the list changed, so a server that
 * was hidden, renamed, or moved shows up without a restart.
 */
export function useHostedServers(sharedInstanceId: MaybeRefOrGetter<string | undefined>) {
	const queryClient = useQueryClient()
	const queryKey = computed(() => ['shared-instance-servers', toValue(sharedInstanceId)] as const)
	const query = useQuery({
		queryKey,
		queryFn: () => apiClient.sharedinstances.instances_v1.getServers(toValue(sharedInstanceId)!),
		enabled: () => !!toValue(sharedInstanceId),
		retry: false,
	})
	useSharedInstanceEvent(
		'shared_instance_servers',
		sharedInstanceId,
		() => void queryClient.invalidateQueries({ queryKey: queryKey.value }),
	)
	const servers = computed(() => (toValue(sharedInstanceId) ? (query.data.value ?? []) : []))
	return { servers, query }
}

/**
 * Whether this account may manage a server someone else hosts, which makes its Worlds row open
 * the server page. Access is given per server, not per Core, so the answer is whether the server
 * is in the account's own server list, which holds every server it can reach.
 */
export function useCanManageHostedServer() {
	const query = useServers()
	const reachable = computed(
		() => new Set((query.data.value?.servers ?? []).map((server) => server.server_id)),
	)
	return (server: HostedServer) => reachable.value.has(server.server_id)
}

/** Whether the server's process is up, including while it starts or stops. */
export function isServerRunning(server: Archon.Servers.v0.Server) {
	return (
		server.power_state === 'running' ||
		server.power_state === 'starting' ||
		server.power_state === 'stopping'
	)
}

type LinkedServerState = 'running' | 'offline' | 'updating' | 'pending' | 'failed'

/** The one state shown for a linked server. An update in progress wins over power state. */
export function linkedServerState(server: LinkedServer): LinkedServerState {
	const update = server.linked_instance.state
	if (update === 'failed') return 'failed'
	if (update === 'downloading' || update === 'applying' || server.status === 'installing') {
		return 'updating'
	}
	if (update === 'pending' && isServerRunning(server)) return 'pending'
	return isServerRunning(server) ? 'running' : 'offline'
}

/** Where this App joins and pings the server: its address on the Core's own network. */
export function serverAddress(server: Archon.Servers.v0.Server) {
	return `${server.net.domain || server.net.ip || '127.0.0.1'}:${server.net.port}`
}

/** The address to give to friends: the one on the internet when the server has one. */
export function shareableServerAddress(server: Archon.Servers.v0.Server) {
	const outside = server.net.outside
	return outside ? `${outside.ip}:${outside.port}` : serverAddress(server)
}

/** Why a running server cannot be joined from the internet, or undefined when it can. */
export function unreachableCause(server: Archon.Servers.v0.Server) {
	if (server.power_state !== 'running' || server.reachability?.state !== 'unreachable') {
		return undefined
	}
	return server.reachability.cause ?? undefined
}

/** Players on a running server, as Core's health check last saw them. */
export function playersOnline(server: Archon.Servers.v0.Server) {
	return server.players?.length ?? 0
}

/**
 * Has a server apply the latest version of the instance's share. The server follows the instance
 * through its share like one more member: its Core fetches the files from the pool itself, so
 * nothing is uploaded from here. The first push installs the instance and links the server to
 * it. A running server takes the version when it next stops, unless `restart` is set.
 */
export async function pushInstanceToServer(
	instance: { id: string; name: string },
	serverId: string,
	options: { properties?: Archon.Content.v1.PropertiesFields; restart?: boolean } = {},
) {
	await ensure_shared_instance(instance.id)
	const share = await get_shared_instance_server_pack(instance.id)
	// A Core server has one world, with the server's own id.
	const worldId = serverId
	await apiClient.archon.content_v1.installContent(serverId, worldId, {
		content_variant: 'instance',
		instance,
		share,
		properties: options.properties,
		restart: options.restart,
	})
}

/** Whether a server runs something other than the share's latest version. */
export function isServerBehind(server: LinkedServer, sharedInstanceId: string, version: number) {
	const linked = server.linked_instance
	return (
		linked.state === 'failed' ||
		linked.shared_instance_id !== sharedInstanceId ||
		linked.version !== version
	)
}
