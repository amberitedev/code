import { createCoreInstanceFromProfile } from '@amberite/amberite-api'
import type { AbstractWebNotificationManager } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { provide, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'
import { useRouter } from 'vue-router'

import type InstanceCreationFlowModal from '@/components/ui/creation-flow/InstanceCreationFlowModal.vue'
import type { InstanceCreationFlowContextValue } from '@/components/ui/creation-flow/types'
import type UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import type ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import { useCoreClient } from '@/composables/useCoreClient'
import { upsertCoreInstanceInCache } from '@/composables/useCoreInstances'
import { trackEvent } from '@/helpers/analytics'
import { get_project_versions, get_search_results } from '@/helpers/cache.js'
import { import_instance } from '@/helpers/import.js'
import {
	type CreatePackLocation,
	install_create_instance,
	install_create_modpack_instance,
	install_get_modpack_preview,
} from '@/helpers/install'
import { list as listInstances } from '@/helpers/instance'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata.js'
import { create, edit } from '@/helpers/profile.js'
import type { InstanceLoader } from '@/helpers/types'
import { registerSyncedProfileBackend } from '@/pages/instance/synced/synced-registration'
import { setLinkedServerId, setLinkedServerPath } from '@/pages/instance/synced/use-synced-link'
import { useTheming } from '@/store/state'

export function setupCreationModal(notificationManager: AbstractWebNotificationManager) {
	const { handleError } = notificationManager
	const router = useRouter()
	const queryClient = useQueryClient()
	const core = useCoreClient()
	const themeStore = useTheming()

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
		const [instances, coreInstances] = await Promise.all([
			listInstances().catch(handleError),
			core.listInstances().catch(() => []),
		])
		return [
			...(instances ?? [])
				.filter((instance) => instance.profile_type !== 'server')
				.map((instance) => instance.name),
			...coreInstances.map((instance) => instance.name),
		]
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
		await install_create_modpack_instance({
			type: 'fromVersionId',
			project_id: projectId,
			version_id: versionId,
			title: name,
			icon_url: iconUrl,
		}).catch(handleError)
		trackEvent('InstanceCreate', { source: 'CreationModalModpack' })
	}

	async function handleCreate(config: InstanceCreationFlowContextValue) {
		try {
			if (config.modpackSelection.value) {
				const { projectId, versionId, name, iconUrl } = config.modpackSelection.value

				const instances = await listInstances().catch(handleError)
				const existingInstance = instances?.find((i) => i.link?.project_id === projectId)

				if (existingInstance && !themeStore.getFeatureFlag('skip_non_essential_warnings')) {
					pendingModpackCreation.value = { projectId, versionId, name, iconUrl }
					installationModal.value?.hide()
					modpackAlreadyInstalledModal.value?.show(existingInstance.name, existingInstance.id)
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
				const location: CreatePackLocation = {
					type: 'fromFile',
					path: config.modpackFilePath.value,
				}
				const preview = await install_get_modpack_preview(location)

				if (preview.unknownFile || preview.externalFilesInModpack.length > 0) {
					const splitPath = config.modpackFilePath.value.split(/[\\/]/)
					const fileName = splitPath
						? splitPath[splitPath.length - 1]
						: config.modpackFilePath.value
					if (unknownPackWarningModal.value) {
						unknownPackWarningModal.value?.show(
							() => install_create_modpack_instance(location).then(() => undefined),
							fileName,
							preview.externalFilesInModpack,
						)
					} else {
						await install_create_modpack_instance(location)
					}
				} else {
					await install_create_modpack_instance(location)
				}
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
				const profile = {
					name,
					gameVersion: config.selectedGameVersion.value!,
					modloader: loader,
					loaderVersion,
				}
				const coreInstance = await createCoreInstanceFromProfile(core, {
					profile,
				})
				upsertCoreInstanceInCache(queryClient, coreInstance)
				await router.push(`/instance/${encodeURIComponent(coreInstance.path)}`)
			} else if (config.instanceType.value === 'synced') {
				const coreInstance = await createCoreInstanceFromProfile(core, {
					profile: {
						name,
						gameVersion: config.selectedGameVersion.value!,
						modloader: loader,
						loaderVersion,
					},
				})
				upsertCoreInstanceInCache(queryClient, coreInstance)
				const profilePath = await create(
					name,
					config.selectedGameVersion.value!,
					toProfileLoader(loader),
					loaderVersion,
					iconPath,
					false,
					null,
					'synced',
				)
				await edit(profilePath, {
					name,
					profile_type: 'synced',
					core_instance_id: coreInstance.id,
				})
				setLinkedServerId(profilePath, coreInstance.id)
				setLinkedServerPath(profilePath, coreInstance.path)
				await registerSyncedProfileBackend({
					profilePath,
					serverInstanceId: coreInstance.id,
					instance: {
						name,
						game_version: config.selectedGameVersion.value!,
						loader: toProfileLoader(loader),
					},
				})
				await router.push(`/instance/${encodeURIComponent(profilePath)}`)
			} else {
				await install_create_instance({
					name,
					gameVersion: config.selectedGameVersion.value!,
					loader: loader as InstanceLoader,
					loaderVersion,
					iconPath,
				}).catch(handleError)
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

	function handleModpackDuplicateGoToInstance(instanceId: string) {
		pendingModpackCreation.value = null
		router.push(`/instance/${encodeURIComponent(instanceId)}/`)
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
