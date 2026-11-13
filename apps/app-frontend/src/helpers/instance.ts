/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import {
	ApiCallError,
	type ContentItem as ContentItemResponse,
	ContractClient,
	desktop,
	type GlobalSyncedOptions,
	type IconConfig,
	instanceContent,
	instancePaths,
	instances,
	instanceSharing,
	type Labrinth,
	launch,
	library,
	localSharing,
	type ProjectType,
	type ScreenshotFile,
	type ScreenshotGroup,
	type ScreenshotGroupImport,
	type ScreenshotGroupMembershipUpdate,
	type ScreenshotKey,
	screenshots,
	type SharedInstanceInvite,
	type SharedInstanceInviteLink,
	type SharedInstancePublishPreview,
	type SharedInstanceUsers,
	type SyncedOption,
	type SyncedOptionJoinPreview,
	type SyncedOptionJoinResolution,
	type SyncedOptionsOverview,
	syncedOptions,
	type SyncedServer,
} from '@modrinth/api-client'
import { type ContentItem, type ContentOwner, getIconSymbolAsset } from '@modrinth/ui'

import { localFileUrl, localTransport } from '@/platform/local'
import { apiClient } from '@/platform/modrinth-client'
import { runOp } from '@/platform/ops'

import type { InstallJobSnapshot } from './install'
import type {
	CacheBehaviour,
	ContentFile,
	ContentFileProjectType,
	GameInstance,
	InstanceLoader,
	SharedInstanceAttachment,
} from './types'

export function getInstanceIconUrl(iconPath: string | null | undefined): string | null {
	if (!iconPath) return null
	if (iconPath.startsWith('http://') || iconPath.startsWith('https://')) return iconPath
	return localFileUrl(iconPath)
}

export async function remove(instanceId: string): Promise<void> {
	return await apiClient.local.call(instances.RemoveInstance, { params: { id: instanceId } })
}

export async function get(instanceId: string): Promise<GameInstance | null> {
	return await apiClient.local.call(instances.GetInstance, { params: { id: instanceId } })
}

export async function get_many(instanceIds: string[]): Promise<GameInstance[]> {
	return await apiClient.local.call(instances.GetInstances, {
		query: { ids: JSON.stringify(instanceIds) },
	})
}

export async function get_projects(
	instanceId: string,
	cacheBehaviour?: CacheBehaviour,
): Promise<Record<string, ContentFile>> {
	return await apiClient.local.call(instanceContent.ListInstanceProjects, {
		params: { id: instanceId },
		query: { cacheBehaviour },
	})
}

export async function get_installed_project_ids(instanceId: string): Promise<string[]> {
	return await apiClient.local.call(instanceContent.ListInstalledProjectIds, {
		params: { id: instanceId },
	})
}

export type InstanceInstallTarget = {
	game_version: string
	loader: string
}

export type InstanceInstallCandidate = {
	id: string
	name: string
	icon_path?: string | null
	game_version: string
	loader: InstanceLoader
	installed: boolean
	compatible: boolean
}

export async function get_install_candidates(
	projectId: string,
	projectType: string,
	targets: InstanceInstallTarget[],
): Promise<InstanceInstallCandidate[]> {
	return await apiClient.local.call(instanceContent.ListInstallCandidates, {
		body: { project_id: projectId, project_type: projectType as ProjectType, targets },
	})
}

// Get content items with rich metadata for an instance
// Returns content items filtered to exclude modpack files (if linked),
// sorted alphabetically by project name
export async function get_content_items(
	instanceId: string,
	cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
	const items = await apiClient.local.call(instanceContent.ListContentItems, {
		params: { id: instanceId },
		query: { cacheBehaviour },
	})
	return adaptContentItems(items)
}

export async function refresh_content_updates(instanceId: string): Promise<void> {
	return await apiClient.local.call(instanceContent.RefreshContentUpdates, {
		params: { id: instanceId },
	})
}

export async function sync_content_files(instanceId: string): Promise<void> {
	return await apiClient.local.call(instanceContent.SyncContentFiles, {
		params: { id: instanceId },
	})
}

// Linked modpack info returned from backend
export interface LinkedModpackInfo {
	project: Labrinth.Projects.v2.Project
	version: Labrinth.Versions.v2.Version | null
	owner: ContentOwner | null
	has_update: boolean
	update_version_id: string | null
	update_version: Labrinth.Versions.v2.Version | null
}

// Get linked modpack info for an instance
// Returns project, version, and owner information for the linked modpack,
// or null if the instance is not linked to a modpack
export async function get_linked_modpack_info(
	instanceId: string,
	cacheBehaviour?: CacheBehaviour,
): Promise<LinkedModpackInfo | null> {
	return (await apiClient.local.call(instanceContent.GetLinkedModpack, {
		params: { id: instanceId },
		query: { cacheBehaviour },
	})) as LinkedModpackInfo | null
}

// Get content items that are part of the linked modpack
// Returns the modpack's dependencies as ContentItem list
// Returns empty array if the instance is not linked to a modpack
export async function get_linked_modpack_content(
	instanceId: string,
	cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
	const items = await apiClient.local.call(instanceContent.ListLinkedModpackContent, {
		params: { id: instanceId },
		query: { cacheBehaviour },
	})
	return adaptContentItems(items)
}

// Convert a list of dependencies into ContentItems with rich metadata
export async function get_dependencies_as_content_items(
	dependencies: Labrinth.Versions.v3.Dependency[],
	cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
	const items = await apiClient.local.call(instanceContent.ListDependencyContentItems, {
		body: {
			dependencies: dependencies.map((dependency) => ({
				project_id: null,
				version_id: null,
				file_name: null,
				...dependency,
			})),
			cacheBehaviour,
		},
	})
	return adaptContentItems(items)
}

/** The engine sends `null` for a field the UI's `ContentItem` leaves out. */
export function adaptContentItems(items: ContentItem[] | ContentItemResponse[]): ContentItem[] {
	return (items as ContentItem[]).map((item) => {
		const embeddedMetadata = item.embedded_metadata
		if (!embeddedMetadata?.icon_path) return item

		return {
			...item,
			embedded_metadata: {
				...embeddedMetadata,
				icon_url: localFileUrl(embeddedMetadata.icon_path),
			},
		}
	})
}

export async function get_full_path(instanceId: string): Promise<string> {
	return await apiClient.local.call(instancePaths.getInstancePath, { params: { id: instanceId } })
}

export async function get_mod_full_path(instanceId: string, projectPath: string): Promise<string> {
	return await apiClient.local.call(instancePaths.getInstanceContentPath, {
		params: { id: instanceId },
		query: { project_path: projectPath },
	})
}

export type {
	GlobalSyncedOptions,
	ScreenshotGroup,
	ScreenshotGroupImport,
	ScreenshotGroupMembershipUpdate,
	ScreenshotKey,
	SyncedOption,
	SyncedOptionCapability,
	SyncedOptionJoinAction,
	SyncedOptionJoinPreview,
	SyncedOptionJoinResolution,
	SyncedOptionsOverview,
	SyncedServer,
} from '@modrinth/api-client'

export type InstanceScreenshot = ScreenshotFile & {
	url: string
}

function withUrl(screenshot: ScreenshotFile): InstanceScreenshot {
	return { ...screenshot, url: localFileUrl(screenshot.path, screenshot.modified_at) }
}

export async function list_instance_screenshots(instanceId: string): Promise<InstanceScreenshot[]> {
	const files = await apiClient.local.call(screenshots.listInstanceScreenshots, {
		params: { id: instanceId },
	})
	return files.map(withUrl)
}

export async function list_all_screenshots(): Promise<InstanceScreenshot[]> {
	const files = await apiClient.local.call(screenshots.listScreenshots)
	return files.map(withUrl)
}

export async function list_synced_screenshots(): Promise<InstanceScreenshot[]> {
	const files = await apiClient.local.call(screenshots.listSyncedScreenshots)
	return files.map(withUrl)
}

export async function save_edited_screenshot(
	key: ScreenshotKey,
	pngBytes: Uint8Array,
	mode: 'create_copy' | 'replace_edit',
): Promise<InstanceScreenshot> {
	const screenshot = await apiClient.local.call(screenshots.saveEditedScreenshot, {
		query: { ...key, mode },
		body: new Blob([pngBytes.slice()], { type: 'image/png' }),
	})
	return withUrl(screenshot)
}

export async function list_screenshot_groups(): Promise<ScreenshotGroup[]> {
	return await apiClient.local.call(screenshots.listScreenshotGroups)
}

export async function create_screenshot_group(
	name: string,
	screenshotIds: string[],
): Promise<ScreenshotGroup> {
	return await apiClient.local.call(screenshots.createScreenshotGroup, {
		body: { name, screenshot_ids: screenshotIds },
	})
}

export async function rename_screenshot_group(
	id: string,
	newName: string,
): Promise<ScreenshotGroup> {
	return await apiClient.local.call(screenshots.renameScreenshotGroup, {
		params: { id },
		body: { name: newName },
	})
}

export async function delete_screenshot_group(id: string): Promise<void> {
	return await apiClient.local.call(screenshots.deleteScreenshotGroup, { params: { id } })
}

export async function set_screenshot_group_memberships(
	updates: ScreenshotGroupMembershipUpdate[],
): Promise<void> {
	return await apiClient.local.call(screenshots.setScreenshotGroupMemberships, { body: updates })
}

export async function import_screenshot_groups(groups: ScreenshotGroupImport[]): Promise<void> {
	return await apiClient.local.call(screenshots.importScreenshotGroups, { body: groups })
}

export async function delete_screenshots(keys: ScreenshotKey[]): Promise<void> {
	return await apiClient.local.call(screenshots.deleteScreenshots, { body: keys })
}

export async function export_screenshots(keys: ScreenshotKey[], exportPath: string): Promise<void> {
	return await apiClient.local.call(screenshots.exportScreenshots, {
		body: { keys, export_path: exportPath },
	})
}

export async function move_screenshots(
	keys: ScreenshotKey[],
	targetInstanceId: string,
): Promise<ScreenshotKey[]> {
	return await apiClient.local.call(screenshots.moveScreenshots, {
		body: { keys, target_instance_id: targetInstanceId },
	})
}

export async function open_screenshot(key: ScreenshotKey): Promise<void> {
	return await new ContractClient(localTransport).call(desktop.openFolder, {
		body: { folder: 'screenshot', ...key },
	})
}

export async function set_synced_option(
	instanceId: string,
	option: SyncedOption,
	enabled: boolean,
	resolution?: SyncedOptionJoinResolution,
): Promise<GameInstance> {
	await apiClient.local.call(syncedOptions.setInstanceSyncedOption, {
		params: { id: instanceId },
		body: { option, enabled, resolution },
	})
	const instance = await get(instanceId)
	if (!instance) throw new Error(`Instance ${instanceId} is gone`)
	return instance
}

export function isSyncedOptionAvailable(option: SyncedOption): boolean {
	return option !== 'data_packs'
}

export async function get_synced_option_join_preview(
	instanceId: string,
	option: SyncedOption,
): Promise<SyncedOptionJoinPreview> {
	return await apiClient.local.call(syncedOptions.previewSyncedOptionJoin, {
		params: { id: instanceId },
		query: { option },
	})
}

export async function get_synced_options_overview(
	instanceId: string,
): Promise<SyncedOptionsOverview> {
	return await apiClient.local.call(syncedOptions.getInstanceSyncedOptions, {
		params: { id: instanceId },
	})
}

export async function get_global_synced_options(): Promise<GlobalSyncedOptions> {
	return await apiClient.local.call(syncedOptions.getGlobalSyncedOptions)
}

export async function get_initialized_synced_options(): Promise<GlobalSyncedOptions> {
	return await apiClient.local.call(syncedOptions.getInitializedSyncedOptions)
}

export async function set_global_synced_option(
	option: SyncedOption,
	enabled: boolean,
	baseInstanceId?: string,
): Promise<GlobalSyncedOptions> {
	return await apiClient.local.call(syncedOptions.setGlobalSyncedOption, {
		body: { option, enabled, base_instance_id: baseInstanceId },
	})
}

export async function get_command_history(): Promise<string> {
	return await apiClient.local.call(syncedOptions.getCommandHistory)
}

export async function set_command_history(contents: string): Promise<string> {
	return await apiClient.local.call(syncedOptions.setCommandHistory, { body: { contents } })
}

export async function open_synced_options_folder(): Promise<void> {
	return await new ContractClient(localTransport).call(desktop.openFolder, {
		body: { folder: 'synced_options' },
	})
}

export async function list_synced_servers(): Promise<SyncedServer[]> {
	return await apiClient.local.call(syncedOptions.listSyncedServers)
}

export async function update_synced_server(server: SyncedServer): Promise<void> {
	return await apiClient.local.call(syncedOptions.updateSyncedServer, { body: server })
}

export async function remove_synced_server(serverId: string): Promise<void> {
	return await apiClient.local.call(syncedOptions.removeSyncedServer, { params: { id: serverId } })
}

export async function rebuild_synced_options(instanceId?: string): Promise<void> {
	return await apiClient.local.call(syncedOptions.rebuildSyncedOptions, {
		body: { instance_id: instanceId },
	})
}

export interface JavaVersion {
	parsed_version: number
	version: string
	architecture: string
	path: string
}

export async function get_optimal_jre_key(instanceId: string): Promise<JavaVersion | null> {
	return await apiClient.local.call(instances.GetOptimalJava, { params: { id: instanceId } })
}

export async function list(): Promise<GameInstance[]> {
	return await apiClient.local.call(instances.ListInstances)
}

export async function check_installed(instanceId: string, projectId: string): Promise<boolean> {
	return await apiClient.local.call(instanceContent.CheckProjectInstalled, {
		params: { id: instanceId },
		query: { project_id: projectId },
	})
}

// Updates a specified project
export async function update_project(instanceId: string, projectPath: string): Promise<string> {
	return await apiClient.local.call(instanceContent.UpdateProject, {
		params: { id: instanceId },
		body: { project_path: projectPath },
	})
}

// Add a project to an instance from a version
// Returns a path to the new project file
export type DownloadReason = 'standalone' | 'dependency' | 'modpack' | 'update'

export interface ResolutionPreferences {
	game_versions?: string[]
	loaders?: string[]
}

export interface ResolveContentRequest {
	project_id: string
	version_id?: string | null
	content_type: Labrinth.Content.v3.ContentType
	selected?: ResolutionPreferences
}

export interface ResolvedContent {
	project_id: string
	version_id: string
	dependent_on_version_id?: string | null
}

export interface ResolveContentPlan {
	primary: ResolvedContent
	dependencies: ResolvedContent[]
	skipped: Array<{
		project_id: string
		version_id?: string | null
		dependent_on_version_id?: string | null
		reason: string
	}>
}

export async function add_project_from_version(
	instanceId: string,
	versionId: string,
	reason: DownloadReason,
	dependentOnVersionId?: string,
): Promise<string> {
	return await apiClient.local.call(instanceContent.AddProjectFromVersion, {
		params: { id: instanceId },
		body: { version_id: versionId, reason, dependent_on_version_id: dependentOnVersionId },
	})
}

export async function install_project_with_dependencies(
	instanceId: string,
	request: ResolveContentRequest,
): Promise<ResolveContentPlan> {
	return await apiClient.local.call(instanceContent.InstallProjectWithDependencies, {
		params: { id: instanceId },
		body: request,
	})
}

export async function switch_project_version_with_dependencies(
	instanceId: string,
	projectPath: string,
	versionId: string,
): Promise<string> {
	return await apiClient.local.call(instanceContent.SwitchProjectVersion, {
		params: { id: instanceId },
		body: { project_path: projectPath, version_id: versionId },
	})
}

// Add a project to an instance from a path + project_type
// Returns a path to the new project file
export async function add_project_from_path(
	instanceId: string,
	projectPath: string,
	projectType?: ContentFileProjectType,
): Promise<string> {
	return await apiClient.local.call(instancePaths.addProjectFromPath, {
		params: { id: instanceId },
		body: { path: projectPath, project_type: projectType },
	})
}

export async function is_file_on_modrinth(projectPath: string): Promise<boolean> {
	return await apiClient.local.call(instancePaths.checkFileOnModrinth, {
		body: { path: projectPath },
	})
}

// Toggle disabling a project
export async function toggle_disable_project(
	instanceId: string,
	projectPath: string,
	desiredEnabled?: boolean,
): Promise<string> {
	return await apiClient.local.call(instanceContent.ToggleProject, {
		params: { id: instanceId },
		body: { project_path: projectPath, desired_enabled: desiredEnabled },
	})
}

export async function set_project_locked(
	instanceId: string,
	projectPath: string,
	locked: boolean,
): Promise<void> {
	return await apiClient.local.call(instanceContent.SetProjectLocked, {
		params: { id: instanceId },
		body: { project_path: projectPath, locked },
	})
}

// Remove a project
export async function remove_project(instanceId: string, projectPath: string): Promise<void> {
	return await apiClient.local.call(instanceContent.RemoveProject, {
		params: { id: instanceId },
		body: { project_path: projectPath },
	})
}

// Update a managed Modrinth instance to a specific version
export async function update_managed_modrinth_version(
	instanceId: string,
	versionId: string,
): Promise<InstallJobSnapshot> {
	return await apiClient.local.call(instanceContent.UpdateModpackVersion, {
		params: { id: instanceId },
		body: { version_id: versionId },
	})
}

// Repair a managed Modrinth instance
export async function update_repair_modrinth(instanceId: string): Promise<InstallJobSnapshot> {
	return await apiClient.local.call(instanceContent.RepairModpack, { params: { id: instanceId } })
}

// Export an instance to .mrpack
// included_overrides and excluded_overrides are inherited path rules for files in the export.
// Version id is optional (ie: 1.1.5)
export async function export_instance_mrpack(
	instanceId: string,
	exportLocation: string,
	includedOverrides: string[],
	excludedOverrides: string[],
	versionId?: string,
	description?: string,
	name?: string,
): Promise<void> {
	return await apiClient.local.call(instancePaths.exportMrpack, {
		params: { id: instanceId },
		body: {
			location: exportLocation,
			included_overrides: includedOverrides,
			excluded_overrides: excludedOverrides,
			version_id: versionId,
			description,
			name,
		},
	})
}

export type PackExportCandidate = {
	path: string
	type: 'directory' | 'file'
	size?: number
	modified?: number
	count?: number
	disabled: boolean
	defaultSelected: boolean
}

// Given a folder path, populate an array of exportable direct children.
// Allows selection for 'included_overrides' in export_instance_mrpack.
export async function get_pack_export_candidates(
	instanceId: string,
	parent?: string,
): Promise<PackExportCandidate[]> {
	return await apiClient.local.call(instanceContent.ListPackExportCandidates, {
		params: { id: instanceId },
		query: { parent },
	})
}

// Run Minecraft using an instance
// Returns PID of child
export async function run(
	instanceId: string,
	serverAddress: string | null = null,
): Promise<unknown> {
	return await apiClient.local.call(launch.playInstance, {
		body: { instance_id: instanceId, server_address: serverAddress },
	})
}

export async function kill(instanceId: string): Promise<void> {
	return await apiClient.local.call(instances.KillInstance, { params: { id: instanceId } })
}

// Edits an instance
export async function edit(
	instanceId: string,
	editInstance: { [K in keyof GameInstance]?: GameInstance[K] | null },
): Promise<void> {
	return await apiClient.local.call(instances.EditInstance, {
		params: { id: instanceId },
		body: editInstance,
	})
}

// Edits an instance's icon
export async function edit_icon(instanceId: string, iconPath: string | null): Promise<void> {
	return await apiClient.local.call(instancePaths.setInstanceIconFromPath, {
		params: { id: instanceId },
		body: { path: iconPath },
	})
}

export async function edit_generated_icon(
	instanceId: string,
	config: IconConfig,
	symbolBytes: number[],
): Promise<string> {
	const path = await apiClient.local.call(library.setGeneratedInstanceIcon, {
		params: { id: instanceId },
		body: { config, symbol_bytes: symbolBytes, only_if_empty: false },
	})
	if (path === null) throw new Error('The instance icon was not set.')
	return path
}

export async function edit_generated_icon_if_empty(
	instanceId: string,
	config: IconConfig,
	symbolBytes: number[],
): Promise<boolean> {
	const result = await apiClient.local.call(library.setGeneratedInstanceIcon, {
		params: { id: instanceId },
		body: { config, symbol_bytes: symbolBytes, only_if_empty: true },
	})
	return result !== null
}

export async function cache_generated_icon(
	config: IconConfig,
	symbolBytes: number[],
	addToRecents = false,
): Promise<string> {
	return await apiClient.local.call(library.cacheGeneratedIcon, {
		body: { config, symbol_bytes: symbolBytes, add_to_recents: addToRecents },
	})
}

/** The bytes of an icon editor symbol, which the App draws its icons from. */
export async function loadSymbolBytes(asset: string): Promise<number[]> {
	const response = await fetch(asset)
	if (!response.ok) throw new Error('Failed to load the icon symbol.')
	return Array.from(new Uint8Array(await response.arrayBuffer()))
}

/**
 * A share's icon as an instance icon path or URL. A share made in the icon editor arrives as its
 * settings only, so it is drawn here, cached by content. Null when there is none to show.
 */
export async function shared_icon_path(
	icon: string | null | undefined,
	config: IconConfig | null | undefined,
): Promise<string | null> {
	if (icon) return icon
	const asset = config ? getIconSymbolAsset(config.symbol) : undefined
	if (!config || !asset) return null
	return await cache_generated_icon(config, await loadSymbolBytes(asset))
}

export async function get_recent_icon_configs(): Promise<IconConfig[]> {
	return await apiClient.local.call(library.listRecentIconConfigs)
}

export type {
	SharedInstanceInvite,
	SharedInstanceInviteLink,
	SharedInstanceJoinType,
	SharedInstancePublishFile,
	SharedInstancePublishPreview,
	SharedInstanceUser,
	SharedInstanceUsers,
} from '@modrinth/api-client'

export async function can_current_user_use_shared_instances(): Promise<boolean> {
	return await apiClient.local.call(localSharing.canUseSharedInstances)
}

export async function get_shared_instance_users(instanceId: string): Promise<SharedInstanceUsers> {
	return await apiClient.local.call(instanceSharing.GetSharedInstanceUsers, {
		params: { id: instanceId },
	})
}

export async function invite_shared_instance_users(
	instanceId: string,
	userIds: string[],
): Promise<SharedInstanceUsers> {
	return await apiClient.local.call(instanceSharing.InviteSharedInstanceUsers, {
		params: { id: instanceId },
		body: { user_ids: userIds },
	})
}

export async function create_shared_instance_invite_link(
	instanceId: string,
	options: {
		maxAgeSeconds?: number
		maxUses?: number
		replaceInviteId?: string
	} = {},
): Promise<SharedInstanceInviteLink> {
	return await apiClient.local.call(instanceSharing.CreateSharedInstanceInviteLink, {
		params: { id: instanceId },
		body: {
			max_age_seconds: options.maxAgeSeconds ?? null,
			max_uses: options.maxUses ?? null,
			replace_invite_id: options.replaceInviteId ?? null,
		},
	})
}

export async function get_shared_instance_invites(
	instanceId: string,
): Promise<SharedInstanceInvite[]> {
	return await apiClient.local.call(instanceSharing.ListSharedInstanceInvites, {
		params: { id: instanceId },
	})
}

export async function revoke_shared_instance_invite(
	instanceId: string,
	inviteId: string,
): Promise<void> {
	return await apiClient.local.call(instanceSharing.RevokeSharedInstanceInvite, {
		params: { id: instanceId, invite_id: inviteId },
	})
}

export async function remove_shared_instance_users(
	instanceId: string,
	userIds: string[],
	hasPendingRecipients: boolean,
): Promise<SharedInstanceUsers> {
	return await apiClient.local.call(instanceSharing.RemoveSharedInstanceUsers, {
		params: { id: instanceId },
		body: { user_ids: userIds, has_pending_recipients: hasPendingRecipients },
	})
}

export async function get_shared_instance_publish_preview(
	instanceId: string,
): Promise<SharedInstancePublishPreview | null> {
	return await apiClient.local.call(instanceSharing.GetSharedInstancePublishPreview, {
		params: { id: instanceId },
	})
}

/**
 * `serverCoreId` is a Core the account owns that runs a linked server of this instance. The
 * backend then keeps the share's home copy on it.
 */
export async function publish_shared_instance(
	instanceId: string,
	filePaths: string[],
	removedPaths: string[],
	serverCoreId: string | null,
	onProgress: (uploadedBytes: number, totalBytes: number) => void,
): Promise<SharedInstanceAttachment> {
	const op = crypto.randomUUID()
	const { error } = await runOp(
		op,
		() =>
			apiClient.local.call(localSharing.publishSharedInstance, {
				body: {
					op,
					instance_id: instanceId,
					file_paths: filePaths,
					removed_paths: removedPaths,
					server_core_id: serverCoreId,
				},
			}),
		({ done, total }) => onProgress(done, total ?? done),
	)
	if (error) throw new ApiCallError(500, error.error, error.description)
	const attachment = (await get(instanceId))?.shared_instance
	if (!attachment) throw new Error(`Instance ${instanceId} has no share`)
	return attachment
}

export async function get_shared_instance_publish_support_details(
	instanceId: string,
): Promise<string | null> {
	return await apiClient.local.call(instanceSharing.GetSharedInstancePublishSupportDetails, {
		params: { id: instanceId },
	})
}

export async function unlink_shared_instance(instanceId: string): Promise<void> {
	return await apiClient.local.call(instanceSharing.UnlinkSharedInstance, {
		params: { id: instanceId },
	})
}

export async function unpublish_shared_instance(instanceId: string): Promise<void> {
	return await apiClient.local.call(instanceSharing.UnpublishSharedInstance, {
		params: { id: instanceId },
	})
}

/** The instance's share, created with its first version when it has none, so it can run servers. */
export async function ensure_shared_instance(
	instanceId: string,
): Promise<SharedInstanceAttachment> {
	return await apiClient.local.call(instanceSharing.EnsureSharedInstance, {
		params: { id: instanceId },
	})
}
