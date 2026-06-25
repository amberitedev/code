import { createCoreInstanceFromProfile } from '@amberite/amberite-api'
import type {
	AbstractPopupNotificationManager,
	AbstractWebNotificationManager,
} from '@modrinth/ui'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { provide, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'
import { useRouter } from 'vue-router'

import type InstanceCreationFlowModal from '@/components/ui/creation-flow/InstanceCreationFlowModal.vue'
import type { InstanceCreationFlowContextValue } from '@/components/ui/creation-flow/types'
import type UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import type ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import { useCoreClient } from '@/composables/useCoreClient'
import { trackEvent } from '@/helpers/analytics'
import { get_project_versions, get_search_results } from '@/helpers/cache.js'
import { import_instance } from '@/helpers/import.js'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata.js'
import { create_profile_and_install, create_profile_and_install_from_file } from '@/helpers/pack'
import { create, edit, list } from '@/helpers/profile.js'
import type { InstanceLoader } from '@/helpers/types'
import { registerSyncedProfileBackend } from '@/pages/instance/synced/synced-registration'

export function setupCreationModal(
	notificationManager: AbstractWebNotificationManager,
	popupNotificationManager: AbstractPopupNotificationManager,
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
	})

	const installationModal =
		useTemplateRef<ComponentExposed<typeof InstanceCreationFlowModal>>('installationModal')
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
	) {
		await create_profile_and_install(projectId, versionId, name, iconUrl).catch(handleError)
		trackEvent('InstanceCreate', { source: 'CreationModalModpack' })
	}

	async function handleCreate(config: InstanceCreationFlowContextValue) {
		try {
			if (config.modpackSelection.value) {
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
				await proceedWithModpackCreation(projectId, versionId, name, iconUrl)
				return
			}

			if (config.modpackFilePath.value) {
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
				).catch(handleError)
				popupNotificationManager.removeNotification(waitingNotification.id)
				trackEvent('InstanceCreate', { source: 'CreationModalModpackFile' })
				return
			}

			// Custom/vanilla setup
			const loader = config.hideLoaderChips.value
				? 'vanilla'
				: (config.selectedLoader.value ?? 'vanilla')
			const loaderVersion = config.hideLoaderVersion.value
				? null
				: (config.selectedLoaderVersion.value ?? config.loaderVersionType.value)
			const iconPath = config.instanceIconPath.value ?? null
			const name = config.instanceName.value.trim() || config.autoInstanceName.value

			if (config.instanceType.value === 'server') {
				const core = useCoreClient()
				const profile = {
					name,
					gameVersion: config.selectedGameVersion.value!,
					modloader: loader,
					loaderVersion,
				}
				const coreInstance = await createCoreInstanceFromProfile(core, {
					profile,
				})
				const profileSlug = await getAvailableServerProfileSlug(name)
				const profilePath = await create(
					profileSlug,
					config.selectedGameVersion.value!,
					toProfileLoader(loader),
					loaderVersion,
					iconPath,
					true,
					null,
					'server',
				)
				await edit(profilePath, {
					name,
					profile_type: 'server',
					install_stage: 'installed',
					core_instance_id: coreInstance.id,
					server_manifest_json: {
						...profile,
						port: coreInstance.port,
						memory: coreInstance.memory,
					},
				})
				await router.push(`/instance/${encodeURIComponent(profilePath)}`)
			} else if (config.instanceType.value === 'synced') {
				const core = useCoreClient()
				const coreInstance = await createCoreInstanceFromProfile(core, {
					profile: {
						name,
						gameVersion: config.selectedGameVersion.value!,
						modloader: loader,
						loaderVersion,
					},
				})
				await create(
					coreInstance.id,
					config.selectedGameVersion.value!,
					toProfileLoader(loader),
					loaderVersion,
					iconPath,
					false,
					null,
					'synced',
				)
				await edit(coreInstance.id, {
					name,
					profile_type: 'synced',
				})
				await registerSyncedProfileBackend({
					profilePath: coreInstance.id,
					serverInstanceId: coreInstance.id,
					instance: {
						name,
						game_version: config.selectedGameVersion.value!,
						loader: toProfileLoader(loader),
					},
				})
				await router.push(`/instance/${encodeURIComponent(coreInstance.id)}`)
			} else {
				await create(
					name,
					config.selectedGameVersion.value!,
					toProfileLoader(loader),
					loaderVersion,
					iconPath,
					false,
					null,
					'client',
				).catch(handleError)
			}

			trackEvent('InstanceCreate', {
				source: 'CreationModal',
			})
		} catch (err) {
			handleError(err as Error)
		}
	}

	function toProfileLoader(loader: string): InstanceLoader {
		if (loader === 'paper' || loader === 'purpur') return 'vanilla'
		return loader as InstanceLoader
	}

	function slugServerProfileName(value: string): string {
		const slug = value
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
		return slug || 'server'
	}

	async function getAvailableServerProfileSlug(value: string): Promise<string> {
		const base = slugServerProfileName(value)
		const existing = new Set((await list().catch(handleError))?.map((instance) => instance.path) ?? [])
		if (!existing.has(base)) return base
		let suffix = 2
		while (existing.has(`${base}-${suffix}`)) suffix += 1
		return `${base}-${suffix}`
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
		await proceedWithModpackCreation(projectId, versionId, name, iconUrl)
	}

	function handleModpackDuplicateGoToInstance(instancePath: string) {
		pendingModpackCreation.value = null
		router.push(`/instance/${encodeURIComponent(instancePath)}/`)
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
