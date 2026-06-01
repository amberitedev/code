<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
	commonMessages,
	defineMessages,
	formatLoaderLabel,
	injectNotificationManager,
	InstallationSettingsLayout,
	provideInstallationSettings,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { get_project_versions, get_version } from '@/helpers/cache'

import { injectCoreServerContext } from '../../core-server-instance'
import { injectServerSettings } from '../server-settings'
import { useCoreInstallationMetadata } from '../use-core-installation-metadata'

const core = useCoreClient()
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const queryClient = useQueryClient()
const { rawInstance, instanceId, changeVersion, repairServer } = injectCoreServerContext()
const serverSettings = injectServerSettings()

const instance = computed(() => rawInstance.value)

const { resolveGameVersions, resolveLoaderVersions, resolveHasSnapshots } =
	useCoreInstallationMetadata(handleError)

const modpackKey = computed(() => ['core-modpack', instanceId.value])
const { data: modpack } = useQuery({
	queryKey: modpackKey,
	queryFn: () => core.getModpack(instanceId.value),
	enabled: computed(() => !!instanceId.value),
})

const repairing = ref(false)
const reinstalling = ref(false)

const messages = defineMessages({
	loaderVersion: {
		id: 'server.settings.tabs.installation.loader-version',
		defaultMessage: '{loader} version',
	},
})

provideInstallationSettings({
	closeSettings: () => serverSettings.closeModal?.(),
	loading: computed(() => !instance.value),
	installationInfo: computed(() => {
		const inst = instance.value
		if (!inst) return []
		const rows = [
			{
				label: formatMessage(commonMessages.platformLabel),
				value: formatLoaderLabel(inst.loader),
			},
			{ label: formatMessage(commonMessages.gameVersionLabel), value: inst.game_version },
		]
		if (inst.loader !== 'vanilla' && inst.loader_version) {
			rows.push({
				label: formatMessage(messages.loaderVersion, { loader: formatLoaderLabel(inst.loader) }),
				value: inst.loader_version,
			})
		}
		return rows
	}),
	isLinked: computed(() => !!modpack.value),
	isBusy: computed(
		() => instance.value?.install_status === 'installing' || repairing.value || reinstalling.value,
	),
	modpack: computed(() => {
		const pack = modpack.value
		if (!pack) return null
		return {
			iconUrl: undefined,
			title: pack.pack_name,
			link: pack.modrinth_project_id ? `/project/${pack.modrinth_project_id}` : undefined,
			versionNumber: pack.pack_version,
		}
	}),
	currentPlatform: computed(() => instance.value?.loader ?? 'vanilla'),
	currentGameVersion: computed(() => instance.value?.game_version ?? ''),
	currentLoaderVersion: computed(() => instance.value?.loader_version ?? ''),
	availablePlatforms: ['vanilla', 'paper', 'fabric', 'forge', 'neoforge', 'quilt'],

	resolveGameVersions,
	resolveLoaderVersions,
	resolveHasSnapshots,

	async save(platform, gameVersion, loaderVersionId) {
		await changeVersion({
			game_version: gameVersion,
			loader: platform,
			loader_version:
				platform === 'vanilla' || platform === 'paper' ? null : loaderVersionId || null,
		})
	},

	async repair() {
		repairing.value = true
		await repairServer()
		repairing.value = false
	},

	async reinstallModpack() {
		const pack = modpack.value
		if (!pack?.modrinth_project_id || !pack.modrinth_version_id) return
		reinstalling.value = true
		await core
			.installModpackVersion(instanceId.value, pack.modrinth_project_id, pack.modrinth_version_id)
			.catch(handleError)
		reinstalling.value = false
		await queryClient.invalidateQueries({ queryKey: modpackKey.value })
	},

	async unlinkModpack() {
		await core.removeModpack(instanceId.value).catch(handleError)
		await queryClient.invalidateQueries({ queryKey: modpackKey.value })
	},

	getCachedModpackVersions: () => null,
	async fetchModpackVersions() {
		const projectId = modpack.value?.modrinth_project_id
		if (!projectId) return []
		const versions = await get_project_versions(projectId).catch(handleError)
		return (versions ?? []) as Labrinth.Versions.v2.Version[]
	},

	async getVersionChangelog(versionId: string) {
		return (await get_version(versionId, 'must_revalidate').catch(
			() => null,
		)) as Labrinth.Versions.v2.Version | null
	},

	async onModpackVersionConfirm(version) {
		const projectId = modpack.value?.modrinth_project_id
		if (!projectId) return
		await core.installModpackVersion(instanceId.value, projectId, version.id).catch(handleError)
		await queryClient.invalidateQueries({ queryKey: modpackKey.value })
	},

	updaterModalProps: computed(() => ({
		isApp: true,
		currentVersionId: modpack.value?.modrinth_version_id ?? '',
		projectIconUrl: undefined,
		projectName: modpack.value?.pack_name ?? 'Modpack',
		currentGameVersion: instance.value?.game_version ?? '',
		currentLoader: instance.value?.loader ?? 'vanilla',
	})),

	isServer: true,
	isApp: true,
	showModpackVersionActions: computed(() => !!modpack.value),
	repairing,
	reinstalling,
})
</script>

<template>
	<InstallationSettingsLayout />
</template>
