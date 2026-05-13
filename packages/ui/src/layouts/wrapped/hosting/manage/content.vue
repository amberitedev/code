<script setup lang="ts">
import type { CoreMod } from '@amberite/api-lib'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import { useReadyState } from '#ui/composables'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
	injectServerSettingsModal,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

import ConfirmModpackUpdateModal from '../../../shared/content-tab/components/modals/ConfirmModpackUpdateModal.vue'
import ConfirmUnlinkModal from '../../../shared/content-tab/components/modals/ConfirmUnlinkModal.vue'
import ContentUpdaterModal from '../../../shared/content-tab/components/modals/ContentUpdaterModal.vue'
import ModpackContentModal from '../../../shared/content-tab/components/modals/ModpackContentModal.vue'
import ContentPageLayout from '../../../shared/content-tab/layout.vue'
import type { ContentModpackData } from '../../../shared/content-tab/providers/content-manager'
import { provideContentManager } from '../../../shared/content-tab/providers/content-manager'
import type { ContentItem } from '../../../shared/content-tab/types'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	failedToRemoveContent: {
		id: 'hosting.content.failed-to-remove',
		defaultMessage: 'Failed to remove content',
	},
	failedToToggle: {
		id: 'hosting.content.failed-to-toggle',
		defaultMessage: 'Failed to toggle {name}',
	},
	failedToUpload: {
		id: 'hosting.content.failed-to-upload',
		defaultMessage: 'Failed to upload file',
	},
	failedToUpdate: {
		id: 'hosting.content.failed-to-update',
		defaultMessage: 'Failed to update',
	},
	failedToBulkDelete: {
		id: 'hosting.content.failed-to-bulk-delete',
		defaultMessage: 'Failed to delete content',
	},
	failedToBulkEnable: {
		id: 'hosting.content.failed-to-bulk-enable',
		defaultMessage: 'Failed to enable content',
	},
	failedToBulkDisable: {
		id: 'hosting.content.failed-to-bulk-disable',
		defaultMessage: 'Failed to disable content',
	},
	failedToBulkUpdate: {
		id: 'hosting.content.failed-to-bulk-update',
		defaultMessage: 'Failed to update content',
	},
})

const coreClient = injectCoreClient()
const { server, worldId, busyReasons, isSyncingContent, uploadState, cancelUpload, serverId } =
	injectModrinthServerContext()
const { addNotification } = injectNotificationManager()
const { openServerSettings, browseServerContent } = injectServerSettingsModal()
const router = useRouter()
const queryClient = useQueryClient()

const type = computed(() => {
	const loader = server.value?.loader?.toLowerCase()
	if (loader === 'paper' || loader === 'purpur') return 'plugin'
	if (loader === 'vanilla') return 'datapack'
	return 'mod'
})

const queryKey = computed(() => ['mods', serverId])

const modsQuery = useQuery({
	queryKey,
	queryFn: () => coreClient.listMods(serverId),
	enabled: computed(() => !!serverId),
	staleTime: 0,
	retry: false,
	refetchOnWindowFocus: false,
})

const contentReadyPending = useReadyState(modsQuery)

function friendlyModName(mod: CoreMod): string {
	return mod.display_name ?? mod.filename.replace(/\.jar(\.disabled)?$/, '')
}

const contentItems = computed<ContentItem[]>(() => {
	return (modsQuery.data.value ?? []).map((mod) => ({
		project: {
			id: mod.modrinth_project_id ?? mod.filename,
			slug: mod.modrinth_project_id ?? mod.filename,
			title: friendlyModName(mod),
			icon_url: undefined,
			description: '',
			downloads: undefined,
			followers: undefined,
		},
		version: {
			id: mod.version_number ?? mod.filename,
			version_number: mod.version_number ?? formatMessage(commonMessages.unknownLabel),
			file_name: mod.filename,
		},
		owner: undefined,
		id: mod.id ?? mod.filename,
		enabled: mod.enabled,
		file_name: mod.filename,
		project_type: type.value,
		has_update: mod.update_available ?? false,
		update_version_id: null,
		environment: undefined,
		pack_client_retained: false,
		pack_client_depends: false,
		installing: false,
	}))
})

const deleteMutation = useMutation({
	mutationFn: ({ filename }: { filename: string }) => coreClient.deleteMod(serverId, filename),
	onMutate: async ({ filename }) => {
		await queryClient.cancelQueries({ queryKey: queryKey.value })
		const previousData = queryClient.getQueryData<CoreMod[]>(queryKey.value)
		queryClient.setQueryData(queryKey.value, (oldData: CoreMod[] | undefined) => {
			if (!oldData) return oldData
			return oldData.filter((m) => m.filename !== filename)
		})
		return { previousData }
	},
	onSuccess: () => {
		queryClient.invalidateQueries({ queryKey: queryKey.value })
	},
	onError: (err, _vars, context) => {
		if (context?.previousData) {
			queryClient.setQueryData(queryKey.value, context.previousData)
		}
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToRemoveContent),
			text: err instanceof Error ? err.message : undefined,
		})
	},
})

const toggleMutation = useMutation({
	mutationFn: async ({ filename, enabled }: { filename: string; enabled: boolean }) => {
		await coreClient.toggleMod(serverId, filename, enabled)
		return { filename, newEnabled: enabled }
	},
	onSuccess: ({ filename, newEnabled }) => {
		queryClient.setQueryData(queryKey.value, (oldData: CoreMod[] | undefined) => {
			if (!oldData) return oldData
			return oldData.map((m) =>
				m.filename === filename ? { ...m, enabled: newEnabled } : m,
			)
		})
		queryClient.invalidateQueries({ queryKey: queryKey.value })
	},
	onError: (_err, { filename }) => {
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToToggle, { name: filename }),
		})
	},
})

async function handleToggleEnabled(item: ContentItem) {
	const mod = (modsQuery.data.value ?? []).find((m) => m.filename === item.file_name)
	if (!mod) return
	await toggleMutation.mutateAsync({ filename: mod.filename, enabled: !mod.enabled })
}

async function handleDeleteItem(item: ContentItem) {
	const mod = (modsQuery.data.value ?? []).find((m) => m.filename === item.file_name)
	if (!mod) return
	await deleteMutation.mutateAsync({ filename: mod.filename })
}

async function handleBulkDelete(items: ContentItem[]) {
	for (const item of items) {
		const mod = (modsQuery.data.value ?? []).find((m) => m.filename === item.file_name)
		if (!mod) continue
		try {
			await coreClient.deleteMod(serverId, mod.filename)
		} catch (err) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToBulkDelete),
				text: err instanceof Error ? err.message : undefined,
			})
			return
		}
	}
	await queryClient.invalidateQueries({ queryKey: queryKey.value })
}

async function handleBulkEnable(items: ContentItem[]) {
	for (const item of items) {
		const mod = (modsQuery.data.value ?? []).find((m) => m.filename === item.file_name)
		if (!mod || mod.enabled) continue
		try {
			await coreClient.toggleMod(serverId, mod.filename, true)
		} catch (err) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToBulkEnable),
				text: err instanceof Error ? err.message : undefined,
			})
			return
		}
	}
	await queryClient.invalidateQueries({ queryKey: queryKey.value })
}

async function handleBulkDisable(items: ContentItem[]) {
	for (const item of items) {
		const mod = (modsQuery.data.value ?? []).find((m) => m.filename === item.file_name)
		if (!mod || !mod.enabled) continue
		try {
			await coreClient.toggleMod(serverId, mod.filename, false)
		} catch (err) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToBulkDisable),
				text: err instanceof Error ? err.message : undefined,
			})
			return
		}
	}
	await queryClient.invalidateQueries({ queryKey: queryKey.value })
}

const modpackUnlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const modpackContentModal = ref<InstanceType<typeof ModpackContentModal>>()
const contentUpdaterModal = ref<InstanceType<typeof ContentUpdaterModal>>()
const modpackUpdateModal = ref<InstanceType<typeof ConfirmModpackUpdateModal>>()

const updatingProject = ref<ContentItem | null>(null)
const updatingModpack = ref(false)
const loadingChangelog = ref(false)

// Core does not support version-selection updates; update directly to latest.
async function handleUpdateItem(_id: string) {
	const item = contentItems.value.find((i) => i.id === _id)
	if (!item?.has_update || !item.file_name) return
	try {
		await coreClient.updateMod(serverId, item.file_name)
		await queryClient.invalidateQueries({ queryKey: queryKey.value })
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToUpdate),
			text: err instanceof Error ? err.message : undefined,
		})
	}
}

async function handleBulkUpdate(items: ContentItem[]) {
	const filenames = items.filter((i) => i.has_update).map((i) => i.file_name)
	if (filenames.length === 0) return
	try {
		await coreClient.updateAllMods(serverId)
		await queryClient.invalidateQueries({ queryKey: queryKey.value })
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToBulkUpdate),
			text: err instanceof Error ? err.message : undefined,
		})
	}
}

function handleBrowseContent() {
	const contentType = type.value
	if (browseServerContent && ['mod', 'plugin', 'datapack'].includes(contentType)) {
		browseServerContent({
			serverId,
			worldId: worldId.value,
			type: contentType as 'mod' | 'plugin' | 'datapack',
		})
		return
	}
	router.push({
		path: `/discover/${type.value}s`,
		query: { sid: serverId, wid: worldId.value },
	})
}

function handleUploadFiles() {
	const input = document.createElement('input')
	input.type = 'file'
	input.multiple = true
	input.accept = type.value === 'datapack' ? '.zip' : '.jar'
	input.onchange = async () => {
		if (!input.files) return
		const files = Array.from(input.files)

		uploadState.value = {
			isUploading: true,
			currentFileName: null,
			currentFileProgress: 0,
			uploadedBytes: 0,
			totalBytes: files.reduce((sum, f) => sum + f.size, 0),
			completedFiles: 0,
			totalFiles: files.length,
		}

		for (const file of files) {
			uploadState.value.currentFileName = file.name
			uploadState.value.currentFileProgress = 0
			const handle = coreClient.uploadModFile(serverId, file)
			cancelUpload.value = () => handle.abort()
			handle.onProgress((pct) => {
				uploadState.value.currentFileProgress = pct / 100
				uploadState.value.uploadedBytes = Math.round(
					(uploadState.value.uploadedBytes ?? 0) + file.size * (pct / 100),
				)
			})
			try {
				await handle.done
				uploadState.value.completedFiles++
			} catch (err) {
				if (err instanceof Error && err.message === 'Upload cancelled') break
				addNotification({
					type: 'error',
					title: formatMessage(messages.failedToUpload),
					text: err instanceof Error ? err.message : undefined,
				})
			}
		}

		cancelUpload.value = null
		uploadState.value = {
			isUploading: false,
			currentFileName: null,
			currentFileProgress: 0,
			uploadedBytes: 0,
			totalBytes: 0,
			completedFiles: 0,
			totalFiles: 0,
		}
		await queryClient.invalidateQueries({ queryKey: queryKey.value })
	}
	input.click()
}

// Modpack is not supported by Core's content API.
const modpack = computed<ContentModpackData | null>(() => null)

function handleViewModpackContent() {}
async function handleModpackContentToggle(_item: ContentItem) {}
async function handleModpackBulkToggle(_items: ContentItem[], _enable: boolean) {}
function handleModpackUnlink() {}
async function handleModpackUnlinkConfirm() {}
function handleModpackUpdate() {}
function handleSwitchVersion(_item: ContentItem) {}
function resetUpdateState() {
	updatingModpack.value = false
	updatingProject.value = null
	loadingChangelog.value = false
}
function handleModalUpdate(_selectedVersion: unknown, _event?: MouseEvent) {}
function handleModpackUpdateConfirm() {}
function handleModpackUpdateCancel() {}
function handleVersionSelect(_version: unknown) {}
function handleVersionHover(_version: unknown) {}

function getOverflowOptions(_item: ContentItem) {
	return []
}

provideContentManager({
	items: contentItems,
	loading: computed(() => modsQuery.isLoading.value),
	error: computed(() => modsQuery.error.value ?? null),
	modpack,
	isPackLocked: ref(false),
	isBusy: computed(() => busyReasons.value.length > 0),
	busyMessage: computed(() => {
		const bannerCoversInstalling = server.value?.status === 'installing' || isSyncingContent.value
		const filteredReasons = busyReasons.value.filter((r) => {
			if (
				bannerCoversInstalling &&
				(r.reason.id === 'servers.busy.installing' ||
					r.reason.id === 'servers.busy.syncing-content')
			)
				return false
			if (
				r.reason.id === 'servers.busy.backup-creating' ||
				r.reason.id === 'servers.busy.backup-restoring'
			)
				return false
			return true
		})
		return filteredReasons.length > 0 ? formatMessage(filteredReasons[0].reason) : null
	}),
	contentTypeLabel: type,
	toggleEnabled: handleToggleEnabled,
	deleteItem: handleDeleteItem,
	bulkDeleteItems: handleBulkDelete,
	bulkEnableItems: handleBulkEnable,
	bulkDisableItems: handleBulkDisable,
	refresh: async () => {
		await modsQuery.refetch()
	},
	browse: handleBrowseContent,
	uploadFiles: handleUploadFiles,
	deletionContext: 'server',
	hasUpdateSupport: true,
	updateItem: handleUpdateItem,
	bulkUpdateItems: handleBulkUpdate,
	updateModpack: handleModpackUpdate,
	viewModpackContent: handleViewModpackContent,
	unlinkModpack: handleModpackUnlink,
	openSettings: () => openServerSettings({ tabId: 'installation' }),
	switchVersion: handleSwitchVersion,
	getOverflowOptions,
	mapToTableItem: (item) => {
		const projectType = item.project_type ?? type.value
		const mod = (modsQuery.data.value ?? []).find((m) => m.filename === item.file_name)
		const hasModrinthProject = !!mod?.modrinth_project_id
		return {
			id: item.id,
			project: item.project,
			projectLink: hasModrinthProject
				? `/${projectType}/${item.project.id}`
				: undefined,
			version: item.version,
			versionLink:
				hasModrinthProject && item.version?.id
					? `/${projectType}/${item.project.id}/version/${item.version.id}`
					: undefined,
			owner: item.owner
				? { ...item.owner, link: `/${item.owner.type}/${item.owner.id}` }
				: undefined,
			enabled: item.enabled,
		}
	},
	filterPersistKey: `server:${serverId}:${worldId.value}`,
})
</script>

<template>
	<ReadyTransition :pending="contentReadyPending">
		<ContentPageLayout :bottom-padding="false">
			<template #modals>
				<ConfirmUnlinkModal ref="modpackUnlinkModal" server @unlink="handleModpackUnlinkConfirm" />
				<ModpackContentModal
					ref="modpackContentModal"
					:modpack-name="modpack?.project.title"
					:modpack-icon-url="modpack?.project.icon_url"
					enable-toggle
					@update:enabled="handleModpackContentToggle"
					@bulk:enable="handleModpackBulkToggle($event, true)"
					@bulk:disable="handleModpackBulkToggle($event, false)"
				/>
				<ContentUpdaterModal
					v-if="updatingProject || updatingModpack"
					ref="contentUpdaterModal"
					:versions="[]"
					:current-game-version="server.value?.mc_version ?? ''"
					:current-loader="server.value?.loader ?? ''"
					:current-version-id="''"
					:is-app="false"
					:project-type="updatingModpack ? 'modpack' : updatingProject?.project_type"
					:project-icon-url="
						updatingModpack ? modpack?.project.icon_url : updatingProject?.project?.icon_url
					"
					:project-name="
						updatingModpack
							? (modpack?.project.title ?? formatMessage(commonMessages.modpackLabel))
							: (updatingProject?.project?.title ?? updatingProject?.file_name)
					"
					:loading="false"
					:loading-changelog="loadingChangelog"
					@update="handleModalUpdate"
					@cancel="resetUpdateState"
					@version-select="handleVersionSelect"
					@version-hover="handleVersionHover"
				/>
			</template>
		</ContentPageLayout>
	</ReadyTransition>
	<ConfirmModpackUpdateModal
		ref="modpackUpdateModal"
		:downgrade="false"
		:backup-tip="''"
		server
		@confirm="handleModpackUpdateConfirm"
		@cancel="handleModpackUpdateCancel"
	/>
</template>
