<script setup lang="ts">
import type { CoreFsEntry } from '@amberite/core-client'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import { useReadyState } from '#ui/composables'
import { useVIntl } from '#ui/composables/i18n'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

import FilePageLayout from '../../../shared/files-tab/layout.vue'
import { provideFileManager } from '../../../shared/files-tab/providers/file-manager'
import type { EditingFile, FileItem } from '../../../shared/files-tab/types'

const props = defineProps<{
	showDebugInfo?: boolean
	showRefreshButton?: boolean
}>()

const coreClient = injectCoreClient()
const serverContext = injectModrinthServerContext()
const { serverId, busyReasons, uploadState, cancelUpload: cancelUploadRef } = serverContext
const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()

const route = useRoute()
const router = useRouter()
const queryClient = useQueryClient()

const serverBusy = computed(() => busyReasons.value.length > 0)
const busyTooltip = computed(() =>
	busyReasons.value.length > 0 ? formatMessage(busyReasons.value[0].reason) : undefined,
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

// Path & navigation
const currentPath = computed(() => (typeof route.query.path === 'string' ? route.query.path : '/'))

function navigateTo(path: string) {
	const { editing: _, ...query } = route.query
	router.push({ query: { ...query, path } })
}

// Editing state (synced with URL)
const editingFile = ref<EditingFile | null>(null)

function startEditing(file: EditingFile) {
	editingFile.value = file
	router.push({ query: { ...route.query, path: currentPath.value, editing: file.path } })
}

function stopEditing() {
	editingFile.value = null
	const newQuery = { ...route.query }
	delete newQuery.editing
	router.replace({ query: newQuery })
}

// Sync editing state from URL
watch(
	() => route.query,
	(newQuery, oldQuery) => {
		if (newQuery.editing && editingFile.value?.path !== newQuery.editing) {
			editingFile.value = {
				name: (newQuery.editing as string).split('/').pop() || '',
				path: newQuery.editing as string,
			}
		} else if (oldQuery?.editing && !newQuery.editing) {
			editingFile.value = null
		}
	},
	{ deep: true },
)

// Initialize editing from URL on mount
function initializeFileEdit() {
	if (!route.query.editing) return
	const filePath = route.query.editing as string
	editingFile.value = {
		name: filePath.split('/').pop() || '',
		path: filePath,
	}
}

function mapCoreEntryToFileItem(entry: CoreFsEntry): FileItem {
	return {
		name: entry.name,
		path: entry.path,
		type: entry.type === 'directory' ? 'directory' : 'file',
		modified: entry.modified_at ? new Date(entry.modified_at).getTime() / 1000 : 0,
		created: entry.modified_at ? new Date(entry.modified_at).getTime() / 1000 : 0,
		size: entry.size ?? undefined,
	}
}

// Directory listing query
const {
	data: directoryData,
	isLoading,
	error: loadError,
} = useQuery({
	queryKey: computed(() => ['files', serverId, currentPath.value]),
	queryFn: async () => {
		return coreClient.listDirectory(serverId, currentPath.value, 0, 2000)
	},
	staleTime: 30_000,
})

const items = computed<FileItem[]>(() => directoryData.value?.items.map(mapCoreEntryToFileItem) ?? [])

const filesReadyPending = useReadyState({ isLoading, data: directoryData })

// Prefetching
function prefetchDirectory(path: string) {
	queryClient.prefetchQuery({
		queryKey: ['files', serverId, path],
		queryFn: async () => {
			try {
				return await coreClient.listDirectory(serverId, path, 0, 2000)
			} catch {
				return { items: [], total: 0, current: 0 }
			}
		},
		staleTime: 30_000,
	})
}

function prefetchFile(path: string) {
	queryClient.prefetchQuery({
		queryKey: ['file-content', serverId, path],
		queryFn: async () => {
			try {
				const blob = await coreClient.downloadFile(serverId, path)
				return await blob.text()
			} catch {
				return null
			}
		},
		staleTime: 30_000,
	})
}

function getQueryKey() {
	return ['files', serverId, currentPath.value]
}

function refreshList() {
	queryClient.invalidateQueries({ queryKey: ['files', serverId] })
}

// Mutations
const deleteMutation = useMutation({
	mutationFn: ({ path, recursive }: { path: string; recursive: boolean }) =>
		coreClient.deleteFileOrFolder(serverId, path, recursive),
	onMutate: async ({ path }) => {
		const queryKey = getQueryKey()
		await queryClient.cancelQueries({ queryKey })
		const previous = queryClient.getQueryData(queryKey)
		queryClient.setQueryData(queryKey, (old: { items: CoreFsEntry[] } | undefined) => {
			if (!old) return old
			return { ...old, items: old.items.filter((item) => item.path !== path) }
		})
		return { previous }
	},
	onError: (err: Error, _vars, context) => {
		queryClient.setQueryData(getQueryKey(), context?.previous)
		addNotification({
			title: formatMessage(commonMessages.deleteFailedLabel),
			text: err.message,
			type: 'error',
		})
	},
	onSuccess: () => {
		addNotification({
			title: 'File deleted',
			text: 'Your file has been deleted.',
			type: 'success',
		})
	},
	onSettled: () => {
		queryClient.invalidateQueries({ queryKey: ['files', serverId] })
	},
})

// Unsupported operations — Core does not yet expose rename, move, create, or extract endpoints.
function unsupported(name: string) {
	addNotification({
		title: 'Not supported',
		text: `${name} is not yet supported by Amberite Core.`,
		type: 'error',
	})
}

const renameMutation = useMutation({
	mutationFn: (_args: { path: string; newName: string }) => {
		unsupported('Renaming')
		return Promise.resolve()
	},
})

const moveMutation = useMutation({
	mutationFn: (_args: { source: string; destination: string }) => {
		unsupported('Moving')
		return Promise.resolve()
	},
})

const createMutation = useMutation({
	mutationFn: (_args: { path: string; type: 'file' | 'directory' }) => {
		unsupported(`Creating ${_args.type}s`)
		return Promise.resolve()
	},
})

async function extractFile(_path: string, _override: boolean, _dry: boolean) {
	unsupported('Archive extraction')
}

// File I/O
async function readFile(path: string): Promise<string> {
	const normalizedPath = path.startsWith('/') ? path : `/${path}`
	const cachedContent = queryClient.getQueryData<string>(['file-content', serverId, normalizedPath])
	if (cachedContent) return cachedContent
	const blob = await coreClient.downloadFile(serverId, normalizedPath)
	return await blob.text()
}

async function readFileAsBlob(path: string): Promise<Blob> {
	const normalizedPath = path.startsWith('/') ? path : `/${path}`
	return await coreClient.downloadFile(serverId, normalizedPath)
}

async function writeFile(_path: string, _content: string): Promise<void> {
	unsupported('Editing files')
}

async function downloadFile(path: string, fileName: string): Promise<void> {
	try {
		const blob = await coreClient.downloadFile(serverId, path)
		const link = document.createElement('a')
		link.href = window.URL.createObjectURL(blob)
		link.download = fileName
		link.click()
		window.URL.revokeObjectURL(link.href)
	} catch {
		addNotification({
			title: formatMessage(commonMessages.downloadFailedLabel),
			text: 'Could not download the file.',
			type: 'error',
		})
	}
}

watch(
	() => serverContext.fsOps.value,
	() => {
		refreshList()
	},
)

onMounted(async () => {
	initializeFileEdit()
})

// Restart
async function restartServer() {
	await coreClient.restart(serverId)
}

let activeUploadCancel: (() => void) | null = null

async function uploadFiles(files: File[]) {
	if (files.length === 0) return

	const totalBytes = files.reduce((sum, f) => sum + f.size, 0)
	uploadState.value = {
		isUploading: true,
		currentFileName: files[0].name,
		currentFileProgress: 0,
		uploadedBytes: 0,
		totalBytes,
		completedFiles: 0,
		totalFiles: files.length,
	}
	cancelUploadRef.value = () => activeUploadCancel?.()

	let completedBytes = 0

	for (let i = 0; i < files.length; i++) {
		const file = files[i]
		const targetDir = currentPath.value

		uploadState.value.currentFileName = file.name
		uploadState.value.currentFileProgress = 0

		try {
			const handle = coreClient.uploadFile(serverId, targetDir, file)
			handle.onProgress((pct) => {
				uploadState.value.currentFileProgress = pct / 100
				uploadState.value.uploadedBytes = completedBytes + Math.round(file.size * (pct / 100))
			})
			activeUploadCancel = () => handle.abort()

			await handle.done
			completedBytes += file.size
			uploadState.value.completedFiles = i + 1
			uploadState.value.uploadedBytes = completedBytes
		} catch (err) {
			if (err instanceof Error && err.message === 'Upload cancelled') break
			addNotification({
				title: formatMessage(commonMessages.uploadFailedLabel),
				text: `Failed to upload ${file.name}`,
				type: 'error',
			})
		}
	}

	activeUploadCancel = null
	cancelUploadRef.value = null
	refreshList()
	uploadState.value = {
		isUploading: false,
		currentFileName: null,
		currentFileProgress: 0,
		uploadedBytes: 0,
		totalBytes: 0,
		completedFiles: 0,
		totalFiles: 0,
	}
}

function cancelUpload() {
	activeUploadCancel?.()
}

// Provide the file manager context
provideFileManager({
	items,
	loading: computed(() => isLoading.value),
	error: computed(() => loadError.value ?? null),
	currentPath,
	navigateTo,
	editingFile,
	startEditing,
	stopEditing,
	createItem: async (name, type) => {
		const path = `${currentPath.value}/${name}`.replace('//', '/')
		await createMutation.mutateAsync({ path, type })
	},
	renameItem: async (path, newName) => {
		await renameMutation.mutateAsync({ path, newName })
	},
	moveItem: async (source, destination) => {
		await moveMutation.mutateAsync({ source, destination })
	},
	deleteItem: async (path, recursive) => {
		await deleteMutation.mutateAsync({ path, recursive })
	},
	readFile,
	readFileAsBlob,
	writeFile,
	downloadFile,
	uploadFiles,
	cancelUpload,
	uploadState,
	refresh: refreshList,
	isBusy: serverBusy,
	busyTooltip,
	busyWarning,
	extractFile,
	prefetchDirectory,
	prefetchFile,
	showInstallFromUrl: false,
	canRestart: true,
	restartServer,
	canShareToMclogs: true,
})
</script>

<template>
	<ReadyTransition :pending="filesReadyPending">
		<FilePageLayout
			:show-debug-info="props.showDebugInfo"
			:show-refresh-button="props.showRefreshButton"
		/>
	</ReadyTransition>
</template>
