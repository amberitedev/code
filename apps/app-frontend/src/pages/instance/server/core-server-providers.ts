import type { CoreChangeVersionBody, CoreInstance, CoreStats } from '@amberite/amberite-api'
import type { Archon, UploadState } from '@modrinth/api-client'
import type { LogLine } from '@modrinth/ui'
import { provideModrinthServerContext, provideServerSettingsModal } from '@modrinth/ui'
import type { Stats } from '@modrinth/utils'
import type { ComputedRef, Ref } from 'vue'
import { computed, provide, ref } from 'vue'
import { useRouter } from 'vue-router'

import type { CoreServerContext, CoreServerViewData } from './core-server-instance'
import { coreServerContextKey, toHostingServer } from './core-server-instance'

type CoreServerProviderState = {
	instanceId: ComputedRef<string>
	rawInstance: Ref<CoreInstance | null>
	server: ComputedRef<CoreServerViewData | null>
	statsData: Ref<CoreStats | null>
	stats: Ref<Stats>
	powerState: Ref<Archon.Websocket.v0.PowerState>
	isConnected: Ref<boolean>
	isWsAuthIncorrect: Ref<boolean>
	isSyncingContent: Ref<boolean>
	logLines: Ref<LogLine[]>
	refreshServer: () => Promise<void>
	refreshStats: () => Promise<void>
	sendCommand: (command: string) => Promise<void>
	startServer: () => Promise<void>
	stopServer: () => Promise<void>
	restartServer: () => Promise<void>
	killServer: () => Promise<void>
	repairServer: () => Promise<void>
	changeVersion: (body: CoreChangeVersionBody) => Promise<void>
}

export function provideCoreServerRuntime(state: CoreServerProviderState) {
	const router = useRouter()
	const worldId = ref('default')
	const fsAuth = ref(null)
	const fsOps = ref([])
	const fsQueuedOps = ref([])
	const uploadState = ref<UploadState>({
		isUploading: false,
		currentFileName: null,
		currentFileProgress: 0,
		uploadedBytes: 0,
		totalBytes: 0,
		completedFiles: 0,
		totalFiles: 0,
	})
	const cancelUpload = ref(null)
	const activeOperations = computed(() => [])
	const busyReasons = computed(() => [])
	const isServerRunning = computed(() => state.powerState.value === 'running')
	const powerStateDetails = ref(undefined)
	const serverContextServer = computed(
		() => state.server.value ?? toHostingServer(createFallbackInstance(state.instanceId.value)),
	)

	provideModrinthServerContext({
		get serverId() {
			return state.instanceId.value
		},
		worldId,
		server: serverContextServer,
		isConnected: state.isConnected,
		isWsAuthIncorrect: state.isWsAuthIncorrect,
		powerState: state.powerState,
		powerStateDetails,
		isServerRunning,
		stats: state.stats,
		uptimeSeconds: computed(() => state.statsData.value?.uptime_seconds ?? 0),
		isSyncingContent: state.isSyncingContent,
		busyReasons,
		fsAuth,
		fsOps,
		fsQueuedOps,
		refreshFsAuth: async () => {
			fsAuth.value = null
		},
		uploadState,
		cancelUpload,
		activeOperations,
		dismissOperation: () => {},
	})

	provideServerSettingsModal({
		openServerSettings: () => {},
		browseServerContent: () => {
			void router.push(
				`/browse/${state.rawInstance.value?.loader === 'vanilla' ? 'datapack' : 'mod'}`,
			)
		},
	})

	provide<CoreServerContext>(coreServerContextKey, {
		instanceId: state.instanceId,
		server: state.server,
		statsData: state.statsData,
		stats: state.stats,
		powerState: state.powerState,
		logLines: state.logLines,
		refreshServer: state.refreshServer,
		refreshStats: state.refreshStats,
		sendCommand: state.sendCommand,
		startServer: state.startServer,
		stopServer: state.stopServer,
		restartServer: state.restartServer,
		killServer: state.killServer,
		repairServer: state.repairServer,
		changeVersion: state.changeVersion,
	})
}

function createFallbackInstance(id: string): CoreInstance {
	return {
		id,
		name: id,
		game_version: 'Unknown',
		loader: 'vanilla',
		loader_version: null,
		port: 25565,
		memory: { min_mb: 0, max_mb: 0 },
		java_version: null,
		install_status: 'ready',
		status: 'offline',
		data_dir: '',
		created_at: new Date().toISOString(),
		updated_at: new Date().toISOString(),
	}
}
