<script setup lang="ts">
import { core as coreRoutes } from '@amberite/contracts/core'
import type { ContentItem as EngineContentItem } from '@amberite/contracts/engine'
import { type Labrinth, ModrinthApiError } from '@modrinth/api-client'
import { UnlinkIcon } from '@modrinth/assets'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { ButtonMenuOption } from '#ui/components/base/buttons'
import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import UnknownFileWarningModal from '#ui/components/modal/UnknownFileWarningModal.vue'
import type { UpdateAllSelection } from '#ui/components/modal/update-all-modal/update-all-modal-types'
import { useContentUpload } from '#ui/composables/content-upload'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerPermissions } from '#ui/composables/server-permissions'
import { useServerPreferences } from '#ui/composables/server-preferences'
import { serverKeys } from '#ui/composables/server-queries'
import { addServerVersions } from '#ui/composables/server-setup'
import {
	injectModrinthClient,
	injectModrinthServerContext,
	injectNotificationManager,
	injectServerSettingsModal,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'
import { versionChangesGameVersion } from '#ui/utils/version-compatibility'

import type { BrowseInstallPlan } from '../../../shared/browse-tab/composables/install-logic'
import {
	flushStoredServerAddonInstallQueue,
	getStoredServerAddonInstallQueue,
	getTargetInstallPreferences,
	resolveServerAddonInstallPlans,
} from '../../../shared/browse-tab/composables/install-logic'
import ManagedContentModal from '../../../shared/content-tab/components/managed-content-modal/index.vue'
import ConfirmEnvironmentModal from '../../../shared/content-tab/components/modals/ConfirmEnvironmentModal.vue'
import ConfirmModpackUpdateModal from '../../../shared/content-tab/components/modals/ConfirmModpackUpdateModal.vue'
import ConfirmUnlinkModal from '../../../shared/content-tab/components/modals/ConfirmUnlinkModal.vue'
import ContentUpdaterModal from '../../../shared/content-tab/components/modals/content-updater-modal/index.vue'
import ContentPageLayout from '../../../shared/content-tab/layout.vue'
import type { ManagedContentData } from '../../../shared/content-tab/providers/content-manager'
import { provideContentManager } from '../../../shared/content-tab/providers/content-manager'
import { useInstanceContent } from '../../../shared/content-tab/providers/instance-content'
import type { ContentItem, ContentSide } from '../../../shared/content-tab/types'
import {
	friendlyAddonName,
	getAddonEnvironment,
	isEnabledForAnySide,
	isIncompatibleEnvironment,
	isPlayerOnlyContent,
	isRequiredEnvironment,
} from '../../../shared/content-tab/utils/addon-environment'
import { summarizeManagedContent } from '../../../shared/content-tab/utils/managed-content'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	modpackContent: {
		id: 'hosting.content.managed-content.modpack-header',
		defaultMessage: 'Modpack content',
	},
	failedToRemoveContent: {
		id: 'hosting.content.failed-to-remove',
		defaultMessage: 'Failed to remove content',
	},
	failedToToggle: {
		id: 'hosting.content.failed-to-toggle',
		defaultMessage: 'Failed to enable or disable {name}',
	},
	failedToSetEnabledFor: {
		id: 'hosting.content.failed-to-set-enabled-for',
		defaultMessage: 'Failed to change where {name} is enabled',
	},
	failedToUpload: {
		id: 'hosting.content.failed-to-upload',
		defaultMessage: 'Failed to upload file',
	},
	failedToUnlink: {
		id: 'hosting.content.failed-to-unlink',
		defaultMessage: 'Failed to unlink modpack',
	},
	failedToLoadModpackContent: {
		id: 'hosting.content.failed-to-load-modpack-content',
		defaultMessage: 'Failed to load modpack content',
	},
	failedToLoadVersions: {
		id: 'hosting.content.failed-to-load-versions',
		defaultMessage: 'Failed to load versions',
	},
	failedToUpdate: {
		id: 'hosting.content.failed-to-update',
		defaultMessage: 'Failed to update',
	},
	failedToBulkDelete: {
		id: 'hosting.content.failed-to-bulk-delete',
		defaultMessage: 'Failed to delete content',
	},
	failedToBulkUpdate: {
		id: 'hosting.content.failed-to-bulk-update',
		defaultMessage: 'Failed to update content',
	},
	failedToInstallContent: {
		id: 'hosting.content.failed-to-install',
		defaultMessage: 'Failed to install content',
	},
	instanceContent: {
		id: 'hosting.content.instance.content-header',
		defaultMessage: 'Instance content',
	},
	restartToApply: {
		id: 'hosting.content.instance.restart-to-apply',
		defaultMessage: 'Restart to apply',
	},
	retryUpdate: {
		id: 'hosting.content.instance.retry-update',
		defaultMessage: 'Retry update',
	},
	failedToApplyUpdate: {
		id: 'hosting.content.instance.apply-failed',
		defaultMessage: 'Could not apply the update',
	},
	unlinkFromInstance: {
		id: 'hosting.content.instance.unlink',
		defaultMessage: 'Unlink from instance',
	},
	updateWaiting: {
		id: 'hosting.content.instance.update-waiting',
		defaultMessage: 'Update applies when the server stops',
	},
	updateFailed: {
		id: 'hosting.content.instance.update-failed-status',
		defaultMessage: 'Last update failed',
	},
})

const client = injectModrinthClient()
const {
	serverId,
	coreId,
	core,
	server,
	busyReasons,
	beginInstallation,
	cancelOptimisticInstallation,
	uploadState,
	cancelUpload,
} = injectModrinthServerContext()
const contentUpload = useContentUpload({
	client: core,
	instanceId: serverId,
	uploadState,
	cancelUpload,
})
const unknownFileWarningModal = ref<InstanceType<typeof UnknownFileWarningModal> | null>()
const unknownFileName = ref('')
let resolveUnknownFileConfirmation: ((confirmed: boolean) => void) | null = null
const skipUnknownFileWarningKey = 'hosting-skip-unknown-file-warning'
const { addNotification } = injectNotificationManager()
const { openServerSettings, browseServerContent } = injectServerSettingsModal()
const { canSetup, permissionDeniedMessage } = useServerPermissions()
const router = useRouter()
const queryClient = useQueryClient()
const userPreferences = useServerPreferences(serverId)
const environmentWarningModal = ref<InstanceType<typeof ConfirmEnvironmentModal>>()

const params = { params: { id: serverId } }

const type = computed(() => {
	const loader = server.value?.loader
	if (loader === 'Paper') return 'plugin'
	if (loader === 'Vanilla') return 'datapack'
	return 'mod'
})

/** The App instance this server follows, whose owner pushes its versions. */
const follow = computed(() => server.value?.linked_instance)
const linkedInstanceId = computed(() => follow.value?.id ?? null)
const applyingInstanceUpdate = ref(false)

// Under the server's content key, which its Core's socket invalidates when the server changes.
const contentKey = serverKeys.content(serverId)
const content = useInstanceContent({
	client: core,
	instanceId: serverId,
	queryKeys: {
		content: () => [...contentKey, 'own'],
		packContent: () => [...contentKey, 'pack-content'],
	},
	mapping: { formatMessage },
	packContentEnabled: (modpack) => !!modpack || !!follow.value,
})
const contentQuery = content.content
const packContentQuery = content.packContent
const linkedModpack = computed(() => contentQuery.data.value?.modpack ?? null)

const isModpackContentModalOpen = ref(false)

/** Files the panel is changing now, by path, shown as installing until the list reads again. */
const changing = ref(new Set<string>())

/** Shows the files the panel is changing as installing. */
function withChanging(items: ContentItem[] | undefined) {
	return (items ?? []).map((item) => ({ ...item, installing: changing.value.has(item.id) }))
}

const contentItems = computed(() => withChanging(content.items.value))
const packContentItems = computed(() => withChanging(content.packItems.value))

const refresh = content.refresh

const setupActionDisabled = computed(() => !canSetup.value || busyReasons.value.length > 0)
const isInstallingContent = computed(
	() =>
		server.value?.status === 'installing' ||
		busyReasons.value.some(
			(r) =>
				r.reason.id === 'servers.busy.installing' || r.reason.id === 'servers.busy.syncing-content',
		),
)
const setupActionBusyMessage = computed(() => {
	if (!canSetup.value) return permissionDeniedMessage.value

	const filteredReasons = busyReasons.value.filter((r) => {
		if (
			isInstallingContent.value &&
			(r.reason.id === 'servers.busy.installing' || r.reason.id === 'servers.busy.syncing-content')
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
})

const contentActionDisabled = computed(() => !canSetup.value || busyReasons.value.length > 0)
const contentActionBusyMessage = computed(() => {
	if (!canSetup.value) return permissionDeniedMessage.value
	return busyReasons.value.length > 0 ? formatMessage(busyReasons.value[0].reason) : null
})

const modpackProjectId = computed(() => linkedModpack.value?.project.id ?? null)

const modpackVersionsQuery = useQuery({
	queryKey: computed(() => ['labrinth', 'versions', 'v2', modpackProjectId.value]),
	queryFn: () =>
		client.labrinth.versions_v2.getProjectVersions(modpackProjectId.value!, {
			include_changelog: false,
		}),
	enabled: computed(() => !!modpackProjectId.value),
})

function getVersionTime(version: Labrinth.Versions.v2.Version) {
	return new Date(version.date_published).getTime()
}

function sortVersionsByPublishedDate(versions: Labrinth.Versions.v2.Version[]) {
	return [...versions].sort((a, b) => getVersionTime(b) - getVersionTime(a))
}

const updatingModpack = ref(false)
const modpackVersions = computed(() => modpackVersionsQuery.data.value ?? [])
const currentModpackVersionId = computed(() => linkedModpack.value?.version?.id ?? null)

const managedContent = computed<ManagedContentData | null>(() => {
	const pack = linkedModpack.value
	const followed = follow.value
	if (!pack && !followed) return null
	const summary = packContentQuery.data.value
		? summarizeManagedContent(packContentItems.value)
		: packContentQuery.isLoading.value
			? undefined
			: []
	const disabled = setupActionDisabled.value || applyingInstanceUpdate.value
	const disabledText = setupActionBusyMessage.value ?? formatMessage(commonMessages.installingLabel)
	if (followed) {
		return {
			card: {
				kind: 'modpack',
				installing:
					isInstallingContent.value ||
					followed.state === 'downloading' ||
					followed.state === 'applying',
				title: followed.name,
				status:
					followed.state === 'pending'
						? formatMessage(messages.updateWaiting)
						: followed.state === 'failed'
							? (followed.error ?? formatMessage(messages.updateFailed))
							: undefined,
				// Only when a pushed update is waiting or failed; an up-to-date server has nothing to do.
				primaryActionLabel:
					followed.state === 'pending'
						? formatMessage(messages.restartToApply)
						: followed.state === 'failed'
							? formatMessage(messages.retryUpdate)
							: undefined,
				manager: {
					name: followed.name,
					link: `/instance/${encodeURIComponent(followed.id)}`,
				},
				summary,
				canSwitchVersion: false,
			},
			disabled,
			disabledText,
		}
	}
	const project = pack!.project
	const slug = project.slug ?? project.id
	return {
		card: {
			kind: 'modpack',
			installing: isInstallingContent.value,
			manager: {
				name: project.title,
				iconUrl: project.icon_url ?? undefined,
				link: `/project/${slug}`,
			},
			summary,
			versionNumber: pack!.version?.version_number,
			versionLink: pack!.version ? `/project/${slug}/version/${pack!.version.id}` : undefined,
			updatedAt: pack!.version?.date_published,
			canSwitchVersion: true,
		},
		disabled,
		disabledText,
	}
})

const managedContentMenuOptions = computed<ButtonMenuOption[]>(() =>
	linkedInstanceId.value
		? [
				{
					id: 'unlink',
					label: formatMessage(messages.unlinkFromInstance),
					icon: UnlinkIcon,
					tone: 'red',
					action: () => handleModpackUnlink(),
				},
			]
		: [],
)

/** Applies the version the instance pushed: restarts a running server, or retries a failed one. */
async function handleApplyInstanceUpdate() {
	const followed = follow.value
	if (!followed?.shared_instance_id || applyingInstanceUpdate.value) return
	applyingInstanceUpdate.value = true
	try {
		await core.call(coreRoutes.FollowInstance, {
			...params,
			body: {
				instance: { id: followed.id, name: followed.name },
				shared_instance_id: followed.shared_instance_id,
				restart: true,
			},
		})
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToApplyUpdate),
			text: err instanceof Error ? err.message : undefined,
		})
	} finally {
		applyingInstanceUpdate.value = false
	}
}

/** The engine's files by path, the id each row has. */
const filesByPath = computed(
	() =>
		new Map(
			[...(contentQuery.data.value?.items ?? []), ...(packContentQuery.data.value ?? [])].map(
				(item) => [item.file_path, item],
			),
		),
)

function fileFor(item: ContentItem) {
	return filesByPath.value.get(item.id)
}

const isFlushingStoredServerInstalls = ref(false)

function getInstalledProjectIds() {
	return new Set(
		(contentQuery.data.value?.items ?? [])
			.map((item) => item.project?.id)
			.filter((projectId): projectId is string => !!projectId),
	)
}

function toResolvePreferences(
	preferences?: BrowseInstallPlan['preferences'],
): Labrinth.Content.v3.ResolutionPreferences {
	return {
		game_versions: preferences?.gameVersions,
		loaders: preferences?.loaders,
	}
}

async function resolveStoredServerAddonPlans(plans: BrowseInstallPlan[]) {
	return await resolveServerAddonInstallPlans({
		plans,
		existingProjectIds: getInstalledProjectIds(),
		resolvePlan: async (plan, existingProjectIds) => {
			const target = getTargetInstallPreferences(
				{
					gameVersion: server.value?.mc_version ?? undefined,
					loader: server.value?.loader?.toLowerCase(),
				},
				plan.contentType,
			)
			const resolved = await client.labrinth.content_v3.resolve({
				project_id: plan.projectId,
				version_id: plan.versionId,
				content_type: plan.contentType as Labrinth.Content.v3.ContentType,
				selected: toResolvePreferences(plan.preferences),
				target: toResolvePreferences(target),
				existing_project_ids: existingProjectIds,
				update_channel: server.value?.update_channel,
			})

			return [resolved.primary, ...resolved.dependencies].map((item) => ({
				projectId: item.project_id,
				versionId: item.version_id,
			}))
		},
	})
}

/** Installs what the browse page queued for this server, then reads the list again. */
async function flushStoredServerInstalls() {
	if (isFlushingStoredServerInstalls.value) return
	// A Core server has one world, named by the server's id.
	const queuedPlans = getStoredServerAddonInstallQueue(serverId, serverId)
	if (queuedPlans.size === 0) return

	isFlushingStoredServerInstalls.value = true
	try {
		const result = await flushStoredServerAddonInstallQueue({
			serverId,
			worldId: serverId,
			install: async (plans) => {
				const addons = await resolveStoredServerAddonPlans(plans)
				await addServerVersions(
					client,
					coreId,
					serverId,
					addons.map((addon) => addon.version_id),
				)
			},
		})

		if (!result.ok) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToInstallContent),
				text: result.error instanceof Error ? result.error.message : undefined,
			})
		}
	} finally {
		isFlushingStoredServerInstalls.value = false
		await refresh()
	}
}
void flushStoredServerInstalls()

const contentReadyPending = computed(
	() => contentQuery.isLoading.value && contentQuery.data.value === undefined,
)

/** Runs `change` on the files at `paths`, showing them as installing until the list reads again. */
async function changeFiles(
	paths: string[],
	change: () => Promise<unknown>,
	failure: (error: unknown) => void,
) {
	changing.value = new Set([...changing.value, ...paths])
	try {
		await change()
	} catch (error) {
		failure(error)
	} finally {
		await refresh()
		const next = new Set(changing.value)
		for (const path of paths) next.delete(path)
		changing.value = next
	}
}

type SideChanges = { enabled?: boolean; server?: boolean; player?: boolean }

/** Turns the file on or off, then its sides. Turning it on renames it, so sides use its new path. */
function applyChanges(addon: EngineContentItem, changes: SideChanges) {
	return changeFiles(
		[addon.file_path],
		async () => {
			let path = addon.file_path
			if (changes.enabled !== undefined) path = await content.toggle(path, changes.enabled)
			for (const side of ['server', 'player'] as const) {
				const enabled = changes[side]
				if (enabled === undefined) continue
				await content.setSide(path, side, enabled)
			}
		},
		(error) =>
			addNotification({
				type: 'error',
				title: formatMessage(
					changes.enabled !== undefined ? messages.failedToToggle : messages.failedToSetEnabledFor,
					{ name: friendlyAddonName(addon) },
				),
				text: error instanceof Error ? error.message : undefined,
			}),
	)
}

async function handleSetEnabledFor(item: ContentItem, side: ContentSide, enabled: boolean) {
	if (contentActionDisabled.value) return
	const addon = fileFor(item)
	if (
		!addon ||
		(addon.server?.installed === false && side !== 'server') ||
		(side === 'server' && isPlayerOnlyContent(addon))
	)
		return
	const showPerformanceWarning = enabled && side === 'server' && addon.project?.id === 'uCdwusMi'
	if (
		userPreferences.value.warnOnIncompatibleContent &&
		(showPerformanceWarning ||
			(enabled ? isIncompatibleEnvironment(addon, side) : isRequiredEnvironment(addon, side)))
	) {
		const confirmed = await environmentWarningModal.value?.show(
			friendlyAddonName(addon),
			side,
			enabled,
			getAddonEnvironment(addon) === 'singleplayer_only',
			showPerformanceWarning,
		)
		if (!confirmed || contentActionDisabled.value) return
	}
	await applyChanges(addon, { [side]: enabled })
}

async function handleToggleEnabled(item: ContentItem) {
	if (contentActionDisabled.value) return
	const addon = fileFor(item)
	if (!addon || addon.server?.installed === false) return
	if (addon.enabled && !isEnabledForAnySide(addon)) {
		const sides = (['server', 'player'] as const).filter(
			(side) =>
				!(side === 'server' && isPlayerOnlyContent(addon)) &&
				!isIncompatibleEnvironment(addon, side),
		)
		const changes: SideChanges = {}
		for (const side of sides.length > 0 ? sides : (['player'] as const)) changes[side] = true
		await applyChanges(addon, changes)
		return
	}
	await applyChanges(addon, { enabled: !addon.enabled })
}

async function handleDeleteItem(item: ContentItem) {
	await handleBulkDelete([item])
}

async function handleBulkDelete(items: ContentItem[]) {
	if (contentActionDisabled.value) return
	const paths = items.filter((item) => !item.installing && fileFor(item)).map((item) => item.id)
	if (paths.length === 0) return
	await changeFiles(
		paths,
		async () => {
			for (const path of paths) await content.remove(path)
		},
		(error) =>
			addNotification({
				type: 'error',
				title: formatMessage(
					paths.length > 1 ? messages.failedToBulkDelete : messages.failedToRemoveContent,
				),
				text: error instanceof Error ? error.message : undefined,
			}),
	)
}

const modpackUnlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const modpackContentModal = ref<InstanceType<typeof ManagedContentModal>>()
const contentUpdaterModal = ref<InstanceType<typeof ContentUpdaterModal>>()

const updatingProject = ref<ContentItem | null>(null)
const loadingChangelog = ref(false)

watch(packContentItems, (items) => {
	if (isModpackContentModalOpen.value) modpackContentModal.value?.setItems(items)
})

const updatingProjectId = computed(() => updatingProject.value?.project?.id ?? null)

const projectVersionsQuery = useQuery({
	queryKey: computed(() => ['labrinth', 'versions', 'v2', updatingProjectId.value]),
	queryFn: () =>
		client.labrinth.versions_v2.getProjectVersions(updatingProjectId.value!, {
			include_changelog: false,
		}),
	enabled: computed(() => !!updatingProjectId.value && !updatingModpack.value),
})

const updatingProjectVersions = computed(() => {
	const source = updatingModpack.value ? modpackVersions.value : projectVersionsQuery.data.value
	if (!source) return []
	return sortVersionsByPublishedDate(source)
})

const loadingVersions = computed(() =>
	updatingModpack.value
		? modpackVersionsQuery.isLoading.value
		: projectVersionsQuery.isLoading.value,
)

const modpackUpdateModal = ref<InstanceType<typeof ConfirmModpackUpdateModal>>()
const pendingModpackUpdateVersion = ref<Labrinth.Versions.v2.Version | null>(null)
const isModpackUpdateDowngrade = ref(false)

const currentGameVersion = computed(() => server.value?.mc_version ?? '')
const currentLoader = computed(() => server.value?.loader?.toLowerCase() ?? '')

function handleBrowseContent() {
	if (contentActionDisabled.value) return
	const contentType = type.value
	if (browseServerContent) {
		browseServerContent({ serverId, type: contentType })
		return
	}

	router.push({ path: `/discover/${contentType}s`, query: { sid: serverId } })
}

function handleUploadFiles() {
	if (contentActionDisabled.value) return
	const input = document.createElement('input')
	input.type = 'file'
	input.multiple = true
	input.accept = type.value === 'datapack' ? '.zip' : '.jar'
	input.onchange = async () => {
		if (!input.files) return
		const files = Array.from(input.files)

		try {
			const fileRecognition = await Promise.all(files.map(isFileOnModrinth))
			const unrecognizedFileSet = new Set(files.filter((_, index) => !fileRecognition[index]))
			const confirmedFiles: File[] = []
			for (const file of files) {
				if (!unrecognizedFileSet.has(file) || (await confirmUnknownFileInstallation(file.name))) {
					confirmedFiles.push(file)
				}
			}
			if (confirmedFiles.length === 0) return

			await contentUpload.uploadFiles(confirmedFiles)
		} catch (err) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToUpload),
				text: err instanceof Error ? err.message : undefined,
			})
		} finally {
			await refresh()
		}
	}
	input.click()
}

async function isFileOnModrinth(file: File) {
	const buffer = await file.arrayBuffer()
	const digest = await crypto.subtle.digest('SHA-1', buffer)
	const hash = Array.from(new Uint8Array(digest), (byte) =>
		byte.toString(16).padStart(2, '0'),
	).join('')

	try {
		await client.labrinth.versions_v2.getVersionFromFileHash(hash, 'sha1')
		return true
	} catch (error) {
		return !(error instanceof ModrinthApiError && error.statusCode === 404)
	}
}

function confirmUnknownFileInstallation(fileName: string) {
	if (localStorage.getItem(skipUnknownFileWarningKey) === 'true') {
		return Promise.resolve(true)
	}

	unknownFileName.value = fileName
	return new Promise<boolean>((resolve) => {
		resolveUnknownFileConfirmation = resolve
		void nextTick(() => unknownFileWarningModal.value?.show())
	})
}

function resolveUnknownFileWarning(confirmed: boolean) {
	const resolve = resolveUnknownFileConfirmation
	resolveUnknownFileConfirmation = null
	unknownFileName.value = ''
	resolve?.(confirmed)
}

function handleUnknownFileContinue(dontShowAgain: boolean) {
	if (dontShowAgain) localStorage.setItem(skipUnknownFileWarningKey, 'true')
	resolveUnknownFileWarning(true)
}

async function handleViewModpackContent() {
	isModpackContentModalOpen.value = true

	if (packContentQuery.data.value) {
		modpackContentModal.value?.show(packContentItems.value)
		return
	}

	modpackContentModal.value?.showLoading()
	try {
		const { data } = await packContentQuery.refetch()
		if (!data) throw new Error('Failed to load modpack content')
		modpackContentModal.value?.show(packContentItems.value)
	} catch (err) {
		isModpackContentModalOpen.value = false
		modpackContentModal.value?.hide()
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToLoadModpackContent),
			text: err instanceof Error ? err.message : undefined,
		})
	}
}

function handleModpackUnlink() {
	modpackUnlinkModal.value?.show()
}

async function handleModpackUnlinkConfirm() {
	if (setupActionDisabled.value) return
	try {
		await content.unlinkModpack()
	} catch (err) {
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToUnlink),
			text: err instanceof Error ? err.message : undefined,
		})
	} finally {
		await refresh()
	}
}

async function handleBulkUpdate(selections: UpdateAllSelection[]) {
	if (contentActionDisabled.value) return
	const updates = selections.flatMap((selection) => {
		const item = contentItems.value.find((item) => item.id === selection.id)
		if (
			!item?.has_update ||
			item.locked ||
			item.installing ||
			item.project?.id !== selection.projectId
		)
			return []
		return [{ path: item.id, versionId: selection.version.id }]
	})
	if (updates.length === 0) return
	let failed: unknown
	await changeFiles(
		updates.map(({ path }) => path),
		async () => {
			for (const { path, versionId } of updates) await content.switchVersion(path, versionId)
		},
		(error) => {
			failed = error
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToBulkUpdate),
				text: error instanceof Error ? error.message : undefined,
			})
		},
	)
	if (failed) throw failed
}

async function handleUpdateItem(id: string) {
	const item = contentItems.value.find((i) => i.id === id)
	if (!item?.has_update || !item.project?.id || !item.version?.id) return

	updatingModpack.value = false
	updatingProject.value = item
	loadingChangelog.value = false

	await nextTick()

	contentUpdaterModal.value?.show(item.update_version_id ?? undefined)
}

async function handleSwitchVersion(item: ContentItem) {
	if (!item.project?.id || !item.version?.id) return

	updatingModpack.value = false
	updatingProject.value = item
	loadingChangelog.value = false

	await nextTick()

	contentUpdaterModal.value?.show(item.version.id, { switchMode: true })
}

async function handleModpackUpdate() {
	if (setupActionDisabled.value) return
	if (linkedInstanceId.value) return handleApplyInstanceUpdate()
	const pack = linkedModpack.value
	if (!pack) return

	updatingModpack.value = true
	updatingProject.value = null
	loadingChangelog.value = false

	await nextTick()

	contentUpdaterModal.value?.show(pack.update_version_id ?? undefined)
}

function spliceVersionInCache(fullVersion: Labrinth.Versions.v2.Version) {
	const projectId = updatingModpack.value ? modpackProjectId.value : updatingProjectId.value
	if (!projectId) return
	const key = ['labrinth', 'versions', 'v2', projectId]
	queryClient.setQueryData(key, (old: Labrinth.Versions.v2.Version[] | undefined) => {
		if (!old) return old
		return old.map((v) => (v.id === fullVersion.id ? fullVersion : v))
	})
}

async function handleVersionSelect(version: Labrinth.Versions.v2.Version) {
	if (version.changelog) return
	loadingChangelog.value = true
	try {
		const fullVersion = await client.labrinth.versions_v2.getVersion(version.id)
		spliceVersionInCache(fullVersion)
	} catch {
		// Silently fail on changelog fetch
	} finally {
		loadingChangelog.value = false
	}
}

async function handleVersionHover(version: Labrinth.Versions.v2.Version) {
	if (version.changelog) return
	try {
		const fullVersion = await client.labrinth.versions_v2.getVersion(version.id)
		spliceVersionInCache(fullVersion)
	} catch {
		// Silently fail on hover prefetch
	}
}

function resetUpdateState() {
	updatingModpack.value = false
	updatingProject.value = null
	loadingChangelog.value = false
}

function handleModalUpdate(selectedVersion: Labrinth.Versions.v2.Version, event?: MouseEvent) {
	if (updatingModpack.value) {
		if (setupActionDisabled.value) return
		pendingModpackUpdateVersion.value = selectedVersion

		const currentVersion = updatingProjectVersions.value.find(
			(v) => v.id === currentModpackVersionId.value,
		)
		isModpackUpdateDowngrade.value = currentVersion
			? new Date(selectedVersion.date_published) < new Date(currentVersion.date_published)
			: false
		const shouldShowWarning =
			isModpackUpdateDowngrade.value ||
			versionChangesGameVersion(selectedVersion, currentGameVersion.value)

		if (event?.shiftKey || !shouldShowWarning) {
			handleModpackUpdateConfirm()
		} else {
			modpackUpdateModal.value?.show()
		}
		return
	}

	if (contentActionDisabled.value) return
	void performUpdate(selectedVersion)
}

async function performUpdate(selectedVersion: Labrinth.Versions.v2.Version) {
	const item = updatingProject.value
	const modpack = updatingModpack.value
	resetUpdateState()
	if (modpack) {
		if (setupActionDisabled.value) return
		const pack = linkedModpack.value
		if (!pack) return
		try {
			beginInstallation({
				type: 'modrinth_modpack',
				project_id: pack.project.id,
				version_id: selectedVersion.id,
			})
			await content.updateModpack(selectedVersion.id)
		} catch (err) {
			cancelOptimisticInstallation()
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToUpdate),
				text: err instanceof Error ? err.message : undefined,
			})
		}
		return
	}
	if (!item || contentActionDisabled.value) return
	await changeFiles(
		[item.id],
		() => content.switchVersion(item.id, selectedVersion.id),
		(error) =>
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedToUpdate),
				text: error instanceof Error ? error.message : undefined,
			}),
	)
}

function handleModpackUpdateConfirm() {
	if (setupActionDisabled.value) return
	if (pendingModpackUpdateVersion.value) {
		contentUpdaterModal.value?.hide()
		void performUpdate(pendingModpackUpdateVersion.value)
		pendingModpackUpdateVersion.value = null
	}
}

function handleModpackUpdateCancel() {
	pendingModpackUpdateVersion.value = null
}

provideContentManager({
	items: contentItems,
	loading: computed(() => contentQuery.isLoading.value),
	error: computed(() => contentQuery.error.value ?? null),
	managedContent,
	isPackLocked: ref(false),
	isBusy: contentActionDisabled,
	disableWhileMutating: false,
	busyMessage: contentActionBusyMessage,
	disableAddContent: computed(() => !canSetup.value),
	disableAddContentTooltip: permissionDeniedMessage.value,
	contentTypeLabel: type,
	toggleEnabled: handleToggleEnabled,
	// A Core keeps side toggles only for a server that follows no instance: the instance's owner
	// decides them for its servers.
	get setEnabledFor() {
		return linkedInstanceId.value ? undefined : handleSetEnabledFor
	},
	deleteItem: handleDeleteItem,
	bulkDeleteItems: handleBulkDelete,
	refresh,
	browse: handleBrowseContent,
	uploadFiles: handleUploadFiles,
	deletionContext: 'server',
	showEnvironmentWarnings: true,
	hasUpdateSupport: true,
	updateItem: handleUpdateItem,
	bulkUpdateSelections: handleBulkUpdate,
	currentGameVersion,
	currentLoader,
	runManagedContentPrimaryAction: handleModpackUpdate,
	viewManagedContent: handleViewModpackContent,
	unlinkModpack: handleModpackUnlink,
	managedContentMenuOptions,
	openManagedContentSettings: () => openServerSettings({ tabId: 'installation' }),
	switchVersion: handleSwitchVersion,
	getItemId: (item) => item.id,
	mapToTableItem: (item) => {
		const projectType = item.project_type ?? type.value
		const projectSlugOrId = item.project?.slug ?? item.project?.id
		return {
			id: item.id,
			project: item.project ?? {
				id: item.file_name,
				slug: item.file_name,
				title: friendlyAddonName(item),
				icon_url: undefined,
			},
			projectLink: projectSlugOrId ? `/${projectType}/${projectSlugOrId}` : undefined,
			version: item.version ?? {
				id: item.file_name,
				version_number:
					item.embedded_metadata?.version ?? formatMessage(commonMessages.unknownLabel),
				file_name: item.file_name,
			},
			versionLink:
				projectSlugOrId && item.version?.id
					? `/${projectType}/${projectSlugOrId}/version/${item.version.id}`
					: undefined,
			owner: item.owner
				? { ...item.owner, link: item.owner.link ?? `/${item.owner.type}/${item.owner.id}` }
				: undefined,
			external: item.external,
			enabled: item.enabled,
			enabledFor: item.enabledFor,
			installing: item.installing,
		}
	},
	filterPersistKey: `server:${serverId}`,
})

async function handleModpackSetEnabledFor(item: ContentItem, side: ContentSide, enabled: boolean) {
	await handleSetEnabledFor(item, side, enabled)
}

async function handleModpackToggleEnabled(item: ContentItem) {
	await handleToggleEnabled(item)
}
</script>

<template>
	<ReadyTransition :pending="contentReadyPending">
		<ContentPageLayout :bottom-padding="false">
			<template #modals>
				<UnknownFileWarningModal
					ref="unknownFileWarningModal"
					mode="mod"
					:file-name="unknownFileName"
					@cancel="resolveUnknownFileWarning(false)"
					@continue="handleUnknownFileContinue"
				/>
				<ConfirmUnlinkModal
					ref="modpackUnlinkModal"
					server
					:instance="!!linkedInstanceId"
					:action-disabled="setupActionDisabled"
					:action-disabled-tooltip="setupActionBusyMessage ?? undefined"
					@unlink="handleModpackUnlinkConfirm"
				/>
				<ManagedContentModal
					ref="modpackContentModal"
					:source-name="managedContent?.card.manager.name"
					:source-icon-url="managedContent?.card.manager.iconUrl"
					:header="
						formatMessage(linkedInstanceId ? messages.instanceContent : messages.modpackContent)
					"
					:enable-enabled-for="!linkedInstanceId"
					show-environment-warnings
					:action-disabled="contentActionDisabled"
					:action-disabled-tooltip="contentActionBusyMessage ?? undefined"
					@update:enabled="handleModpackToggleEnabled"
					@update:enabled-for="handleModpackSetEnabledFor"
					@hide="isModpackContentModalOpen = false"
				/>
				<ContentUpdaterModal
					v-if="updatingProject || updatingModpack"
					ref="contentUpdaterModal"
					:versions="updatingProjectVersions"
					:current-game-version="currentGameVersion"
					:current-loader="currentLoader"
					:current-version-id="
						updatingModpack ? (currentModpackVersionId ?? '') : (updatingProject?.version?.id ?? '')
					"
					:is-app="false"
					:project-type="updatingModpack ? 'modpack' : updatingProject?.project_type"
					:project-icon-url="
						updatingModpack
							? managedContent?.card.manager.iconUrl
							: updatingProject?.project?.icon_url
					"
					:project-name="
						updatingModpack
							? (managedContent?.card.manager.name ?? formatMessage(commonMessages.modpackLabel))
							: (updatingProject?.project?.title ?? updatingProject?.file_name)
					"
					:loading="loadingVersions"
					:loading-changelog="loadingChangelog"
					:action-disabled="updatingModpack ? setupActionDisabled : contentActionDisabled"
					:action-disabled-tooltip="
						(updatingModpack ? setupActionBusyMessage : contentActionBusyMessage) ?? undefined
					"
					@update="handleModalUpdate"
					@cancel="resetUpdateState"
					@version-select="handleVersionSelect"
					@version-hover="handleVersionHover"
				/>
			</template>
		</ContentPageLayout>
	</ReadyTransition>
	<ConfirmEnvironmentModal ref="environmentWarningModal" :action-disabled="contentActionDisabled" />
	<ConfirmModpackUpdateModal
		ref="modpackUpdateModal"
		:downgrade="isModpackUpdateDowngrade"
		:backup-tip="
			[managedContent?.card.manager.name, pendingModpackUpdateVersion?.version_number]
				.filter(Boolean)
				.join(' ')
		"
		server
		:action-disabled="setupActionDisabled"
		:action-disabled-tooltip="setupActionBusyMessage ?? undefined"
		@confirm="handleModpackUpdateConfirm"
		@cancel="handleModpackUpdateCancel"
	/>
</template>
