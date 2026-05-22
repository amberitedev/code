import type { CoreInstanceStateManager, CoreInstanceSummary } from '@amberite/amberite-api'
import type {
	AbstractPopupNotificationManager,
	AbstractWebNotificationManager,
	CreationFlowContextValue,
	CreationFlowModal,
} from '@modrinth/ui'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { provide, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'
import { useRouter } from 'vue-router'

import type UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import type ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { get_project_versions, get_search_results } from '@/helpers/cache.js'
import { import_instance } from '@/helpers/import.js'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata.js'
import { create_profile_and_install, create_profile_and_install_from_file } from '@/helpers/pack'
import { create, list } from '@/helpers/profile.js'
import type { InstanceLoader, ProfileKind } from '@/helpers/types'

const coreLoaderOptions = ['vanilla', 'paper', 'fabric', 'forge', 'neoforge', 'quilt'] as const
type CoreCreationLoader = (typeof coreLoaderOptions)[number]

export function setupCreationModal(
	notificationManager: AbstractWebNotificationManager,
	popupNotificationManager: AbstractPopupNotificationManager,
	coreInstances: CoreInstanceStateManager,
) {
	const { handleError } = notificationManager
	const { formatMessage } = useVIntl()
	const router = useRouter()

	const messages = defineMessages({
		installingModpackTitle: {
			id: 'app.creation-modal.installing-modpack.title',
			defaultMessage: 'Installing modpack...',
		},
		installingModpackDescription: {
			id: 'app.creation-modal.installing-modpack.description',
			defaultMessage: '{fileName}',
		},
		importedModpackServerUnsupported: {
			id: 'app.creation-modal.imported-modpack-server-unsupported',
			defaultMessage: 'Imported modpack files can only create client instances for now.',
		},
		importServerUnsupported: {
			id: 'app.creation-modal.import-server-unsupported',
			defaultMessage: 'Importing as a server or synced instance is not supported yet.',
		},
	})

	const installationModal =
		useTemplateRef<ComponentExposed<typeof CreationFlowModal>>('installationModal')
	const unknownPackWarningModal =
		useTemplateRef<InstanceType<typeof UnknownPackWarningModal>>('unknownPackWarningModal')
	const modpackAlreadyInstalledModal = ref<InstanceType<typeof ModpackAlreadyInstalledModal>>()

	function setModpackAlreadyInstalledModal(
		modal: InstanceType<typeof ModpackAlreadyInstalledModal>,
	) {
		modpackAlreadyInstalledModal.value = modal
	}

	async function fetchExistingInstanceNames(): Promise<string[]> {
		const instances = await list().catch(handleError)
		return instances?.map((i) => i.name) ?? []
	}

	provide('showCreationModal', () => {
		installationModal.value?.show()
	})

	async function proceedWithModpackCreation(
		projectId: string,
		versionId: string,
		name: string,
		iconUrl?: string,
		kind: ProfileKind = 'client',
	) {
		const versions = await get_project_versions(projectId)
		const version = versions?.find((candidate) => candidate.id === versionId)
		const gameVersion = version?.game_versions?.[0]
		const versionLoaders = [...(version?.mrpack_loaders ?? []), ...(version?.loaders ?? [])]
		if (!gameVersion) throw new Error('Modpack version is missing a Minecraft version')

		const serverLoader = getServerLoader(versionLoaders)
		const profileLoader = getProfileLoader(serverLoader)

		let coreInstance: CoreInstanceSummary | null = null
		if (kind !== 'client') {
			coreInstance = await coreInstances.create({
				name,
				game_version: gameVersion,
				loader: serverLoader,
				loader_version: undefined,
				port: 25565,
			})
		}

		let profilePath = ''
		try {
			if (coreInstance) {
				await coreInstances.client.installModpackVersion(coreInstance.id, projectId, versionId)
			}

			if (kind === 'client') {
				await create_profile_and_install(
					projectId,
					versionId,
					name,
					iconUrl,
					(profile) => {
						profilePath = profile
					},
					{
						kind,
						coreInstanceId: null,
						port: 25565,
						installLocalPack: true,
					},
				)
			} else {
				profilePath = await create(
					name,
					gameVersion,
					profileLoader,
					null,
					iconUrl ?? null,
					true,
					{ project_id: projectId, version_id: versionId, locked: true },
					kind,
					25565,
					coreInstance?.id ?? null,
				)
			}
		} catch (err) {
			if (coreInstance) await coreInstances.delete(coreInstance.id).catch(() => {})
			throw err
		}
		if (profilePath) await router.push(`/instance/${encodeURIComponent(profilePath)}/content`)
		trackEvent('InstanceCreate', { source: 'CreationModalModpack' })
	}

	function getServerLoader(loaders: string[]): CoreCreationLoader {
		return coreLoaderOptions.find((loader) => loaders.includes(loader)) ?? 'fabric'
	}

	function getProfileLoader(loader: string): InstanceLoader {
		if (loader === 'paper' || loader === 'purpur') return 'vanilla'
		return loader as InstanceLoader
	}

	async function handleCreate(config: CreationFlowContextValue) {
		try {
			const kind = config.instanceKind.value as ProfileKind

			if (config.modpackSelection.value && kind === 'client') {
				const { projectId, versionId, name, iconUrl } = config.modpackSelection.value

				const instances = await list().catch(handleError)
				const existingInstance = instances?.find((i) => i.linked_data?.project_id === projectId)

				if (existingInstance) {
					pendingModpackCreation.value = { projectId, versionId, name, iconUrl }
					installationModal.value?.hide()
					modpackAlreadyInstalledModal.value?.show(existingInstance.name, existingInstance.path)
					return
				}
			}

			installationModal.value?.hide()

			if (config.isImportMode.value) {
				if (kind !== 'client') {
					throw new Error(formatMessage(messages.importServerUnsupported))
				}
				for (const [launcherName, instanceSet] of Object.entries(
					config.importSelectedInstances.value,
				)) {
					const launcher = config.importLaunchers.value.find((l) => l.name === launcherName)
					if (!launcher || instanceSet.size === 0) continue
					for (const name of instanceSet) {
						await import_instance(launcher.name, launcher.path, name).catch(handleError)
					}
				}
				trackEvent('InstanceCreate', { source: 'CreationModalImport' })
				return
			}

			if (config.modpackSelection.value) {
				const { projectId, versionId, name, iconUrl } = config.modpackSelection.value
				await proceedWithModpackCreation(projectId, versionId, name, iconUrl, kind)
				return
			}

			if (config.modpackFilePath.value) {
				if (kind !== 'client') {
					throw new Error(formatMessage(messages.importedModpackServerUnsupported))
				}

				const waitingNotification = popupNotificationManager.addPopupNotification({
					title: formatMessage(messages.installingModpackTitle),
					text: formatMessage(messages.installingModpackDescription, {
						fileName: config.modpackFilePath.value.split('/').pop() ?? config.modpackFilePath.value,
					}),
					type: 'info',
					autoCloseMs: null,
					waiting: true,
				})

				await create_profile_and_install_from_file(
					config.modpackFilePath.value,
					(createProfile, fileName) => {
						popupNotificationManager.removeNotification(waitingNotification.id)
						unknownPackWarningModal.value?.show(createProfile, fileName)
					},
					{
						kind,
						coreInstanceId: null,
						port: 25565,
					},
				).catch(handleError)
				popupNotificationManager.removeNotification(waitingNotification.id)
				trackEvent('InstanceCreate', { source: 'CreationModalModpackFile' })
				return
			}

			// Custom/vanilla setup
			const loader = config.hideLoaderChips.value
				? 'vanilla'
				: (config.selectedLoader.value ?? 'vanilla')
			const profileLoader = getProfileLoader(loader)
			const loaderVersion = config.hideLoaderVersion.value
				? null
				: (config.selectedLoaderVersion.value ?? config.loaderVersionType.value)
			const iconPath = config.instanceIconPath.value ?? null
			const name = config.instanceName.value.trim() || config.autoInstanceName.value
			const skipInstall = false

			const coreInstance =
				kind === 'server' || kind === 'synced'
					? await coreInstances.create({
							name,
							game_version: config.selectedGameVersion.value!,
							loader: loader as CoreCreationLoader,
							loader_version: loaderVersion ?? undefined,
							port: 25565,
						})
					: null

			let profilePath = ''
			try {
				profilePath = await create(
					name,
					config.selectedGameVersion.value!,
					profileLoader,
					loaderVersion,
					iconPath,
					skipInstall,
					undefined,
					kind,
					25565,
					coreInstance?.id ?? null,
				)
			} catch (err) {
				if (coreInstance) {
					await coreInstances.delete(coreInstance.id).catch(() => {})
				}
				throw err
			}

			await router.push(`/instance/${encodeURIComponent(profilePath)}/content`)

			trackEvent('InstanceCreate', {
				source: 'CreationModal',
			})
		} catch (err) {
			handleError(err as Error)
		}
	}

	const pendingModpackCreation = ref<{
		projectId: string
		versionId: string
		name: string
		iconUrl?: string
	} | null>(null)

	async function handleModpackDuplicateCreateAnyway() {
		if (!pendingModpackCreation.value) return
		const { projectId, versionId, name, iconUrl } = pendingModpackCreation.value
		pendingModpackCreation.value = null
		await proceedWithModpackCreation(projectId, versionId, name, iconUrl, 'client')
	}

	function handleModpackDuplicateGoToInstance(instancePath: string) {
		pendingModpackCreation.value = null
		router.push(`/instance/${encodeURIComponent(instancePath)}/content`)
	}

	function handleBrowseModpacks() {
		installationModal.value?.hide()
		router.push('/browse/modpack')
	}

	async function searchModpacks(query: string, limit: number = 10) {
		const params = [`facets=[["project_type:modpack"]]`, `limit=${limit}`]
		if (query) {
			params.push(`query=${encodeURIComponent(query)}`)
		}
		const raw = await get_search_results(`?${params.join('&')}`)
		if (raw?.result) return raw.result
		return { hits: [], offset: 0, limit, total_hits: 0 }
	}

	async function getProjectVersions(projectId: string) {
		const versions = await get_project_versions(projectId)
		return versions ?? []
	}

	return {
		installationModal,
		unknownPackWarningModal,
		fetchExistingInstanceNames,
		handleCreate,
		handleBrowseModpacks,
		searchModpacks,
		getProjectVersions,
		getLoaderManifest,
		setModpackAlreadyInstalledModal,
		handleModpackDuplicateCreateAnyway,
		handleModpackDuplicateGoToInstance,
	}
}
