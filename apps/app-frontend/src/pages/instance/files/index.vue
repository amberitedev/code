<script setup lang="ts">
import {
	commonMessages,
	defineMessages,
	FilePageLayout,
	type FilesFailure,
	injectNotificationManager,
	provideFileManager,
	ReadyTransition,
	useInstanceFiles,
	useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import { useAppEvent } from '@/composables/use-app-event'
import { get_full_path } from '@/helpers/instance'
import { highlightInFolder } from '@/helpers/utils'
import { save } from '@/platform/dialog'
import { apiClient } from '@/platform/modrinth-client'
import { saveInstanceFileAs } from '@/platform/ops'

import { injectInstancePage } from '../instance-context'
import { instanceKeys } from '../query-options'

const instancePage = injectInstancePage()
const instanceId = instancePage.instanceId

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()

const messages = defineMessages({
	readOnly: {
		id: 'instance.files.managed-content-read-only',
		defaultMessage: 'Manage your installed content via the Content tab',
	},
	saveAs: {
		id: 'instance.files.save-as',
		defaultMessage: 'Save as...',
	},
	addingFiles: {
		id: 'instance.files.adding-files',
		defaultMessage: 'Adding files ({completed}/{total})',
	},
})

const failureTitles = {
	create: commonMessages.createFailedLabel,
	rename: commonMessages.renameFailedLabel,
	move: commonMessages.moveFailedLabel,
	delete: commonMessages.deleteFailedLabel,
	upload: commonMessages.uploadFailedLabel,
	write: commonMessages.uploadFailedLabel,
	zip: commonMessages.createFailedLabel,
} satisfies Record<FilesFailure, unknown>

// Only joins paths for display and Open in folder, so the listing never waits on it.
const instanceRootQuery = useQuery(
	computed(() => ({
		queryKey: instanceKeys.rootPath(instanceId.value),
		networkMode: 'always' as const,
		queryFn: () => get_full_path(instanceId.value),
		enabled: !!instanceId.value,
		staleTime: Infinity,
	})),
)
const instanceRoot = computed(() => instanceRootQuery.data.value ?? '')

const currentPath = ref('')
const files = useInstanceFiles({
	client: apiClient.local,
	instanceId,
	currentPath,
	queryKey: (path) => instanceKeys.files(instanceId.value, path),
	queryKeyPrefix: () => [...instanceKeys.detail(instanceId.value), 'files'],
	onFailure: (failure, error) =>
		addNotification({
			title: formatMessage(failureTitles[failure]),
			text: error instanceof Error ? error.message : '',
			type: 'error',
		}),
})

// `mods/` and the content the engine manages are hard links into the content store.
function isReadOnly(path: string): boolean {
	const normalized = path.startsWith('/') ? path.slice(1) : path
	return (
		normalized.split('/')[0].toLowerCase() === 'mods' ||
		files.items.value.some((item) => item.path === normalized && item.readOnly === true)
	)
}

async function downloadFile(path: string, fileName: string) {
	const destination = await save({ defaultPath: fileName })
	if (!destination) return
	const { error } = await saveInstanceFileAs({
		op: crypto.randomUUID(),
		instance_id: instanceId.value,
		path,
		destination,
	})
	if (error) throw new Error(error.description)
}

useAppEvent('instance', async (event) => {
	if (event.instance_id === instanceId.value && event.event === 'synced') await files.refresh()
})

watch(instanceId, () => {
	currentPath.value = ''
})

provideFileManager({
	isReadOnly,
	readOnlyReason: computed(() => formatMessage(messages.readOnly)),
	items: files.items,
	loading: files.loading,
	error: files.error,
	currentPath,
	navigateTo: files.navigateTo,
	prefetchDirectory: files.prefetchDirectory,
	editingFile: files.editingFile,
	startEditing: files.startEditing,
	stopEditing: files.stopEditing,
	createItem: files.createItem,
	renameItem: files.renameItem,
	moveItem: files.moveItem,
	deleteItem: files.deleteItem,
	readFile: files.readFile,
	readFileAsBlob: files.readFileAsBlob,
	writeFile: files.writeFile,
	downloadFile,
	uploadFiles: files.uploadFiles,
	cancelUpload: files.cancelUpload,
	uploadState: files.uploadState,
	extractFile: files.extractFile,
	refresh: files.refresh,
	basePath: instanceRoot,
	openInFolder: (path: string) => highlightInFolder(path),
	downloadButtonLabel: formatMessage(messages.saveAs),
	uploadingLabel: (completed: number, total: number) =>
		formatMessage(messages.addingFiles, { completed, total }),
})
</script>

<template>
	<ReadyTransition :pending="files.firstPaintPending.value">
		<div>
			<FilePageLayout :show-refresh-button="true" />
		</div>
	</ReadyTransition>
</template>
