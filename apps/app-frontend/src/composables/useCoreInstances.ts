import type {
	CoreApiClient,
	CoreEventStream,
	CoreInstance,
	CoreInstanceEvent,
	CoreInstanceSummary,
	CoreInstanceStatus,
} from '@amberite/amberite-api'
import { useQuery, useQueryClient, type QueryClient } from '@tanstack/vue-query'
import type { ComputedRef } from 'vue'
import { computed, onMounted, onUnmounted } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

export const CORE_INSTANCES_QUERY_KEY = ['core-instances', 'v1'] as const

const DISPLAY_CACHE_KEY = 'amberite:core-instances:v1'
const DISPLAY_CACHE_VERSION = 1

interface PersistedCoreInstances {
	version: typeof DISPLAY_CACHE_VERSION
	instances: CoreInstanceSummary[]
	updatedAt: number
}

export interface UseCoreInstancesReturn {
	instances: ComputedRef<Map<string, CoreInstanceSummary>>
	loading: ComputedRef<boolean>
	error: ComputedRef<Error | null>
	refresh: () => Promise<void>
	start: (pathOrId: string) => Promise<void>
	stop: (pathOrId: string) => Promise<void>
	restart: (pathOrId: string) => Promise<void>
	kill: (pathOrId: string) => Promise<void>
}

let subscribers = 0
let stream: CoreEventStream | null = null
let openingStream: Promise<void> | null = null
let reconnectTimer: ReturnType<typeof setTimeout> | null = null

export function useCoreInstances(): UseCoreInstancesReturn {
	const client = useCoreClient()
	const queryClient = useQueryClient()

	const query = useQuery({
		queryKey: CORE_INSTANCES_QUERY_KEY,
		queryFn: async () => {
			const instances = await client.listInstances()
			persistCoreInstances(instances)
			return instances
		},
		initialData: () => {
			const cached =
				queryClient.getQueryData<CoreInstanceSummary[]>(CORE_INSTANCES_QUERY_KEY) ??
				loadPersistedCoreInstances()
			return cached.length > 0 ? cached : undefined
		},
		staleTime: Infinity,
		gcTime: 24 * 60 * 60_000,
		retry: false,
		refetchOnMount: false,
		refetchOnReconnect: false,
		refetchOnWindowFocus: false,
	})

	const instances = computed(
		() => new Map((query.data.value ?? []).map((instance) => [instance.id, instance])),
	)
	const loading = computed(() => query.isFetching.value)
	const error = computed(() => {
		const value = query.error.value
		if (!value) return null
		return value instanceof Error ? value : new Error(String(value))
	})

	async function refresh(): Promise<void> {
		await refreshCoreInstances(queryClient, client)
	}

	onMounted(() => {
		subscribers += 1
		void ensureCoreEventStream(queryClient, client)
	})

	onUnmounted(() => {
		subscribers = Math.max(0, subscribers - 1)
		if (subscribers === 0) {
			closeCoreEventStream()
		}
	})

	return {
		instances,
		loading,
		error,
		refresh,
		start: (pathOrId) =>
			runStatusAction(queryClient, client, pathOrId, 'starting', (core) =>
				core.start(pathOrId),
			),
		stop: (pathOrId) =>
			runStatusAction(queryClient, client, pathOrId, 'stopping', (core) =>
				core.stop(pathOrId),
			),
		restart: (pathOrId) =>
			runStatusAction(queryClient, client, pathOrId, 'starting', (core) =>
				core.restart(pathOrId),
			),
		kill: (pathOrId) =>
			runStatusAction(queryClient, client, pathOrId, 'offline', (core) =>
				core.kill(pathOrId),
			),
	}
}

export function toCoreInstanceSummary(
	instance: CoreInstance | CoreInstanceSummary,
): CoreInstanceSummary {
	return {
		id: instance.id,
		path: instance.path,
		name: instance.name,
		game_version: instance.game_version,
		loader: instance.loader,
		loader_version: instance.loader_version,
		port: instance.port,
		memory: instance.memory,
		install_status: instance.install_status,
		status: instance.status,
		installation_id: instance.installation_id,
		created_at: instance.created_at,
		updated_at: instance.updated_at,
	}
}

export function setCoreInstancesInCache(
	queryClient: QueryClient,
	instances: CoreInstanceSummary[],
): void {
	queryClient.setQueryData(CORE_INSTANCES_QUERY_KEY, instances)
	persistCoreInstances(instances)
}

export function upsertCoreInstanceInCache(
	queryClient: QueryClient,
	instance: CoreInstance | CoreInstanceSummary,
): void {
	const summary = toCoreInstanceSummary(instance)
	const instances = queryClient.getQueryData<CoreInstanceSummary[]>(CORE_INSTANCES_QUERY_KEY) ?? []
	const existingIndex = instances.findIndex((item) => item.id === summary.id)
	const next =
		existingIndex === -1
			? [...instances, summary]
			: instances.map((item, index) => (index === existingIndex ? summary : item))
	setCoreInstancesInCache(queryClient, next)
}

export function patchCoreInstanceInCache(
	queryClient: QueryClient,
	id: string,
	patch: Partial<CoreInstanceSummary>,
): void {
	const instances = queryClient.getQueryData<CoreInstanceSummary[]>(CORE_INSTANCES_QUERY_KEY)
	if (!instances) return
	const next = instances.map((instance) =>
		instance.id === id ? { ...instance, ...patch } : instance,
	)
	setCoreInstancesInCache(queryClient, next)
}

export function removeCoreInstanceFromCache(queryClient: QueryClient, id: string): void {
	const instances = queryClient.getQueryData<CoreInstanceSummary[]>(CORE_INSTANCES_QUERY_KEY)
	if (!instances) return
	setCoreInstancesInCache(
		queryClient,
		instances.filter((instance) => instance.id !== id),
	)
}

export function findCoreInstanceInCache(
	queryClient: QueryClient,
	pathOrId: string,
): CoreInstanceSummary | null {
	const instances = queryClient.getQueryData<CoreInstanceSummary[]>(CORE_INSTANCES_QUERY_KEY) ?? []
	return instances.find((instance) => instance.id === pathOrId || instance.path === pathOrId) ?? null
}

async function refreshCoreInstances(
	queryClient: QueryClient,
	client: CoreApiClient,
): Promise<CoreInstanceSummary[]> {
	const instances = await client.listInstances()
	setCoreInstancesInCache(queryClient, instances)
	return instances
}

async function ensureCoreEventStream(
	queryClient: QueryClient,
	client: CoreApiClient,
): Promise<void> {
	if (stream || openingStream || subscribers === 0) return
	openingStream = (async () => {
		try {
			const nextStream = await client.openEvents()
			if (subscribers === 0) {
				nextStream.close()
				return
			}

			stream = nextStream
			nextStream.onEvent((event) => applyCoreEvent(queryClient, event))
			nextStream.onError(() => scheduleCoreEventReconnect(queryClient, client))
			nextStream.onClose(() => {
				if (stream === nextStream) stream = null
				scheduleCoreEventReconnect(queryClient, client)
			})

			await refreshCoreInstances(queryClient, client).catch(() => undefined)
		} catch {
			scheduleCoreEventReconnect(queryClient, client)
		} finally {
			openingStream = null
		}
	})()
	await openingStream
}

function scheduleCoreEventReconnect(queryClient: QueryClient, client: CoreApiClient): void {
	if (subscribers === 0 || reconnectTimer) return
	reconnectTimer = setTimeout(() => {
		reconnectTimer = null
		void ensureCoreEventStream(queryClient, client)
	}, 3_000)
}

function closeCoreEventStream(): void {
	if (reconnectTimer) {
		clearTimeout(reconnectTimer)
		reconnectTimer = null
	}
	stream?.close()
	stream = null
	openingStream = null
}

function applyCoreEvent(
	queryClient: QueryClient,
	event: CoreInstanceEvent,
): void {
	switch (event.type) {
		case 'instance_created':
		case 'instance_updated':
			upsertCoreInstanceInCache(queryClient, event.instance)
			break
		case 'instance_deleted':
			removeCoreInstanceFromCache(queryClient, event.instance_id)
			break
		case 'status_changed':
			patchCoreInstanceInCache(queryClient, event.instance_id, { status: event.status })
			break
		case 'install_status_changed':
			patchCoreInstanceInCache(queryClient, event.instance_id, {
				install_status: event.install_status,
			})
			break
	}
}

async function runStatusAction(
	queryClient: QueryClient,
	client: CoreApiClient,
	pathOrId: string,
	status: CoreInstanceStatus,
	action: (client: CoreApiClient) => Promise<void>,
): Promise<void> {
	const cached = findCoreInstanceInCache(queryClient, pathOrId)
	if (cached) patchCoreInstanceInCache(queryClient, cached.id, { status })
	try {
		await action(client)
	} catch (error) {
		await refreshCoreInstances(queryClient, client).catch(() => undefined)
		throw error
	}
}

function loadPersistedCoreInstances(): CoreInstanceSummary[] {
	if (typeof localStorage === 'undefined') return []
	try {
		const raw = localStorage.getItem(DISPLAY_CACHE_KEY)
		if (!raw) return []
		const parsed = JSON.parse(raw) as Partial<PersistedCoreInstances>
		if (parsed.version !== DISPLAY_CACHE_VERSION || !Array.isArray(parsed.instances)) {
			return []
		}
		return parsed.instances.filter(isCoreInstanceSummary)
	} catch {
		return []
	}
}

function persistCoreInstances(instances: CoreInstanceSummary[]): void {
	if (typeof localStorage === 'undefined') return
	try {
		const payload: PersistedCoreInstances = {
			version: DISPLAY_CACHE_VERSION,
			instances,
			updatedAt: Date.now(),
		}
		localStorage.setItem(DISPLAY_CACHE_KEY, JSON.stringify(payload))
	} catch {
	}
}

function isCoreInstanceSummary(value: unknown): value is CoreInstanceSummary {
	if (!value || typeof value !== 'object') return false
	const instance = value as Partial<CoreInstanceSummary>
	return (
		typeof instance.id === 'string' &&
		typeof instance.path === 'string' &&
		typeof instance.name === 'string' &&
		typeof instance.game_version === 'string' &&
		typeof instance.loader === 'string' &&
		typeof instance.port === 'number' &&
		typeof instance.status === 'string' &&
		typeof instance.install_status === 'string' &&
		typeof instance.created_at === 'string' &&
		typeof instance.updated_at === 'string'
	)
}
