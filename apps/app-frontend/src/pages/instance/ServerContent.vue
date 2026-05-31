<template>
	<ContentPageLayout />
</template>

<script setup lang="ts">
import type { CoreMod, CoreModpackManifest } from '@amberite/amberite-api'
import type { ContentItem, ContentModpackData } from '@modrinth/ui'
import { ContentPageLayout, injectNotificationManager, provideContentManager } from '@modrinth/ui'
import { computed, inject, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey, toContentItem } from './server/core-server-instance'

const core = useCoreClient()
const router = useRouter()
const { addNotification, handleError } = injectNotificationManager()
const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const mods = ref<CoreMod[]>([])
const rawModpack = ref<CoreModpackManifest | null>(null)
const loading = ref(false)
const error = ref<Error | null>(null)
const items = computed(() => mods.value.map(toContentItem))
const isBusy = computed(
	() => ctx.powerState.value === 'starting' || ctx.powerState.value === 'stopping',
)

const modpack = computed<ContentModpackData | null>(() => {
	const mp = rawModpack.value
	if (!mp) return null
	return {
		project: {
			id: mp.modrinth_project_id ?? mp.id,
			slug: mp.modrinth_project_id ?? mp.id,
			title: mp.pack_name,
			icon_url: null,
			description: '',
		},
		projectLink: mp.modrinth_project_id ? `/project/${mp.modrinth_project_id}` : undefined,
		version: {
			id: mp.modrinth_version_id ?? mp.id,
			version_number: mp.pack_version,
			date_published: mp.installed_at,
		},
		categories: [],
		hasUpdate: false,
	}
})

async function refresh() {
	loading.value = true
	error.value = null
	try {
		mods.value = await core.listMods(ctx.instanceId.value)
		rawModpack.value = await core.getModpack(ctx.instanceId.value)
	} catch (err) {
		error.value = err as Error
		handleError(err as Error)
	} finally {
		loading.value = false
	}
}

function getFilename(item: ContentItem) {
	return item.file_name.replace(/\.disabled$/, '')
}

async function toggleEnabled(item: ContentItem) {
	await core.toggleMod(ctx.instanceId.value, getFilename(item), !item.enabled).catch(handleError)
	await refresh()
}

async function deleteItem(item: ContentItem) {
	await core.deleteMod(ctx.instanceId.value, getFilename(item)).catch(handleError)
	await refresh()
}

async function updateItem(item: ContentItem) {
	await core.updateMod(ctx.instanceId.value, getFilename(item)).catch(handleError)
	await refresh()
}

function uploadFiles() {
	const input = document.createElement('input')
	input.type = 'file'
	input.multiple = true
	input.accept = '.jar'
	input.onchange = async () => {
		if (!input.files?.length) return
		const files = Array.from(input.files)
		console.log('[server-instance] Uploading', files.length, 'mod file(s)')
		try {
			for (const file of files) {
				await core.uploadModFile(ctx.instanceId.value, file).done
			}
			addNotification({
				title: 'Upload complete',
				text: `Added ${files.length} file(s) to the server.`,
				type: 'success',
			})
			await refresh()
		} catch (err) {
			console.error('[server-instance] Mod upload failed:', err)
			handleError(err as Error)
		}
	}
	input.click()
}

async function unlinkModpack() {
	try {
		await core.removeModpack(ctx.instanceId.value)
		addNotification({
			title: 'Modpack removed',
			text: 'The modpack was unlinked.',
			type: 'success',
		})
		await refresh()
	} catch (err) {
		console.error('[server-instance] Remove modpack failed:', err)
		handleError(err as Error)
	}
}

await refresh()

provideContentManager({
	items,
	loading,
	error,
	modpack,
	isPackLocked: computed(() => false),
	isBusy,
	contentTypeLabel: ref('content'),
	toggleEnabled,
	deleteItem,
	bulkDeleteItems: (selected) => Promise.all(selected.map(deleteItem)).then(() => {}),
	bulkEnableItems: (selected) =>
		Promise.all(selected.filter((item) => !item.enabled).map(toggleEnabled)).then(() => {}),
	bulkDisableItems: (selected) =>
		Promise.all(selected.filter((item) => item.enabled).map(toggleEnabled)).then(() => {}),
	refresh,
	browse: () => {
		void router.push({ path: '/browse/mod', query: { i: ctx.instanceId.value } })
	},
	uploadFiles,
	unlinkModpack,
	hasUpdateSupport: true,
	bulkUpdateItem: updateItem,
	deletionContext: 'server',
	getItemId: (item) => item.file_path ?? item.file_name,
	mapToTableItem: (item) => ({
		id: item.file_path ?? item.file_name,
		project: item.project ?? {
			id: item.file_name,
			slug: item.file_name,
			title: item.file_name.replace('.disabled', ''),
			icon_url: null,
		},
		projectLink: item.project?.id ? `/project/${item.project.id}` : undefined,
		version: item.version,
		owner: item.owner,
		enabled: item.enabled,
		hasUpdate: item.has_update,
	}),
	filterPersistKey: `core-server-${ctx.instanceId.value}`,
})
</script>
