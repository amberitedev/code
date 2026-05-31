<template>
	<FilePageLayout show-refresh-button />
</template>

<script setup lang="ts">
import type { UploadState } from '@modrinth/api-client'
import { FilePageLayout, injectNotificationManager, provideFileManager } from '@modrinth/ui'
import { computed, inject, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey, toFileItem } from './server/core-server-instance'

const core = useCoreClient()
const route = useRoute()
const router = useRouter()
const { handleError } = injectNotificationManager()
const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const items = ref([])
const loading = ref(false)
const error = ref<Error | null>(null)
const editingFile = ref(null)
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

function navigateTo(path: string) {
	const { editing: _, ...query } = route.query
	void router.push({ query: { ...query, path } })
}

async function refresh() {
	loading.value = true
	error.value = null
	try {
		const listing = await core.listDirectory(ctx.instanceId.value, currentPath.value, 0, 2000)
		items.value = listing.items.map(toFileItem)
	} catch (err) {
		error.value = err as Error
		handleError(err as Error)
	} finally {
		loading.value = false
	}
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
	const a = document.createElement('a')
	a.href = url
	a.download = fileName
	document.body.appendChild(a)
	a.click()
	document.body.removeChild(a)
	URL.revokeObjectURL(url)
}

async function uploadFiles(files: File[]) {
	if (files.length === 0) return
	uploadState.value = {
		...uploadState.value,
		isUploading: true,
		totalFiles: files.length,
		completedFiles: 0,
	}
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

watch(currentPath, () => void refresh(), { immediate: true })

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
	writeFile: (path, content) => core.writeFile(ctx.instanceId.value, path, content),
	downloadFile,
	uploadFiles,
	uploadState,
	refresh: () => void refresh(),
	isBusy: computed(() => false),
	extractFile: (path, _override, dry) =>
		dry
			? Promise.resolve({ modpack_name: null, conflicting_files: [] })
			: core.unzipFile(ctx.instanceId.value, path),
	canRestart: true,
	restartServer: ctx.restartServer,
})
</script>
