<template>
	<ContentPageLayout />
</template>

<script setup lang="ts">
import type { CoreMod } from '@amberite/amberite-api'
import type { ContentItem } from '@modrinth/ui'
import { ContentPageLayout, injectNotificationManager, provideContentManager } from '@modrinth/ui'
import { computed, inject, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey, toContentItem } from './server/core-server-instance'

const core = useCoreClient()
const router = useRouter()
const { handleError } = injectNotificationManager()
const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const mods = ref<CoreMod[]>([])
const loading = ref(false)
const error = ref<Error | null>(null)
const items = computed(() => mods.value.map(toContentItem))
const isBusy = computed(
	() => ctx.powerState.value === 'starting' || ctx.powerState.value === 'stopping',
)

async function refresh() {
	loading.value = true
	error.value = null
	try {
		mods.value = await core.listMods(ctx.instanceId.value)
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

await refresh()

provideContentManager({
	items,
	loading,
	error,
	modpack: computed(() => null),
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
	uploadFiles: () => {},
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
