<script setup lang="ts">
import { instanceFiles } from '@amberite/contracts/engine'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import ConfigPushPromptModal from '#ui/components/servers/ConfigPushPromptModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerDownload } from '#ui/composables/server-download'
import { useServerPermissions } from '#ui/composables/server-permissions'
import { changeServerPower } from '#ui/composables/server-power'
import { serverKeys } from '#ui/composables/server-queries'
import { toShareableConfigPath } from '#ui/composables/server-sharing/config-files'
import { useServerOwnShare } from '#ui/composables/server-sharing/own-share'
import { useServerShareReview } from '#ui/composables/server-sharing/use-server-share-review'
import {
	injectModrinthClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

import FilePageLayout from '../../../shared/files-tab/layout.vue'
import { provideFileManager } from '../../../shared/files-tab/providers/file-manager'
import {
	type FilesFailure,
	instancePath,
	useInstanceFiles,
} from '../../../shared/files-tab/providers/instance-files'

const props = defineProps<{
	showDebugInfo?: boolean
	showRefreshButton?: boolean
}>()

const client = injectModrinthClient()
const serverContext = injectModrinthServerContext()
const { serverId, coreId, core, busyReasons } = serverContext
const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { canWriteFiles, canUsePowerActions, canSetup, permissionDeniedMessage } =
	useServerPermissions()
const { pending: sharePending, runAction: runShareAction } = useServerShareReview()
const serverDownload = useServerDownload()

const route = useRoute()
const router = useRouter()

const messages = defineMessages({
	zipCreated: {
		id: 'servers.files.zip-created',
		defaultMessage: 'ZIP created',
	},
})

const failureTitles = {
	create: commonMessages.createFailedLabel,
	rename: commonMessages.renameFailedLabel,
	move: commonMessages.moveFailedLabel,
	delete: commonMessages.deleteFailedLabel,
	upload: commonMessages.uploadFailedLabel,
	write: commonMessages.uploadFailedLabel,
	zip: commonMessages.zipFailedLabel,
} satisfies Record<FilesFailure, unknown>

const serverBusy = computed(() => busyReasons.value.length > 0)
const busyTooltip = computed(() =>
	busyReasons.value.length > 0 ? formatMessage(busyReasons.value[0].reason) : undefined,
)
const fileWriteDisabled = computed(() => !canWriteFiles.value || serverBusy.value)
const fileWriteDisabledTooltip = computed(() =>
	canWriteFiles.value ? busyTooltip.value : permissionDeniedMessage.value,
)
const nonBackupBusyReasons = computed(() =>
	busyReasons.value.filter(
		(r) =>
			r.reason.id !== 'servers.busy.backup-creating' &&
			r.reason.id !== 'servers.busy.backup-restoring',
	),
)
const busyWarning = computed(() =>
	nonBackupBusyReasons.value.length > 0
		? formatMessage(nonBackupBusyReasons.value[0].reason)
		: null,
)

// Config pushes go to the server's own share; a server that follows an instance has none.
const {
	sharedInstanceId,
	installedOnce: sharedInstanceInstalledOnce,
	needsUpdate: shareNeedsUpdate,
} = useServerOwnShare()
const configPushModal = ref<InstanceType<typeof ConfigPushPromptModal>>()
const configPushDeferredPaths = new Set<string>()

async function promptConfigPush(path: string) {
	const configPath = toShareableConfigPath(`/${path}`)
	if (
		!configPath ||
		configPushDeferredPaths.has(configPath) ||
		!canSetup.value ||
		sharePending.value ||
		!sharedInstanceId.value ||
		!sharedInstanceInstalledOnce.value
	)
		return
	const choice = await configPushModal.value?.show(configPath)
	if (choice === 'push') await runShareAction('push', false, [configPath])
	else if (choice === 'later') configPushDeferredPaths.add(configPath)
}

// The folder shown is in the URL, so back navigation returns to it.
const currentPath = computed({
	get: () => instancePath(typeof route.query.path === 'string' ? route.query.path : ''),
	set: (path: string) => {
		const { editing: _, ...query } = route.query
		void router.push({ query: { ...query, path: `/${path}` } })
	},
})

const files = useInstanceFiles({
	client: core,
	events: client.coreEvents(coreId),
	instanceId: serverId,
	currentPath,
	queryKey: (path) => serverKeys.directory(serverId, path),
	queryKeyPrefix: serverKeys.files(serverId),
	uploadState: serverContext.uploadState,
	onFailure: (failure, error) =>
		addNotification({
			title: formatMessage(failureTitles[failure]),
			text: error instanceof Error ? error.message : undefined,
			type: 'error',
		}),
	onWritten: (path) => void promptConfigPush(path),
})
watch(
	() => files.uploadState.value.isUploading,
	(uploading) => {
		serverContext.cancelUpload.value = uploading ? files.cancelUpload : null
	},
)

// The file being edited is in the URL too.
watch(
	() => route.query.editing,
	(editing) => {
		if (typeof editing !== 'string') {
			files.editingFile.value = null
			return
		}
		if (files.editingFile.value?.path !== editing)
			files.editingFile.value = { name: editing.split('/').pop() || '', path: editing }
	},
	{ immediate: true },
)
watch(
	() => files.editingFile.value?.path,
	() => configPushDeferredPaths.clear(),
)

/** Runs `change` unless writing is off now. */
function guarded<Args extends unknown[]>(change: (...args: Args) => Promise<unknown>) {
	return async (...args: Args) => {
		if (!fileWriteDisabled.value) await change(...args)
	}
}

provideFileManager({
	items: files.items,
	loading: files.loading,
	error: files.error,
	currentPath,
	navigateTo: files.navigateTo,
	editingFile: files.editingFile,
	startEditing: (file) => {
		files.startEditing(file)
		void router.push({ query: { ...route.query, editing: file.path } })
	},
	stopEditing: () => {
		files.stopEditing()
		const { editing: _, ...query } = route.query
		void router.replace({ query })
	},
	createItem: guarded(files.createItem),
	renameItem: guarded(files.renameItem),
	moveItem: guarded(files.moveItem),
	deleteItem: guarded(files.deleteItem),
	readFile: files.readFile,
	readFileAsBlob: files.readFileAsBlob,
	writeFile: guarded(files.writeFile),
	downloadFile: (path, fileName) =>
		serverDownload.download('server-file', fileName, instanceFiles.ReadInstanceFile, {
			params: { id: serverId },
			query: { path: instancePath(path) },
		}),
	statFile: files.statFile,
	zipFolder: guarded(async (path: string) => {
		await files.zipFolder(path)
		addNotification({ title: formatMessage(messages.zipCreated), type: 'success' })
	}),
	zipPaths: guarded(async (parent: string, include: string[], target: string) => {
		await files.zipPaths(parent, include, target)
		addNotification({ title: formatMessage(messages.zipCreated), type: 'success' })
	}),
	uploadFiles: (uploaded) => {
		if (!fileWriteDisabled.value) void files.uploadFiles(uploaded)
	},
	cancelUpload: files.cancelUpload,
	uploadState: files.uploadState,
	// The panel shows an upload's progress on every tab.
	showUploadProgress: false,
	refresh: files.refresh,
	isBusy: fileWriteDisabled,
	busyTooltip: fileWriteDisabledTooltip,
	busyWarning,
	extractFile: async (path, override, dry) =>
		fileWriteDisabled.value ? undefined : files.extractFile(path, override, dry),
	prefetchDirectory: files.prefetchDirectory,
	canRestart: canUsePowerActions.value,
	restartServer: async () => {
		if (canUsePowerActions.value) await changeServerPower(client, coreId, serverId, 'Restart')
	},
	canShareToMclogs: true,
})
</script>

<template>
	<ReadyTransition :pending="files.firstPaintPending.value">
		<FilePageLayout
			:show-debug-info="props.showDebugInfo"
			:show-refresh-button="props.showRefreshButton"
		/>
	</ReadyTransition>
	<ConfigPushPromptModal
		ref="configPushModal"
		:action-disabled="!canSetup || sharePending"
		:includes-other-changes="shareNeedsUpdate"
		:server-running="serverContext.isServerRunning.value"
	/>
</template>
