<template>
	<ReadyTransition :pending="firstPaintPending">
		<FilePageLayout />
	</ReadyTransition>
</template>

<script setup lang="ts">
import type { CoreFsEntry, UploadHandle } from '@amberite/amberite-api'
import type { UploadState } from '@modrinth/api-client'
import { computed, ref } from 'vue'

import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import { useVIntl } from '#ui/composables/i18n'
import {
	type EditingFile,
	type FileItem,
	FilePageLayout,
	provideFileManager,
} from '#ui/layouts/shared/files-tab'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

const coreClient = injectCoreClient()
const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()
const ctx = injectModrinthServerContext()

const items = ref<FileItem[]>([])
const firstPaintPending = ref(true)
const loading = ref(true)
const error = ref<Error | null>(null)
const currentPath = ref('/')
const editingFile = ref<EditingFile | null>(null)
const uploadState = ref<UploadState>({
	isUploading: false,
	currentFileName: null,
	currentFileProgress: 0,
	uploadedBytes: 0,
	totalBytes: 0,
	completedFiles: 0,
	totalFiles: 0,
})
const cancelUpload = ref<(() => void) | null>(null)

const isBusy = computed(() => ctx.busyReasons.value.length > 0 || ctx.isSyncingContent.value)
const busyTooltip = computed(() => (isBusy.value ? 'Your server is busy.' : undefined))

function normalizePath(path: string) {
	if (!path || path === '/') return '/'
	return path.startsWith('/') ? path : `/${path}`
}

function childPath(name: string) {
	return currentPath.value === '/' ? name : `${currentPath.value.replace(/^\//, '')}/${name}`
}

function toTimestamp(value: string | null) {
	return value ? Math.floor(new Date(value).getTime() / 1000) : 0
}

function mapEntry(entry: CoreFsEntry): FileItem {
	return {
		name: entry.name,
		type: entry.type,
		path: entry.path.startsWith('/') ? entry.path.slice(1) : entry.path,
		modified: toTimestamp(entry.modified_at),
		created: toTimestamp(entry.modified_at),
		size: entry.size ?? undefined,
	}
}

async function refresh() {
	loading.value = true
	error.value = null
	try {
		const listing = await coreClient.listDirectory(ctx.serverId, currentPath.value, 0, 500)
		items.value = listing.items.map(mapEntry)
	} catch (err) {
		error.value = err instanceof Error ? err : new Error(String(err))
		items.value = []
	} finally {
		loading.value = false
		firstPaintPending.value = false
	}
}

function navigateTo(path: string) {
	currentPath.value = normalizePath(path)
	void refresh()
}

function startEditing(file: EditingFile) {
	editingFile.value = file
}

function stopEditing() {
	editingFile.value = null
}

async function createItem(name: string, type: 'file' | 'directory') {
	const path = childPath(name)
	try {
		if (type === 'directory') await coreClient.createDir(ctx.serverId, path)
		else await coreClient.createFile(ctx.serverId, path)
		await refresh()
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(commonMessages.createFailedLabel),
			text: err instanceof Error ? err.message : String(err),
		})
	}
}

async function renameItem(path: string, newName: string) {
	const normalized = path.replace(/^\//, '')
	const parent = normalized.includes('/') ? normalized.slice(0, normalized.lastIndexOf('/')) : ''
	const destination = parent ? `${parent}/${newName}` : newName
	try {
		await coreClient.moveEntry(ctx.serverId, normalized, destination)
		await refresh()
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(commonMessages.renameFailedLabel),
			text: err instanceof Error ? err.message : String(err),
		})
	}
}

async function moveItem(source: string, destination: string) {
	try {
		await coreClient.moveEntry(
			ctx.serverId,
			source.replace(/^\//, ''),
			destination.replace(/^\//, ''),
		)
		await refresh()
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(commonMessages.moveFailedLabel),
			text: err instanceof Error ? err.message : String(err),
		})
	}
}

async function deleteItem(path: string, recursive: boolean) {
	try {
		await coreClient.deleteFileOrFolder(ctx.serverId, path.replace(/^\//, ''), recursive)
		await refresh()
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(commonMessages.deleteFailedLabel),
			text: err instanceof Error ? err.message : String(err),
		})
	}
}

async function readFile(path: string) {
	const data = await coreClient.readFile(ctx.serverId, path.replace(/^\//, ''))
	return new TextDecoder().decode(data)
}

async function readFileAsBlob(path: string) {
	return new Blob([await coreClient.readFile(ctx.serverId, path.replace(/^\//, ''))])
}

async function writeFile(path: string, content: string) {
	await coreClient.writeFile(ctx.serverId, path.replace(/^\//, ''), content)
}

async function downloadFile(path: string, fileName: string) {
	const blob = await coreClient.downloadFile(ctx.serverId, path.replace(/^\//, ''))
	const url = URL.createObjectURL(blob)
	const link = document.createElement('a')
	link.href = url
	link.download = fileName
	document.body.appendChild(link)
	link.click()
	document.body.removeChild(link)
	URL.revokeObjectURL(url)
}

function setUploadProgress(
	handle: UploadHandle,
	file: File,
	completed: number,
	totalFiles: number,
) {
	handle.onProgress((progress) => {
		uploadState.value = {
			...uploadState.value,
			currentFileName: file.name,
			currentFileProgress: progress,
			uploadedBytes: uploadState.value.uploadedBytes,
			completedFiles: completed,
			totalFiles,
		}
	})
}

function uploadFiles(files: File[]) {
	void (async () => {
		uploadState.value = {
			isUploading: true,
			currentFileName: null,
			currentFileProgress: 0,
			uploadedBytes: 0,
			totalBytes: files.reduce((sum, file) => sum + file.size, 0),
			completedFiles: 0,
			totalFiles: files.length,
		}

		try {
			for (const [index, file] of files.entries()) {
				const handle = coreClient.uploadFile(ctx.serverId, currentPath.value, file)
				cancelUpload.value = () => handle.abort()
				setUploadProgress(handle, file, index, files.length)
				await handle.done
				uploadState.value.uploadedBytes += file.size
				uploadState.value.completedFiles = index + 1
			}
			await refresh()
		} catch (err) {
			addNotification({
				type: 'error',
				title: 'Failed to upload files',
				text: err instanceof Error ? err.message : String(err),
			})
		} finally {
			cancelUpload.value = null
			uploadState.value.isUploading = false
		}
	})()
}

async function extractFile(path: string, _override: boolean, dry: boolean) {
	if (dry) return { modpack_name: null, conflicting_files: [] }
	await coreClient.unzipFile(ctx.serverId, path.replace(/^\//, ''))
	await refresh()
}

provideFileManager({
	items,
	loading,
	error,
	currentPath,
	navigateTo,
	editingFile,
	startEditing,
	stopEditing,
	createItem,
	renameItem,
	moveItem,
	deleteItem,
	readFile,
	readFileAsBlob,
	writeFile,
	downloadFile,
	uploadFiles,
	cancelUpload: () => cancelUpload.value?.(),
	uploadState,
	refresh: () => void refresh(),
	isBusy,
	busyTooltip,
	extractFile,
	prefetchDirectory: () => {},
	prefetchFile: () => {},
	canRestart: true,
	restartServer: () => coreClient.restart(ctx.serverId),
})

await refresh()
</script>
