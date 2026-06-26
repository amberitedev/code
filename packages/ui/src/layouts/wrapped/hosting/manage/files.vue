<template>
	<FilePageLayout :show-debug-info="showDebugInfo" :show-refresh-button="showRefreshButton" />
</template>

<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { injectHostingBackend, injectNotificationManager } from '#ui/providers'
import FilePageLayout from '#ui/layouts/shared/files-tab/layout.vue'
import { provideFileManager } from '#ui/layouts/shared/files-tab/providers/file-manager'
import type { EditingFile, UploadState } from '#ui/layouts/shared/files-tab/types'

import { injectCoreServerContext } from './context'
import { toFileItem } from './mappers'

withDefaults(
	defineProps<{
		showDebugInfo?: boolean
		showRefreshButton?: boolean
	}>(),
	{
		showDebugInfo: false,
		showRefreshButton: true,
	},
)

const backend = injectHostingBackend()
const core = backend.core
const route = useRoute()
const router = useRouter()
const { handleError } = injectNotificationManager()
const ctx = injectCoreServerContext()
const editingFile = ref<EditingFile | null>(null)
const currentPath = computed(() => (typeof route.query.path === 'string' ? route.query.path : '/'))
const uploadState = ref<UploadState>({
	isUploading: false,
	currentFileName: null,
	currentFileProgress: 0,
	uploadedBytes: 0,
	totalBytes: 0,
	completedFiles: 0,
	totalFiles: 0,
})

const filesQuery = useQuery({
	queryKey: computed(() => ['core-files', ctx.instanceId.value, currentPath.value]),
	queryFn: () =>
		core
			.listDirectory(ctx.instanceId.value, currentPath.value, 0, 2000)
			.then((listing) => listing.items.map(toFileItem)),
	staleTime: 15_000,
})

const items = computed(() => filesQuery.data.value ?? [])
const loading = computed(() => filesQuery.isLoading.value)
const error = computed(() => (filesQuery.error.value as Error | null) ?? null)

function navigateTo(path: string) {
	const query = { ...route.query }
	delete query.editing
	void router.push({ query: { ...query, path } })
}

async function refresh() {
	await filesQuery.refetch()
}

function childPath(name: string) {
	return currentPath.value === '/' ? `/${name}` : `${currentPath.value}/${name}`
}

async function readFile(path: string) {
	const bytes = await core.readFile(ctx.instanceId.value, path)
	return new TextDecoder().decode(bytes)
}

async function readFileAsBlob(path: string) {
	return await core.downloadFile(ctx.instanceId.value, path)
}

async function downloadFile(path: string, fileName: string) {
	const blob = await readFileAsBlob(path)
	const url = URL.createObjectURL(blob)
	const anchor = document.createElement('a')
	anchor.href = url
	anchor.download = fileName
	document.body.appendChild(anchor)
	anchor.click()
	document.body.removeChild(anchor)
	URL.revokeObjectURL(url)
}

async function uploadFiles(files: File[]) {
	if (files.length === 0) return
	uploadState.value = { ...uploadState.value, isUploading: true, totalFiles: files.length }
	try {
		for (const file of files) {
			uploadState.value.currentFileName = file.name
			const handle = core.uploadFile(ctx.instanceId.value, currentPath.value, file)
			handle.onProgress((progress) => {
				uploadState.value.currentFileProgress = progress
			})
			await handle.done
			uploadState.value.completedFiles += 1
		}
		await refresh()
	} catch (err) {
		handleError(err as Error)
	} finally {
		uploadState.value = {
			...uploadState.value,
			isUploading: false,
			currentFileName: null,
			currentFileProgress: 0,
		}
	}
}

provideFileManager({
	items,
	loading,
	error,
	currentPath,
	navigateTo,
	editingFile,
	startEditing: (file) => {
		editingFile.value = file
		void router.push({ query: { ...route.query, path: currentPath.value, editing: file.path } })
	},
	stopEditing: () => {
		editingFile.value = null
		const query = { ...route.query }
		delete query.editing
		void router.replace({ query })
	},
	createItem: async (name, type) => {
		if (type === 'directory') await core.createDir(ctx.instanceId.value, childPath(name))
		else await core.createFile(ctx.instanceId.value, childPath(name))
		await refresh()
	},
	renameItem: async (path, newName) => {
		const parent = path.split('/').slice(0, -1).join('/') || '/'
		const to = parent === '/' ? `/${newName}` : `${parent}/${newName}`
		await core.moveEntry(ctx.instanceId.value, path, to)
		await refresh()
	},
	moveItem: async (source, destination) => {
		await core.moveEntry(ctx.instanceId.value, source, destination)
		await refresh()
	},
	deleteItem: async (path, recursive) => {
		await core.deleteFileOrFolder(ctx.instanceId.value, path, recursive)
		await refresh()
	},
	readFile,
	readFileAsBlob,
	writeFile: (path, content) => core.writeFile(ctx.instanceId.value, path, content).then(() => {}),
	downloadFile,
	uploadFiles,
	uploadState,
	refresh: () => void refresh(),
	isBusy: computed(() => false),
	extractFile: (path, _override, dry) =>
		dry
			? Promise.resolve({ modpack_name: null, conflicting_files: [] })
			: core.unzipFile(ctx.instanceId.value, path).then(() => {}),
	canRestart: true,
	restartServer: ctx.restartServer,
})
</script>
