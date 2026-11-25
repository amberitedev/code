import type { CoreEvent, PowerState, Server, ServerPower } from '@amberite/contracts/core'
import type { UploadState } from '@modrinth/api-client'
import { useQueryClient } from '@tanstack/vue-query'
import type { ComputedRef, Ref } from 'vue'
import { computed, onScopeDispose, ref, watch } from 'vue'

import type { FileOperation } from '../layouts/shared/files-tab/types'
import { injectModrinthClient, provideModrinthServerContext } from '../providers'
import type { BusyReason, CancelUploadHandler } from '../providers/server-context'
import { defineMessage } from './i18n'
import { useInstallJobs } from './install-jobs'
import { createConsoleState } from './server-console'
import { useServerInstallationTracker } from './server-installation-tracker'
import { serverKeys } from './server-queries'

/** How long a burst of changes waits before the server is read again, so it is read once. */
const CHANGES_SETTLE_MS = 500

type BackupProgress = Extract<CoreEvent, { type: 'backup_progress' }>['payload']

type UseServerManageCoreRuntimeOptions = {
	serverId: string
	coreId: string
	server: Ref<Server | undefined>
	extraBusyReasons?: ComputedRef<BusyReason[]>
	onBackupProgress?: (progress: BackupProgress) => void
}

/**
 * The open server's live state, from its Core's one event socket: power, uptime, its console,
 * its install jobs and backups. Provides the server context every tab of the panel reads.
 */
export function useServerManageCoreRuntime(options: UseServerManageCoreRuntimeOptions) {
	const client = injectModrinthClient()
	const queryClient = useQueryClient()
	// Each panel has its own console: a Core sends the backlog on subscribe, and the next panel
	// may mount before this one unmounts.
	const serverConsole = createConsoleState()
	const { serverId, coreId } = options
	const core = client.core(coreId)
	const events = client.coreEvents(coreId)

	const isConnected = ref(events.open)
	const powerState = ref<PowerState>('stopped')
	// The server's power until the socket says otherwise.
	let heardPower = false
	watch(
		() => options.server.value?.power_state,
		(power) => {
			if (power && !heardPower) powerState.value = fromServerPower(power)
		},
		{ immediate: true },
	)
	const isServerRunning = computed(() => powerState.value === 'running')
	const uptimeSeconds = ref(0)
	let uptimeTicker: ReturnType<typeof setInterval> | undefined

	const jobs = useInstallJobs({
		client: core,
		instanceId: serverId,
		onJob: (handler) => events.on('install_job', handler),
	})
	const tracker = useServerInstallationTracker(jobs)

	const busyReasons = computed<BusyReason[]>(() => [
		...(tracker.isBlocking.value || options.server.value?.status === 'installing'
			? [
					{
						reason: defineMessage({
							id: 'servers.busy.installing',
							defaultMessage: 'Server is installing',
						}),
					},
				]
			: []),
		...(options.extraBusyReasons?.value ?? []),
	])

	function setUptime(seconds: number) {
		clearInterval(uptimeTicker)
		uptimeSeconds.value = seconds
		uptimeTicker = setInterval(() => uptimeSeconds.value++, 1000)
	}

	let settle: ReturnType<typeof setTimeout> | undefined
	/** What the server shows may have changed: read it again, once for a burst of changes. */
	function changed() {
		settle ??= setTimeout(() => {
			settle = undefined
			void queryClient.invalidateQueries({ queryKey: serverKeys.detail(serverId) })
			void queryClient.invalidateQueries({ queryKey: serverKeys.content(serverId) })
		}, CHANGES_SETTLE_MS)
	}

	let wasClosed = false
	const stops = [
		...[
			{ type: 'installs' as const },
			{ type: 'server' as const, id: serverId },
			{ type: 'backups' as const, id: serverId },
			{ type: 'console' as const, id: serverId },
			{ type: 'instance' as const, id: serverId },
		].map((topic) => events.subscribe(topic)),
		events.onStatus((open) => {
			isConnected.value = open
			// A reopened socket starts the console again from its backlog.
			if (open && wasClosed) serverConsole.clear()
			wasClosed = !open
		}),
		events.on('resync', () => void jobs.refresh()),
		events.on('power_state', (power) => {
			if (power.server !== serverId) return
			heardPower = true
			powerState.value = power.state
			if (power.state !== 'running') {
				clearInterval(uptimeTicker)
				uptimeSeconds.value = 0
			}
		}),
		events.on('uptime', (uptime) => {
			if (uptime.server === serverId) setUptime(uptime.uptime)
		}),
		events.on('log', (log) => {
			if (log.instance_id !== serverId) return
			if (log.type === 'legacy') serverConsole.addLegacyLog(log.message)
			else serverConsole.addLog4jEvent(log)
		}),
		events.on('backup_progress', (progress) => {
			if (progress.server === serverId) options.onBackupProgress?.(progress)
		}),
		events.on('server_changed', (change) => {
			if (change.server === serverId) changed()
		}),
		events.on('instance', (instance) => {
			if (instance.instance_id === serverId) changed()
		}),
	]
	onScopeDispose(() => {
		for (const stop of stops) stop()
		jobs.stop()
		clearInterval(uptimeTicker)
		clearTimeout(settle)
	})

	const uploadState = ref<UploadState>({
		isUploading: false,
		currentFileName: null,
		currentFileProgress: 0,
		uploadedBytes: 0,
		totalBytes: 0,
		completedFiles: 0,
		totalFiles: 0,
	})
	const cancelUpload = ref<CancelUploadHandler | null>(null)

	const localFileOperations = ref<FileOperation[]>([])
	const activeOperations = computed(() => localFileOperations.value)
	async function dismissOperation(opId: string) {
		localFileOperations.value = localFileOperations.value.filter(
			(operation) => operation.id !== opId,
		)
	}
	function upsertLocalFileOperation(operation: FileOperation) {
		const others = localFileOperations.value.filter((item) => item.id !== operation.id)
		localFileOperations.value = [...others, operation]
	}

	provideModrinthServerContext({
		serverId,
		coreId,
		core,
		server: options.server,
		console: serverConsole,
		isConnected,
		powerState,
		isServerRunning,
		uptimeSeconds,
		installation: tracker.installation,
		beginInstallation: tracker.begin,
		cancelOptimisticInstallation: tracker.cancelOptimistic,
		dismissInstallation: tracker.dismiss,
		busyReasons,
		uploadState,
		cancelUpload,
		activeOperations,
		dismissOperation,
		upsertLocalFileOperation,
	})

	return {
		busyReasons,
		cancelUpload,
		installation: tracker.installation,
		beginInstallation: tracker.begin,
		cancelOptimisticInstallation: tracker.cancelOptimistic,
		dismissInstallation: tracker.dismiss,
		isConnected,
		isServerRunning,
		powerState,
		uptimeSeconds,
		uploadState,
	}
}

function fromServerPower(power: ServerPower): PowerState {
	return power === 'offline' || power === 'killed' ? 'stopped' : power
}
