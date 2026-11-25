<template>
	<div class="flex flex-col gap-6">
		<InstallationSettingsLayout
			ref="installationSettingsLayout"
			@reset-server="showResetServerModal"
		>
			<template #extra>
				<div class="flex flex-col gap-2.5">
					<span class="text-lg font-semibold text-contrast">{{
						formatMessage(messages.resetServerTitle)
					}}</span>
					<div>
						<Button
							v-tooltip="resetServerDisabledTooltip"
							type="colored"
							color="red"
							:disabled="resetServerDisabled"
							@click="showResetServerModal"
						>
							<RotateCounterClockwiseIcon class="size-5" />
							{{ formatMessage(commonMessages.resetServerButton) }}
						</Button>
					</div>
					<span class="text-primary">
						{{ formatMessage(messages.resetServerDescription) }}
					</span>
				</div>
			</template>

			<template #extra-modals>
				<Teleport to="body">
					<div class="relative z-[100]">
						<ServerSetupModal
							ref="setupModal"
							@reinstall="onReinstall"
							@browse-modpacks="onBrowseModpacks"
						/>
						<ServerModpackUploadModal
							ref="modpackUploadModal"
							:header="modpackUploadLabel"
							:confirm-label="modpackUploadLabel"
							:confirm-icon="
								modpackUploadAction === 'swap' ? ArrowLeftRightIcon : RotateCounterClockwiseIcon
							"
							:upload-progress="modpackUploadProgress"
							@confirm="uploadLocalModpackWithSoftOverride"
						/>
					</div>
				</Teleport>
			</template>
		</InstallationSettingsLayout>
	</div>
</template>

<script setup lang="ts">
import type { Server } from '@amberite/contracts/core'
import { type ContentItem as EngineContentItem, instanceContent } from '@amberite/contracts/engine'
import type { Labrinth, UploadProgress } from '@modrinth/api-client'
import { ArrowLeftRightIcon, RotateCounterClockwiseIcon } from '@modrinth/assets'
import {
	commonMessages,
	defineMessages,
	formatLoaderLabel,
	type GameVersionOption,
	injectModrinthClient,
	injectModrinthServerContext,
	injectNotificationManager,
	injectServerSettings,
	injectTags,
	InstallationSettingsLayout,
	type LoaderVersionEntry,
	provideInstallationSettings,
	ServerSetupModal,
	useDebugLogger,
	useModrinthServersConsole,
	useServerPermissions,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { Button } from '#ui/components/base/buttons'
import ServerModpackUploadModal from '#ui/components/servers/ServerModpackUploadModal.vue'
import type { ServerPlatform } from '#ui/composables/server-installation-tracker'
import { serverKeys } from '#ui/composables/server-queries'
import { serverLoader, setUpServer } from '#ui/composables/server-setup'
import { isDisabledFor } from '#ui/layouts/shared/content-tab/utils/addon-environment'

const debug = useDebugLogger('LoaderPage')
const client = injectModrinthClient()
const {
	beginInstallation,
	busyReasons,
	cancelOptimisticInstallation,
	installation,
	server,
	serverId,
	coreId,
	core,
} = injectModrinthServerContext()
const { addNotification } = injectNotificationManager()
const queryClient = useQueryClient()
const serverDetailQueryKey = serverKeys.detail(serverId)
const contentQueryKey = serverKeys.content(serverId)
const tags = injectTags()
const { formatMessage } = useVIntl()
const serverSettings = injectServerSettings()
const modrinthServersConsole = useModrinthServersConsole()
const { canSetup, canResetServer, permissionDeniedMessage } = useServerPermissions()

const modpackUploadModal =
	useTemplateRef<InstanceType<typeof ServerModpackUploadModal>>('modpackUploadModal')
const modpackUploadAction = ref<'reinstall' | 'swap'>('reinstall')
const modpackUploadProgress = ref<UploadProgress | null>(null)
const modpackUploadLabel = computed(() =>
	formatMessage(
		modpackUploadAction.value === 'swap'
			? commonMessages.changeVersionButton
			: commonMessages.reinstallModpackButton,
	),
)

const messages = defineMessages({
	resetServerTitle: {
		id: 'hosting.loader.reset-server',
		defaultMessage: 'Reset server',
	},
	resetServerDescription: {
		id: 'hosting.loader.reset-server-description',
		defaultMessage:
			'Removes all data on your server, including your worlds, mods, and configuration files. Backups will remain and can be restored. Invited players will keep their access.',
	},
	loaderVersionLabel: {
		id: 'hosting.loader.loader-version',
		defaultMessage: '{loader, select, null {Loader} other {{loader}}} version',
	},
	failedToLoadVersions: {
		id: 'hosting.loader.failed-to-load-versions',
		defaultMessage: 'Failed to load versions',
	},
	failedToChangeVersion: {
		id: 'hosting.loader.failed-to-change-version',
		defaultMessage: 'Failed to change modpack version',
	},
	failedToSaveSettings: {
		id: 'hosting.loader.failed-to-save-settings',
		defaultMessage: 'Failed to save installation settings',
	},
	repairStartedTitle: {
		id: 'hosting.loader.repair-started-title',
		defaultMessage: 'Repair completed',
	},
	repairStartedText: {
		id: 'hosting.loader.repair-started-text',
		defaultMessage: 'Your server installation has been repaired.',
	},
	failedToRepair: {
		id: 'hosting.loader.failed-to-repair',
		defaultMessage: 'Failed to repair server',
	},
	failedToReinstall: {
		id: 'hosting.loader.failed-to-reinstall',
		defaultMessage: 'Failed to reinstall modpack',
	},
	failedToUnlink: {
		id: 'hosting.loader.failed-to-unlink',
		defaultMessage: 'Failed to unlink modpack',
	},
})

const emit = defineEmits<{
	reinstall: [unknown?]
	'reinstall-failed': []
}>()

const isInstalling = computed(() => busyReasons.value.length > 0)
const setupActionDisabled = computed(() => !canSetup.value || isInstalling.value)
const setupActionDisabledMessage = computed(() => {
	if (!canSetup.value) return permissionDeniedMessage.value
	return busyReasons.value.length > 0 ? formatMessage(busyReasons.value[0].reason) : null
})
const resetServerDisabled = computed(() => !canResetServer.value || isInstalling.value)
const resetServerDisabledTooltip = computed(() => {
	if (!canResetServer.value) return permissionDeniedMessage.value
	return busyReasons.value.length > 0 ? formatMessage(busyReasons.value[0].reason) : undefined
})
const installationSettingsLayout = ref<InstanceType<typeof InstallationSettingsLayout>>()
const setupModal = ref<InstanceType<typeof ServerSetupModal>>()

function showResetServerModal() {
	if (resetServerDisabled.value) return
	setupModal.value?.show()
}

async function invalidateServerState() {
	debug('invalidateServerState: starting')
	await Promise.all([
		queryClient.invalidateQueries({ queryKey: serverDetailQueryKey }),
		queryClient.invalidateQueries({ queryKey: contentQueryKey }),
	])
	debug('invalidateServerState: complete')
}

const packQuery = useQuery({
	queryKey: [...contentQueryKey, 'pack'],
	queryFn: () =>
		core.call(instanceContent.GetLinkedModpack, { params: { id: serverId }, query: {} }),
	staleTime: 30_000,
})

const requiresInstallation = computed(() => installation.value?.status === 'failed')

/** The Modrinth modpack the server runs. */
const modpack = computed(() => packQuery.data.value ?? null)
/** The App instance the server follows, whose owner pushes its versions. */
const follow = computed(() => server.value.linked_instance)

const modpackProjectId = computed(() => modpack.value?.project.id ?? null)

const modpackVersionsQuery = useQuery({
	queryKey: computed(() => ['labrinth', 'versions', 'v2', modpackProjectId.value]),
	queryFn: () =>
		client.labrinth.versions_v2.getProjectVersions(modpackProjectId.value!, {
			include_changelog: false,
		}),
	enabled: computed(() => !!modpackProjectId.value),
})

const editingPlatform = ref(server.value?.loader?.toLowerCase() ?? 'vanilla')
const editingGameVersion = ref(server.value?.mc_version ?? '')

const modLoaders = ['fabric', 'forge', 'quilt', 'neoforge']
const loaderGameVersionPlaceholder = '${modrinth.gameVersion}'
const minecraftServerDownloadsStartTime = Date.parse('2012-04-04T00:00:00Z')

function toApiLoaderName(loader: string): string {
	return loader === 'neoforge' ? 'neo' : loader
}

const apiLoaderName = computed(() =>
	modLoaders.includes(editingPlatform.value) ? toApiLoaderName(editingPlatform.value) : null,
)
const manifestFormatVersion = computed(() => (apiLoaderName.value === 'quilt' ? 1 : 0))

const manifestQuery = useQuery({
	queryKey: computed(
		() => ['loader-manifest', apiLoaderName.value, manifestFormatVersion.value] as const,
	),
	queryFn: () =>
		client.launchermeta.manifest_v0.getManifest(apiLoaderName.value!, manifestFormatVersion.value),
	enabled: computed(() => !!apiLoaderName.value),
	staleTime: 5 * 60 * 1000,
})

const paperBuildsQuery = useQuery({
	queryKey: computed(() => ['paper-builds', editingGameVersion.value] as const),
	queryFn: () => client.paper.versions_v3.getBuilds(editingGameVersion.value),
	enabled: computed(() => editingPlatform.value === 'paper' && !!editingGameVersion.value),
	staleTime: 5 * 60 * 1000,
})

const purpurBuildsQuery = useQuery({
	queryKey: computed(() => ['purpur-builds', editingGameVersion.value] as const),
	queryFn: () => client.purpur.versions_v2.getBuilds(editingGameVersion.value),
	enabled: computed(() => editingPlatform.value === 'purpur' && !!editingGameVersion.value),
	staleTime: 5 * 60 * 1000,
})

const paperSupportedVersionsQuery = useQuery({
	queryKey: ['paper-supported-versions'] as const,
	queryFn: async () => {
		const project = await client.paper.versions_v3.getProject()
		return new Set(Object.values(project.versions).flat())
	},
	enabled: computed(() => editingPlatform.value === 'paper'),
	staleTime: 5 * 60 * 1000,
})

const purpurSupportedVersionsQuery = useQuery({
	queryKey: ['purpur-supported-versions'] as const,
	queryFn: async () => {
		const project = await client.purpur.versions_v2.getProject()
		return new Set(project.versions)
	},
	enabled: computed(() => editingPlatform.value === 'purpur'),
	staleTime: 5 * 60 * 1000,
})

function handleGameVersionHover(option: GameVersionOption) {
	if (editingPlatform.value === 'paper') {
		void queryClient.prefetchQuery({
			queryKey: ['paper-builds', option.value] as const,
			queryFn: () => client.paper.versions_v3.getBuilds(option.value),
			staleTime: 5 * 60 * 1000,
		})
	} else if (editingPlatform.value === 'purpur') {
		void queryClient.prefetchQuery({
			queryKey: ['purpur-builds', option.value] as const,
			queryFn: () => client.purpur.versions_v2.getBuilds(option.value),
			staleTime: 5 * 60 * 1000,
		})
	}
}

function getLoaderVersionsForGameVersion(
	loader: string,
	gameVersion: string,
): LoaderVersionEntry[] {
	if (loader === 'paper') {
		return (paperBuildsQuery.data.value?.builds ?? [])
			.toSorted((a, b) => b.id - a.id)
			.map((b): LoaderVersionEntry => {
				const u = String(b.channel).toUpperCase()
				let channelTag: LoaderVersionEntry['channelTag'] | undefined
				if (u === 'ALPHA' || u === 'BETA') channelTag = u
				return {
					id: String(b.id),
					stable: b.channel === 'STABLE',
					label: `Build ${b.id}`,
					channelTag,
				}
			})
	}
	if (loader === 'purpur') {
		return (purpurBuildsQuery.data.value?.builds.all ?? [])
			.toSorted((a, b) => parseInt(b) - parseInt(a))
			.map((b) => ({ id: b, stable: true }))
	}

	const manifest = manifestQuery.data.value?.gameVersions
	const versionGroups = manifestQuery.data.value?.versionGroups
	if (!manifest) return []

	const entry = manifest.find((x) => x.id === gameVersion)
	if (!entry) return []
	if (entry?.versionGroup) {
		return versionGroups?.find((group) => group.id === entry.versionGroup)?.loaders ?? []
	}

	const placeholder = manifest.find((x) => x.id === loaderGameVersionPlaceholder)
	if (placeholder) return placeholder.loaders

	return entry?.loaders ?? []
}

function supportsMinecraftServer(version: Labrinth.Tags.v2.GameVersion): boolean {
	return Date.parse(version.date) >= minecraftServerDownloadsStartTime
}

function getSupportedManifestGameVersions(): Set<string> | null {
	const manifest = manifestQuery.data.value?.gameVersions
	if (!manifest) return null

	const hasPlaceholder = manifest.some((entry) => entry.id === loaderGameVersionPlaceholder)
	return new Set(
		manifest
			.filter((entry) => entry.id !== loaderGameVersionPlaceholder)
			.filter((entry) => hasPlaceholder || entry.loaders.length > 0 || !!entry.versionGroup)
			.map((entry) => entry.id),
	)
}

type InstallationCacheSnapshot = { server: Server | undefined }

async function applyOptimisticInstallation(
	platform: string,
	gameVersion: string,
	loaderVersion: string | null,
): Promise<InstallationCacheSnapshot> {
	await queryClient.cancelQueries({ queryKey: serverDetailQueryKey, exact: true })

	const snapshot = { server: queryClient.getQueryData<Server>(serverDetailQueryKey) }
	const resolvedLoaderVersion = platform === 'vanilla' ? null : loaderVersion
	beginInstallation({
		type: 'platform',
		platform: platform as ServerPlatform,
		platform_version: resolvedLoaderVersion ?? '',
		game_version: gameVersion,
	})

	queryClient.setQueryData<Server>(serverDetailQueryKey, (current) =>
		current
			? {
					...current,
					status: 'installing',
					loader: serverLoader(platform),
					loader_version: resolvedLoaderVersion,
					mc_version: gameVersion,
				}
			: current,
	)

	return snapshot
}

function rollbackOptimisticInstallation(snapshot: InstallationCacheSnapshot) {
	cancelOptimisticInstallation()
	queryClient.setQueryData(serverDetailQueryKey, snapshot.server)
}

async function uploadLocalModpackWithSoftOverride(file: File) {
	modpackUploadProgress.value = { loaded: 0, total: 0, progress: 0 }
	try {
		await setUpServer(
			client,
			coreId,
			serverId,
			{ type: 'pack-file', file },
			{ onUploadProgress: (progress) => (modpackUploadProgress.value = progress) },
		)
	} catch (err) {
		emit('reinstall-failed')
		addNotification({
			type: 'error',
			text:
				err instanceof Error
					? err.message
					: formatMessage(
							modpackUploadAction.value === 'swap'
								? messages.failedToChangeVersion
								: messages.failedToReinstall,
						),
		})
		return
	} finally {
		modpackUploadProgress.value = null
	}
	modpackUploadModal.value?.hide()
	beginInstallation({
		type: 'local_modpack',
		filename: file.name,
	})
	emit('reinstall')
}

/** The server's own files that run on the server or players' games. */
async function activeContent() {
	const items = await core.call(instanceContent.ListContentItems, {
		params: { id: serverId },
		query: {},
	})
	return items.filter((item) => !isDisabledFor(item, 'server') || !isDisabledFor(item, 'player'))
}

async function disableAddonsEverywhere(items: EngineContentItem[]) {
	for (const item of items)
		for (const side of ['server', 'player'] as const)
			if (!isDisabledFor(item, side))
				await core.call(instanceContent.SetSide, {
					params: { id: serverId },
					body: { project_path: item.file_path, side, enabled: false },
				})
}

provideInstallationSettings({
	closeSettings: serverSettings.closeModal,
	afterSave: async () => {
		serverSettings.closeModal?.()
	},
	onGameVersionHover: handleGameVersionHover,
	loading: computed(() => packQuery.isLoading.value),
	installationInfo: computed(() => {
		const rawLoader = server.value.loader
		const loader = rawLoader ? formatLoaderLabel(rawLoader) : null
		const rows = [
			{ label: formatMessage(commonMessages.platformLabel), value: loader },
			{ label: formatMessage(commonMessages.gameVersionLabel), value: server.value.mc_version },
		]
		if (loader !== 'Vanilla') {
			rows.push({
				label: formatMessage(messages.loaderVersionLabel, { loader: loader ?? 'null' }),
				value: server.value.loader_version,
			})
		}
		return rows
	}),
	isLinked: computed(() => !!modpack.value || !!follow.value),
	isBusy: setupActionDisabled,
	busyMessage: setupActionDisabledMessage,
	isLinkedInstance: computed(() => !!follow.value),
	modpack: computed(() => {
		const followed = follow.value
		if (followed)
			return {
				iconUrl: null,
				title: followed.name,
				link: `/instance/${encodeURIComponent(followed.id)}`,
			}
		if (!modpack.value) return null
		const { project, version, owner } = modpack.value
		return {
			iconUrl: project.icon_url,
			title: project.title,
			link: `/project/${project.slug ?? project.id}`,
			versionNumber: version?.version_number ?? null,
			owner: owner
				? { id: owner.id, name: owner.name, iconUrl: owner.avatar_url, type: owner.type }
				: undefined,
		}
	}),
	currentPlatform: computed(() => server.value?.loader?.toLowerCase() ?? 'vanilla'),
	currentGameVersion: computed(() => server.value?.mc_version ?? ''),
	currentLoaderVersion: computed(() => server.value?.loader_version ?? ''),
	requiresInstallation,
	availablePlatforms: ['vanilla', 'fabric', 'neoforge', 'forge', 'quilt', 'paper'],

	editingPlatformRef: editingPlatform,
	editingGameVersionRef: editingGameVersion,

	resolveGameVersions(loader, showSnapshots) {
		const serverVersions = tags.gameVersions.value.filter(supportsMinecraftServer)
		const versions = showSnapshots
			? serverVersions
			: serverVersions.filter((v) => v.version_type === 'release')

		if (loader && loader !== 'vanilla') {
			if (loader === 'paper') {
				const supported = paperSupportedVersionsQuery.data.value
				if (!supported) return []
				return versions
					.filter((v) => supported.has(v.version))
					.map((v) => ({ value: v.version, label: v.version }))
			} else if (loader === 'purpur') {
				const supported = purpurSupportedVersionsQuery.data.value
				if (!supported) return []
				return versions
					.filter((v) => supported.has(v.version))
					.map((v) => ({ value: v.version, label: v.version }))
			} else {
				const supportedVersions = getSupportedManifestGameVersions()
				if (!supportedVersions) return []
				return versions
					.filter((v) => supportedVersions.has(v.version))
					.map((v) => ({ value: v.version, label: v.version }))
			}
		}

		return versions.map((v) => ({ value: v.version, label: v.version }))
	},

	resolveLoaderVersions(loader, gameVersion) {
		if (loader === 'vanilla' || !gameVersion) return []
		return getLoaderVersionsForGameVersion(loader, gameVersion)
	},

	resolveHasSnapshots(loader) {
		const serverVersions = tags.gameVersions.value.filter(supportsMinecraftServer)
		if (loader === 'vanilla') {
			return serverVersions.some((v) => v.version_type !== 'release')
		}
		if (loader === 'paper') {
			const supported = paperSupportedVersionsQuery.data.value
			if (!supported) return false
			return serverVersions.some((v) => v.version_type !== 'release' && supported.has(v.version))
		}
		if (loader === 'purpur') {
			const supported = purpurSupportedVersionsQuery.data.value
			if (!supported) return false
			return serverVersions.some((v) => v.version_type !== 'release' && supported.has(v.version))
		}
		const supportedVersions = getSupportedManifestGameVersions()
		if (!supportedVersions) return false
		const supported = serverVersions.filter((v) => supportedVersions.has(v.version))
		return supported.some((v) => v.version_type !== 'release')
	},

	async save(platform, gameVersion, loaderVersionId) {
		if (setupActionDisabled.value) return
		debug('save: called with', { platform, gameVersion, loaderVersionId })
		const currentPlatform = server.value?.loader?.toLowerCase() ?? 'vanilla'
		const platformChanged = platform !== currentPlatform
		const gameVersionChanged = gameVersion !== (server.value?.mc_version ?? '')
		const loaderVersionChanged =
			loaderVersionId !== null && loaderVersionId !== (server.value?.loader_version ?? '')
		// A Core installs the loader again for any change, keeping the server's content.
		if (
			!requiresInstallation.value &&
			!platformChanged &&
			!gameVersionChanged &&
			!loaderVersionChanged
		)
			return

		let resolvedLoaderVersion = loaderVersionId
		if (!resolvedLoaderVersion && platform !== 'vanilla') {
			const versions = getLoaderVersionsForGameVersion(platform, gameVersion)
			resolvedLoaderVersion = versions[0]?.id ?? null
		}

		const snapshot = await applyOptimisticInstallation(platform, gameVersion, resolvedLoaderVersion)
		emit('reinstall', { loader: platform, lVersion: resolvedLoaderVersion, mVersion: gameVersion })
		try {
			await setUpServer(client, coreId, serverId, {
				type: 'loader',
				loader: platform,
				loaderVersion: resolvedLoaderVersion,
				gameVersion,
			})
			debug('save: succeeded')
			serverSettings.closeModal?.()
		} catch (err) {
			debug('save: failed, emitting reinstall-failed', err)
			rollbackOptimisticInstallation(snapshot)
			emit('reinstall-failed')
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToSaveSettings),
			})
			throw err
		}
	},

	async repair() {
		if (setupActionDisabled.value) return
		debug('repair: called')
		beginInstallation({ type: 'unknown' })
		try {
			await core.call(instanceContent.RepairContent, { params: { id: serverId } })
			debug('repair: API succeeded')
			addNotification({
				type: 'success',
				title: formatMessage(messages.repairStartedTitle),
				text: formatMessage(messages.repairStartedText),
			})
		} catch (err) {
			debug('repair: failed', err)
			cancelOptimisticInstallation()
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToRepair),
			})
		}
	},

	async reinstallModpack() {
		if (setupActionDisabled.value) return
		const pack = modpack.value
		if (!pack?.version) return
		const projectId = pack.project.id
		const versionId = pack.version.id
		beginInstallation({ type: 'modrinth_modpack', project_id: projectId, version_id: versionId })
		emit('reinstall')
		try {
			await setUpServer(client, coreId, serverId, { type: 'modpack', projectId, versionId })
		} catch (err) {
			debug('reinstallModpack: failed, emitting reinstall-failed', err)
			cancelOptimisticInstallation()
			emit('reinstall-failed')
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToReinstall),
			})
		}
	},

	async swapModpack() {},

	async unlinkModpack() {
		if (setupActionDisabled.value) return
		debug('unlinkModpack: called')
		try {
			await core.call(instanceContent.UnlinkModpack, { params: { id: serverId } })
		} catch (err) {
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToUnlink),
			})
		} finally {
			await invalidateServerState()
		}
	},

	getCachedModpackVersions: () => modpackVersionsQuery.data.value ?? null,

	async fetchModpackVersions() {
		debug('fetchModpackVersions: called, project:', modpackProjectId.value)
		if (!modpackProjectId.value) throw new Error('No modpack project ID')
		try {
			const versions = await client.labrinth.versions_v2.getProjectVersions(
				modpackProjectId.value,
				{
					include_changelog: false,
				},
			)
			debug('fetchModpackVersions: got', versions.length, 'versions')
			return versions
		} catch (err) {
			debug('fetchModpackVersions: failed', err)
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToLoadVersions),
			})
			throw err
		}
	},

	async getVersionChangelog(versionId) {
		debug('getVersionChangelog: called, versionId:', versionId)
		try {
			return await client.labrinth.versions_v2.getVersion(versionId)
		} catch {
			debug('getVersionChangelog: failed for', versionId)
			return null
		}
	},

	async onModpackVersionConfirm(version) {
		if (setupActionDisabled.value) return
		if (!modpackProjectId.value) return
		debug('onModpackVersionConfirm: called, version:', version.id)
		debug('onModpackVersionConfirm: emitting reinstall before API call')
		beginInstallation({
			type: 'modrinth_modpack',
			project_id: modpackProjectId.value,
			version_id: version.id,
		})
		emit('reinstall')
		try {
			await core.call(instanceContent.UpdateModpackVersion, {
				params: { id: serverId },
				body: { version_id: version.id },
			})
			debug('onModpackVersionConfirm: installContent succeeded')
		} catch (err) {
			debug('onModpackVersionConfirm: failed, emitting reinstall-failed', err)
			cancelOptimisticInstallation()
			emit('reinstall-failed')
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToChangeVersion),
			})
		}
	},

	updaterModalProps: computed(() => ({
		isApp: serverSettings.isApp.value,
		currentVersionId: modpack.value?.version?.id ?? '',
		projectIconUrl: modpack.value?.project.icon_url ?? undefined,
		projectName: modpack.value?.project.title ?? formatMessage(commonMessages.modpackLabel),
		currentGameVersion: server.value.mc_version ?? '',
		currentLoader: server.value.loader ?? '',
	})),

	isServer: true,
	isApp: serverSettings.isApp.value,
	showModpackVersionActions: computed(() => !!modpack.value),
	isLocalFile: false,

	lockPlatform: false,
	hideLoaderVersion: false,

	async disableAllContent() {
		if (setupActionDisabled.value) return
		await disableAddonsEverywhere(await activeContent())
	},

	async disableIncompatibleContent(targetGameVersion) {
		if (setupActionDisabled.value) return
		const active = await activeContent()
		const versionIds = active.flatMap((item) => (item.version ? [item.version.id] : []))
		const versions =
			versionIds.length > 0 ? await client.labrinth.versions_v2.getVersions(versionIds) : []
		const compatible = new Set(
			versions.filter((v) => v.game_versions.includes(targetGameVersion)).map((v) => v.id),
		)
		// Files from outside Modrinth cannot be checked, so they are turned off too.
		await disableAddonsEverywhere(
			active.filter((item) => !item.version || !compatible.has(item.version.id)),
		)
	},

	async saveWithoutAutoFix(platform, gameVersion, loaderVersionId) {
		if (setupActionDisabled.value) return
		let resolvedLoaderVersion = loaderVersionId
		if (!resolvedLoaderVersion && platform !== 'vanilla') {
			const versions = getLoaderVersionsForGameVersion(platform, gameVersion)
			resolvedLoaderVersion = versions[0]?.id ?? null
		}
		const snapshot = await applyOptimisticInstallation(platform, gameVersion, resolvedLoaderVersion)
		emit('reinstall', { loader: platform, lVersion: resolvedLoaderVersion, mVersion: gameVersion })
		try {
			await setUpServer(client, coreId, serverId, {
				type: 'loader',
				loader: platform,
				loaderVersion: resolvedLoaderVersion,
				gameVersion,
			})
		} catch (err) {
			rollbackOptimisticInstallation(snapshot)
			emit('reinstall-failed')
			addNotification({
				type: 'error',
				text: err instanceof Error ? err.message : formatMessage(messages.failedToSaveSettings),
			})
			throw err
		}
	},
})

watch(
	() => server.value?.status,
	(newStatus, oldStatus) => {
		debug('status watcher:', oldStatus, '->', newStatus, {
			'server.loader': server.value?.loader,
			'server.mc_version': server.value?.mc_version,
			'server.loader_version': server.value?.loader_version,
		})
		if (oldStatus === 'installing' && newStatus === 'available') {
			debug('status installing->available, resetting editing refs')
			editingPlatform.value = server.value?.loader?.toLowerCase() ?? 'vanilla'
			editingGameVersion.value = server.value?.mc_version ?? ''
		}
	},
)

function onReinstall(event?: unknown) {
	if (resetServerDisabled.value && !installation.value) return
	installationSettingsLayout.value?.cancelEditing()
	modrinthServersConsole.clear()
	if (!installation.value) {
		const args = event as
			| { loader?: string; lVersion?: string; mVersion?: string | null }
			| undefined
		if (args?.loader && args.mVersion) {
			beginInstallation({
				type: 'platform',
				platform: args.loader as ServerPlatform,
				platform_version: args.lVersion ?? '',
				game_version: args.mVersion,
			})
		} else {
			beginInstallation({ type: 'unknown' })
		}
	}
	emit('reinstall', event)
	serverSettings.closeModal?.()
}

function onBrowseModpacks() {
	if (resetServerDisabled.value) return
	debug('onBrowseModpacks: navigating to modpack discovery')
	serverSettings.browseModpacks({
		serverId,
		from: 'reset-server',
	})
}
</script>
