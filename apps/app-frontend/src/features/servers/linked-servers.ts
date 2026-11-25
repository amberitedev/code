import {
	core,
	type LinkedInstance,
	ModrinthApiError,
	type PatchPropertiesFields,
	type Server,
} from '@modrinth/api-client'
import { serverListQueryOptions } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, onScopeDispose, ref, toValue, watch } from 'vue'

import { useCores } from '@/features/cores/use-cores'
import { ensure_shared_instance } from '@/helpers/instance'
import { apiClient } from '@/platform/modrinth-client'

export type LinkedServer = Server & { linked_instance: LinkedInstance }

/**
 * The one query for the Cores' server list, under the same key and fetch as the Servers page so
 * both read the same cached list. Each Core's event socket says when it changed
 * ({@link useCoreWatch}), so it is never polled. It never retries: a Core that is down refuses the
 * connection at once, and its socket says when it is back.
 */
const serversQueryOptions = {
	...serverListQueryOptions(apiClient),
	staleTime: Infinity,
	retry: false,
}

/** The ids of the watched Cores whose event socket is not open, and of every watched Core. */
const closedCores = ref(new Set<string>())
const watchedCores = ref(new Set<string>())

/** Whether the Core with this id is offline: its event socket is not open. */
export function isCoreOffline(coreId: string) {
	return closedCores.value.has(coreId)
}

/** Whether every Core is offline: none of the account's Cores has its event socket open. */
export const coreOffline = computed(
	() => watchedCores.value.size > 0 && closedCores.value.size === watchedCores.value.size,
)

/** How long a burst of server changes is gathered into one fetch of the list. */
const CHANGES_SETTLE_MS = 200

/**
 * Keeps the server list current for as long as the App is open, so pages that show it already
 * know the answer when they open: each of the account's Cores tells, on its one event socket, when
 * a server changed and when it is online. Call once, from the App root.
 */
export function useCoreWatch() {
	const queryClient = useQueryClient()
	useQuery(serversQueryOptions)
	const { cores } = useCores()
	let settle: ReturnType<typeof setTimeout> | undefined
	const refetch = () => {
		clearTimeout(settle)
		settle = setTimeout(
			() => void queryClient.invalidateQueries({ queryKey: serversQueryOptions.queryKey }),
			CHANGES_SETTLE_MS,
		)
	}
	const setClosed = (coreId: string, closed: boolean) => {
		const next = new Set(closedCores.value)
		if (closed) next.add(coreId)
		else next.delete(coreId)
		closedCores.value = next
	}

	function watchCore(coreId: string) {
		const events = apiClient.coreEvents(coreId)
		setClosed(coreId, !events.open)
		const stops = [
			events.subscribe({ type: 'servers' }),
			events.on('server_changed', refetch),
			// The socket lost events: nothing shown from this Core can be trusted.
			events.on('resync', () => void queryClient.invalidateQueries()),
			events.onStatus((open) => {
				setClosed(coreId, !open)
				if (open) refetch()
			}),
		]
		return () => {
			for (const stop of stops) stop()
			setClosed(coreId, false)
		}
	}

	const watches = new Map<string, () => void>()
	watch(
		cores,
		(listed) => {
			const ids = new Set(listed.map((core) => core.id))
			for (const [id, stop] of watches) {
				if (ids.has(id)) continue
				stop()
				watches.delete(id)
			}
			for (const id of ids) if (!watches.has(id)) watches.set(id, watchCore(id))
			watchedCores.value = ids
		},
		{ immediate: true },
	)
	onScopeDispose(() => {
		clearTimeout(settle)
		for (const stop of watches.values()) stop()
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

/** The Cores' server list, kept fresh by {@link useCoreWatch}. */
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

/** Whether the server's process is up, including while it starts or stops. */
export function isServerRunning(server: Server) {
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

/** Why a running server cannot be joined from the internet, or undefined when it can. */
export function unreachableCause(server: Server) {
	if (server.power_state !== 'running' || server.reachability?.state !== 'unreachable') {
		return undefined
	}
	return server.reachability.title
}

/** Players on a running server, as Core's health check last saw them. */
export function playersOnline(server: Server) {
	return server.players?.length ?? 0
}

/**
 * Has a server apply the latest version of the instance's share. The server follows the instance
 * through its share like one more member: its Core reads the version from the backend as its
 * owner, and fetches the files from the pool itself, so nothing is uploaded from here. The first
 * push installs the instance and links the server to it. A running server takes the version when
 * it next stops, unless `restart` is set.
 */
export async function pushInstanceToServer(
	instance: { id: string; name: string },
	serverId: string,
	options: { properties?: PatchPropertiesFields; restart?: boolean } = {},
) {
	const share = await ensure_shared_instance(instance.id)
	const routes = await apiClient.cores.serverRoutes(serverId)
	const params = { id: serverId }
	if (options.properties)
		await routes.call(core.ChangeProperties, { params, body: options.properties })
	await routes.call(core.FollowInstance, {
		params,
		body: { instance, shared_instance_id: share.id, restart: options.restart ?? false },
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
