import {
	type Archon,
	ModrinthApiError,
	type SharedInstances,
	type UploadProgress,
} from '@modrinth/api-client'
import type { ContentDiffItem } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { invoke } from '@tauri-apps/api/core'
import { join, tempDir } from '@tauri-apps/api/path'
import { computed, type MaybeRefOrGetter, ref, toValue, watch } from 'vue'

import { apiClient } from '@/services/api-client'

import { export_instance_mrpack, get_content_items } from './instance'
import type { GameInstance } from './types'
import { get_server_status } from './worlds'

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
 * The instance's server on the Core this App is connected to. An instance has at most one, so
 * this is undefined when it has none, or when there is no Core or it cannot be reached.
 */
export function useInstanceServer(instanceId: MaybeRefOrGetter<string | undefined>) {
	const query = useServers()
	const server = computed(() => {
		const id = toValue(instanceId)
		return (query.data.value?.servers ?? []).find(
			(server): server is LinkedServer => !!id && server.linked_instance?.id === id,
		)
	})
	return { server, query }
}

/**
 * Keeps the Core's record of which shared instance the instance's server belongs to in step with
 * the instance. The Core lists a server for friends only under that shared instance, so sharing or
 * unsharing an instance reaches its server through here. Call once, from the instance page.
 */
export function useSharedInstanceLink(instance: MaybeRefOrGetter<GameInstance | undefined>) {
	const queryClient = useQueryClient()
	const { server } = useInstanceServer(() => toValue(instance)?.id)
	let sending = false
	watch(
		() => {
			const sharedId = toValue(instance)?.shared_instance?.id ?? null
			const current = server.value
			return current && current.linked_instance.shared_instance_id !== sharedId
				? { serverId: current.server_id, sharedId }
				: undefined
		},
		async (stale) => {
			if (!stale || sending) return
			sending = true
			// On failure the next server list still differs, which tries again.
			await apiClient.archon.servers_v1
				.setLinkedInstance(stale.serverId, { shared_instance: { id: stale.sharedId } })
				.then(() => queryClient.invalidateQueries({ queryKey: serversQueryKey }))
				.catch((error) => console.error('Failed to tell the Core about sharing', error))
			sending = false
		},
		{ immediate: true },
	)
}

export type HostedServer = SharedInstances.Instances.v1.HostedServer

/**
 * The servers other people host for a shared instance and show to its members. Empty when the
 * instance is not shared. Checked again every few seconds so a server that was hidden, renamed,
 * or moved shows up without a restart.
 */
export function useHostedServers(sharedInstanceId: MaybeRefOrGetter<string | undefined>) {
	const query = useQuery({
		queryKey: computed(() => ['shared-instance-servers', toValue(sharedInstanceId)] as const),
		queryFn: () => apiClient.sharedinstances.instances_v1.getServers(toValue(sharedInstanceId)!),
		enabled: () => !!toValue(sharedInstanceId),
		refetchInterval: 15_000,
		retry: false,
	})
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

/** Players on a running server right now, from a status ping. Zero when it cannot be reached. */
export async function playersOnline(server: Archon.Servers.v0.Server) {
	if (!isServerRunning(server)) return 0
	try {
		return (await get_server_status(serverAddress(server))).players?.online ?? 0
	} catch {
		return 0
	}
}

/**
 * Exports the instance as a `.mrpack` with its mods and config. Modrinth content is listed by
 * reference and anything else is packed in, so custom mods reach the server too.
 */
export async function exportInstancePack(instanceId: string): Promise<File> {
	// One file per instance, overwritten by the next push.
	const path = await join(
		await tempDir(),
		`amberite-server-${instanceId.replace(/[^a-zA-Z0-9-]/g, '_')}.mrpack`,
	)
	await export_instance_mrpack(instanceId, path, ['mods', 'config'], [])
	const bytes = await invoke<number[]>('plugin:files|file_read_dragged_file', { path })
	return new File([new Uint8Array(bytes)], 'instance.mrpack')
}

/**
 * Sends an exported instance to a server on the Core. The first push installs it and links the
 * server; later pushes update it. A running server takes the update on its next stop unless
 * `restart` is set.
 */
export async function pushPackToServer(
	pack: File,
	instance: { id: string; name: string },
	serverId: string,
	options: {
		properties?: Archon.Content.v1.PropertiesFields
		restart?: boolean
		onProgress?: (progress: UploadProgress) => void
	} = {},
) {
	// A Core server has one world, with the server's own id.
	const worldId = serverId
	await apiClient.archon.content_v1.uploadInstance(serverId, worldId, pack, instance, {
		properties: options.properties,
		onProgress: options.onProgress,
	}).promise
	if (options.restart) await apiClient.archon.content_v1.updateModpack(serverId, worldId)
}

/**
 * What a push would change on a server: the instance's enabled mods compared with the mods the
 * server last took from it. Config changes are not detected.
 */
export async function diffInstanceAgainstServer(
	instanceId: string,
	server: Archon.Servers.v0.Server,
): Promise<ContentDiffItem[]> {
	const [items, content] = await Promise.all([
		get_content_items(instanceId),
		apiClient.archon.content_v1.getAddons(server.server_id, server.server_id, {
			from_modpack: true,
		}),
	])
	const wanted = items.filter((item) => item.project_type === 'mod' && item.enabled)
	const onServer = (content.addons ?? []).filter((addon) => addon.kind === 'mod')
	// Modrinth content is matched by project, so a new version is an update, not add plus remove.
	const key = (projectId: string | null | undefined, fileName: string) => projectId ?? fileName
	const serverByKey = new Map(
		onServer.map((addon) => [key(addon.project_id, addon.filename), addon]),
	)
	const wantedKeys = new Set<string>()
	const diffs: ContentDiffItem[] = []
	for (const item of wanted) {
		const hasProject = item.project.id !== item.file_name
		const itemKey = key(hasProject ? item.project.id : null, item.file_name)
		wantedKeys.add(itemKey)
		const existing = serverByKey.get(itemKey)
		const names = { projectName: item.project.title, fileName: item.file_name }
		if (!existing) {
			diffs.push({ type: 'added', ...names, newVersionName: item.version?.version_number })
		} else if (existing.filename !== item.file_name) {
			diffs.push({
				type: 'updated',
				...names,
				currentVersionName: existing.version?.name ?? existing.filename,
				newVersionName: item.version?.version_number ?? item.file_name,
			})
		}
	}
	for (const addon of onServer) {
		if (wantedKeys.has(key(addon.project_id, addon.filename))) continue
		diffs.push({
			type: 'removed',
			projectName: addon.name ?? addon.filename,
			fileName: addon.filename,
		})
	}
	return diffs
}
