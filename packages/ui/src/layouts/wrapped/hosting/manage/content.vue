<template>
	<ContentPageLayout />
</template>

<script setup lang="ts">
import type { ContentItem } from '#ui/layouts/shared/content-tab/types'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { injectHostingBackend, injectNotificationManager } from '#ui/providers'
import ContentPageLayout from '#ui/layouts/shared/content-tab/layout.vue'
import type { ContentModpackData } from '#ui/layouts/shared/content-tab/providers/content-manager'
import { provideContentManager } from '#ui/layouts/shared/content-tab/providers/content-manager'

import { injectCoreServerContext } from './context'
import { toContentItem } from './mappers'

const props = defineProps<{
	ownerAvatarUrlBase?: string
	browsePath?: string
}>()

const backend = injectHostingBackend()
const core = backend.core
const { addNotification, handleError } = injectNotificationManager()
const ctx = injectCoreServerContext()
const router = useRouter()
const route = useRoute()

const modsQuery = useQuery({
	queryKey: computed(() => ['core-mods', ctx.instanceId.value]),
	queryFn: () => core.listMods(ctx.instanceId.value),
	staleTime: 30_000,
})

const modpackQuery = useQuery({
	queryKey: computed(() => ['core-modpack', ctx.instanceId.value]),
	queryFn: async () => {
		try {
			return await core.getModpack(ctx.instanceId.value)
		} catch {
			return null
		}
	},
	staleTime: 30_000,
	retry: false,
})

const mods = computed(() => modsQuery.data.value ?? [])
const rawModpack = computed(() => modpackQuery.data.value ?? null)
const loading = computed(() => modsQuery.isLoading.value)
const error = computed(() => (modsQuery.error.value as Error | null) ?? null)
const installedProjectIds = computed(() =>
	mods.value
		.map((mod) => mod.modrinth_project_id)
		.filter((id): id is string => typeof id === 'string' && id.length > 0),
)

const iconsQuery = useQuery({
	queryKey: computed(() => ['modrinth-project-icons', installedProjectIds.value]),
	queryFn: async () => {
		const ids = installedProjectIds.value
		if (!ids.length) return {} as Record<string, string>
		const res = await fetch(
			`https://api.modrinth.com/v2/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`,
		)
		if (!res.ok) return {} as Record<string, string>
		const projects = (await res.json()) as Array<{ id: string; icon_url: string | null }>
		const map: Record<string, string> = {}
		for (const project of projects) {
			if (project.icon_url) map[project.id] = project.icon_url
		}
		return map
	},
	staleTime: 5 * 60_000,
	enabled: computed(() => installedProjectIds.value.length > 0),
})

const iconMap = computed(() => iconsQuery.data.value ?? {})
const items = computed(() =>
	mods.value.map((mod) => {
		const item = toContentItem(mod)
		if (item.project && mod.modrinth_project_id && iconMap.value[mod.modrinth_project_id]) {
			item.project = { ...item.project, icon_url: iconMap.value[mod.modrinth_project_id] }
		}
		return item
	}),
)
const isBusy = computed(
	() => ctx.powerState.value === 'starting' || ctx.powerState.value === 'stopping',
)
const browsePath = computed(() => props.browsePath ?? route.path.replace(/\/content\/?$/, '/browse'))
const modpack = computed<ContentModpackData | null>(() => {
	const pack = rawModpack.value
	if (!pack) return null
	return {
		project: {
			id: pack.modrinth_project_id ?? pack.id,
			slug: pack.modrinth_project_id ?? pack.id,
			title: pack.pack_name,
			icon_url: null,
			description: '',
		},
		projectLink: pack.modrinth_project_id ? `/project/${pack.modrinth_project_id}` : undefined,
		version: {
			id: pack.modrinth_version_id ?? pack.id,
			version_number: pack.pack_version,
			date_published: pack.installed_at,
		},
		categories: [],
		hasUpdate: false,
	}
})

async function refresh() {
	await Promise.all([modsQuery.refetch(), modpackQuery.refetch()])
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
		try {
			for (const file of files) await core.uploadModFile(ctx.instanceId.value, file).done
			addNotification({
				title: 'Upload complete',
				text: `Added ${files.length} file(s) to the server.`,
				type: 'success',
			})
			await refresh()
		} catch (err) {
			handleError(err as Error)
		}
	}
	input.click()
}

async function unlinkModpack() {
	await core.removeModpack(ctx.instanceId.value).catch(handleError)
	await refresh()
}

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
		void router.push(browsePath.value)
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
