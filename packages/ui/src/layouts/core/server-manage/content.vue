<template>
	<ReadyTransition :pending="loading">
		<ContentPageLayout />
	</ReadyTransition>
</template>

<script setup lang="ts">
import type { CoreMod } from '@amberite/amberite-api'
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import { useVIntl } from '#ui/composables/i18n'
import {
	type ContentItem,
	ContentPageLayout,
	provideContentManager,
} from '#ui/layouts/shared/content-tab'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

const router = useRouter()
const route = useRoute()
const coreClient = injectCoreClient()
const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { serverId, isSyncingContent, busyReasons } = injectModrinthServerContext()

const loading = ref(true)
const error = ref<Error | null>(null)
const mods = ref<CoreMod[]>([])

const isBusy = computed(() => isSyncingContent.value || busyReasons.value.length > 0)
const busyMessage = computed(() => (busyReasons.value[0] ? 'Your server is busy.' : null))
const items = computed<ContentItem[]>(() => mods.value.map(mapCoreModToContentItem))
const modrinthProjectIds = computed(
	() => new Set(mods.value.map((mod) => mod.modrinth_project_id).filter(Boolean)),
)
const modrinthVersionIds = computed(
	() => new Set(mods.value.map((mod) => mod.modrinth_version_id).filter(Boolean)),
)

function mapCoreModToContentItem(mod: CoreMod): ContentItem {
	const title = mod.display_name ?? mod.filename.replace(/\.disabled$/, '')
	return {
		id: mod.filename,
		file_name: mod.filename,
		file_path: `mods/${mod.filename}`,
		size: undefined,
		project_type: 'mod',
		has_update: mod.update_available ?? false,
		update_version_id: null,
		environment:
			mod.client_side === 'required' && mod.server_side !== 'required' ? 'client' : 'server',
		enabled: mod.enabled,
		project: {
			id: mod.modrinth_project_id ?? mod.id ?? mod.filename,
			slug: mod.modrinth_project_id ?? undefined,
			title,
			icon_url: null,
		},
		version: {
			id: mod.modrinth_version_id ?? mod.id ?? mod.filename,
			version_number: mod.version_number ?? formatMessage(commonMessages.unknownLabel),
			file_name: mod.filename,
		},
	}
}

async function refresh() {
	loading.value = true
	error.value = null
	try {
		mods.value = await coreClient.listMods(serverId)
	} catch (err) {
		error.value = err instanceof Error ? err : new Error(String(err))
		mods.value = []
	} finally {
		loading.value = false
	}
}

function browse() {
	router.push({
		path: '/browse/mod',
		query: {
			cid: serverId,
			source: 'core',
			back: route.fullPath,
		},
	})
}

function uploadFiles() {
	const input = document.createElement('input')
	input.type = 'file'
	input.multiple = true
	input.accept = '.jar'
	input.onchange = async () => {
		if (!input.files) return
		for (const file of Array.from(input.files)) {
			try {
				await coreClient.uploadModFile(serverId, file).done
			} catch (err) {
				addNotification({
					type: 'error',
					title: 'Failed to upload content',
					text: err instanceof Error ? err.message : String(err),
				})
			}
		}
		await refresh()
	}
	input.click()
}

async function toggleEnabled(item: ContentItem) {
	await coreClient.toggleMod(serverId, item.file_name, !item.enabled)
	await refresh()
}

async function deleteItem(item: ContentItem) {
	await coreClient.deleteMod(serverId, item.file_name)
	await refresh()
}

async function updateItem(id: string) {
	const item = items.value.find((candidate) => candidate.id === id)
	if (!item) return
	const updated = await coreClient.updateMod(serverId, item.file_name)
	if (!updated) {
		addNotification({
			type: 'info',
			title: 'Already up to date',
			text: `${item.file_name} is already on the latest version.`,
		})
	}
	await refresh()
}

async function bulkUpdateItems(targetItems: ContentItem[]) {
	if (targetItems.length === items.value.length) {
		await coreClient.updateAllMods(serverId)
		await refresh()
		return
	}
	await Promise.all(targetItems.map((item) => coreClient.updateMod(serverId, item.file_name)))
	await refresh()
}

function isTrackedProject(item: ContentItem) {
	return !!item.project?.id && modrinthProjectIds.value.has(item.project.id)
}

function isTrackedVersion(item: ContentItem) {
	return !!item.version?.id && modrinthVersionIds.value.has(item.version.id)
}

async function switchVersion(item: ContentItem) {
	if (!isTrackedProject(item) || !isTrackedVersion(item)) return
	await router.push(`/project/${item.project.id}/version/${item.version.id}`)
}

provideContentManager({
	items,
	loading,
	error,
	modpack: ref(null),
	isPackLocked: ref(false),
	isBusy,
	busyMessage,
	contentTypeLabel: ref('project'),
	toggleEnabled,
	deleteItem,
	bulkDeleteItems: (targetItems) => Promise.all(targetItems.map(deleteItem)).then(() => {}),
	bulkEnableItems: (targetItems) =>
		Promise.all(targetItems.filter((item) => !item.enabled).map(toggleEnabled)).then(() => {}),
	bulkDisableItems: (targetItems) =>
		Promise.all(targetItems.filter((item) => item.enabled).map(toggleEnabled)).then(() => {}),
	refresh,
	browse,
	uploadFiles,
	getItemId: (item) => item.file_name,
	hasUpdateSupport: true,
	updateItem: (id) => void updateItem(id),
	bulkUpdateItem: (item) => coreClient.updateMod(serverId, item.file_name).then(() => {}),
	bulkUpdateItems,
	switchVersion,
	shareItems: (targetItems, format) => {
		const text = targetItems
			.map((item) =>
				format === 'file-names' ? item.file_name : (item.project?.title ?? item.file_name),
			)
			.join('\n')
		void navigator.clipboard.writeText(text)
	},
	deletionContext: 'server',
	mapToTableItem: (item) => ({
		id: item.id,
		project: item.project ?? {
			id: item.file_name,
			slug: null,
			title: item.file_name,
			icon_url: null,
		},
		projectLink: isTrackedProject(item) ? { path: `/project/${item.project!.id}` } : undefined,
		version: item.version ?? {
			id: item.file_name,
			version_number: formatMessage(commonMessages.unknownLabel),
			file_name: item.file_name,
		},
		versionLink:
			isTrackedProject(item) && isTrackedVersion(item)
				? { path: `/project/${item.project!.id}/version/${item.version!.id}` }
				: undefined,
		owner: item.owner,
		enabled: item.enabled,
		hasUpdate: item.has_update,
		clientWarning: item.environment === 'client' ? 'environment' : null,
	}),
	filterPersistKey: `core-server-${serverId}`,
})

await refresh()
</script>
