import type { AbstractModrinthClient } from '../core/abstract-client'
import type { AbstractModule } from '../core/abstract-module'
import { SharedInstancesPlayersV1Module } from '../sharing/players/v1'
import { ISO3166Module } from './iso3166'
import { LabrinthVersionsV2Module, LabrinthVersionsV3Module } from './labrinth'
import { LabrinthAttributionInternalModule } from './labrinth/attribution/internal'
import { LabrinthAuthInternalModule } from './labrinth/auth/internal'
import { LabrinthAuthV2Module } from './labrinth/auth/v2'
import { LabrinthBlockedUsersInternalModule } from './labrinth/blocked-users/internal'
import { LabrinthBlockedUsersV3Module } from './labrinth/blocked-users/v3'
import { LabrinthCampaignInternalModule } from './labrinth/campaign/internal'
import { LabrinthCollectionsModule } from './labrinth/collections'
import { LabrinthContentV3Module } from './labrinth/content/v3'
import { LabrinthExternalProjectsInternalModule } from './labrinth/external-projects/internal'
import { LabrinthFriendsV3Module } from './labrinth/friends/v3'
import { LabrinthGeoIpModule } from './labrinth/geoip'
import { LabrinthGlobalsInternalModule } from './labrinth/globals/internal'
import { LabrinthImagesV3Module } from './labrinth/images/v3'
import { LabrinthLimitsV3Module } from './labrinth/limits/v3'
import { LabrinthModerationInternalModule } from './labrinth/moderation/internal'
import { LabrinthNotificationsV2Module } from './labrinth/notifications/v2'
import { LabrinthOAuthInternalModule } from './labrinth/oauth/internal'
import { LabrinthOrganizationsV3Module } from './labrinth/organizations/v3'
import { LabrinthPatsV2Module } from './labrinth/pats/v2'
import { LabrinthProjectsV2Module } from './labrinth/projects/v2'
import { LabrinthProjectsV3Module } from './labrinth/projects/v3'
import { LabrinthReportsV3Module } from './labrinth/reports/v3'
import { LabrinthServerPingInternalModule } from './labrinth/server-ping/internal'
import { LabrinthSessionsV2Module } from './labrinth/sessions/v2'
import { LabrinthStateModule } from './labrinth/state'
import { LabrinthTagsV2Module } from './labrinth/tags/v2'
import { LabrinthTeamsV2Module } from './labrinth/teams/v2'
import { LabrinthTeamsV3Module } from './labrinth/teams/v3'
import { LabrinthThreadsV3Module } from './labrinth/threads/v3'
import { LabrinthUsersV2Module } from './labrinth/users/v2'
import { LabrinthUsersV3Module } from './labrinth/users/v3'
import { LauncherMetaManifestV0Module } from './launcher-meta/v0'
import { MclogsInsightsV1Module } from './mclogs/insights/v1'
import { MclogsLogsV1Module } from './mclogs/logs/v1'
import { PaperVersionsV3Module } from './paper/v3'
import { PurpurVersionsV2Module } from './purpur/v2'
import { SharedInstancesInstancesV1Module } from './shared-instances/instances/v1'
import { SharedInstancesInvitesV1Module } from './shared-instances/invites/v1'
import { SharedInstancesModerationV1Module } from './shared-instances/moderation/v1'
import { SharedInstancesUsersV1Module } from './shared-instances/users/v1'

type ModuleConstructor = new (client: AbstractModrinthClient) => AbstractModule

/**
 * To add a new module:
 * 1. Create your module class extending AbstractModule
 * 2. Add one line here: `<api>_<module>: YourModuleClass`
 *
 * TypeScript will automatically infer the client's field structure from this registry.
 *
 * TODO: Better way? Probably not
 */
const MODULE_REGISTRY = {
	iso3166_data: ISO3166Module,
	mclogs_insights_v1: MclogsInsightsV1Module,
	mclogs_logs_v1: MclogsLogsV1Module,
	launchermeta_manifest_v0: LauncherMetaManifestV0Module,
	labrinth_auth_internal: LabrinthAuthInternalModule,
	labrinth_auth_v2: LabrinthAuthV2Module,
	labrinth_attribution_internal: LabrinthAttributionInternalModule,
	labrinth_blocked_users_internal: LabrinthBlockedUsersInternalModule,
	labrinth_blocked_users_v3: LabrinthBlockedUsersV3Module,
	labrinth_campaign_internal: LabrinthCampaignInternalModule,
	labrinth_collections: LabrinthCollectionsModule,
	labrinth_content_v3: LabrinthContentV3Module,
	labrinth_external_projects_internal: LabrinthExternalProjectsInternalModule,
	labrinth_friends_v3: LabrinthFriendsV3Module,
	labrinth_geoip: LabrinthGeoIpModule,
	labrinth_globals_internal: LabrinthGlobalsInternalModule,
	labrinth_images_v3: LabrinthImagesV3Module,
	labrinth_moderation_internal: LabrinthModerationInternalModule,
	labrinth_notifications_v2: LabrinthNotificationsV2Module,
	labrinth_oauth_internal: LabrinthOAuthInternalModule,
	labrinth_organizations_v3: LabrinthOrganizationsV3Module,
	labrinth_pats_v2: LabrinthPatsV2Module,
	labrinth_limits_v3: LabrinthLimitsV3Module,
	labrinth_projects_v2: LabrinthProjectsV2Module,
	labrinth_projects_v3: LabrinthProjectsV3Module,
	labrinth_reports_v3: LabrinthReportsV3Module,
	labrinth_server_ping_internal: LabrinthServerPingInternalModule,
	labrinth_sessions_v2: LabrinthSessionsV2Module,
	labrinth_state: LabrinthStateModule,
	labrinth_tags_v2: LabrinthTagsV2Module,
	labrinth_teams_v2: LabrinthTeamsV2Module,
	labrinth_teams_v3: LabrinthTeamsV3Module,
	labrinth_threads_v3: LabrinthThreadsV3Module,
	labrinth_users_v2: LabrinthUsersV2Module,
	labrinth_users_v3: LabrinthUsersV3Module,
	labrinth_versions_v2: LabrinthVersionsV2Module,
	labrinth_versions_v3: LabrinthVersionsV3Module,
	paper_versions_v3: PaperVersionsV3Module,
	purpur_versions_v2: PurpurVersionsV2Module,
	sharedinstances_invites_v1: SharedInstancesInvitesV1Module,
	sharedinstances_instances_v1: SharedInstancesInstancesV1Module,
	sharedinstances_moderation_v1: SharedInstancesModerationV1Module,
	sharedinstances_players_v1: SharedInstancesPlayersV1Module,
	sharedinstances_users_v1: SharedInstancesUsersV1Module,
} as const satisfies Record<string, ModuleConstructor>

/**
 * Parse a module ID into [api, moduleName] tuple
 *
 * @param id - Module ID in format `<api>_<module>` (e.g., 'labrinth_projects_v2')
 * @returns Tuple of [api, moduleName] (e.g., ['labrinth', 'projects_v2'])
 * @throws Error if module ID doesn't match expected format
 */
function parseModuleID(id: string): [string, string] {
	const parts = id.split('_')
	if (parts.length < 2) {
		throw new Error(
			`Invalid module ID "${id}". Expected format: <api>_<module> (e.g., "labrinth_projects_v2")`,
		)
	}
	const api = parts[0]
	const moduleName = parts.slice(1).join('_')
	return [api, moduleName]
}

/**
 * Build nested module structure from flat registry
 *
 * Transforms:
 * ```
 * { labrinth_projects_v2: Constructor, labrinth_users_v2: Constructor }
 * ```
 * Into:
 * ```
 * { labrinth: { projects_v2: Constructor, users_v2: Constructor } }
 * ```
 *
 * @returns Nested structure organized by API namespace
 */
export function buildModuleStructure(): Record<string, Record<string, ModuleConstructor>> {
	const structure: Record<string, Record<string, ModuleConstructor>> = {}

	for (const [id, constructor] of Object.entries(MODULE_REGISTRY)) {
		const [api, moduleName] = parseModuleID(id)

		if (!structure[api]) {
			structure[api] = {}
		}

		structure[api][moduleName] = constructor
	}

	return structure
}

/**
 * Extract API name from module ID
 * @example ParseAPI<'labrinth_projects_v2'> = 'labrinth'
 */
type ParseAPI<T extends string> = T extends `${infer API}_${string}` ? API : never

/**
 * Extract module name for a given API
 * @example ParseModule<'labrinth_projects_v2', 'labrinth'> = 'projects_v2'
 */
type ParseModule<T extends string, API extends string> = T extends `${API}_${infer Module}`
	? Module
	: never

/**
 * Group registry modules by API namespace
 *
 * Transforms flat registry into nested structure at the type level:
 * ```
 * { labrinth_projects_v2: ModuleClass } → { labrinth: { projects_v2: ModuleInstance } }
 * ```
 */
type GroupByAPI<Registry extends Record<string, ModuleConstructor>> = {
	[API in ParseAPI<keyof Registry & string>]: {
		[Module in ParseModule<keyof Registry & string, API>]: InstanceType<
			Registry[`${API}_${Module}`]
		>
	}
}

/**
 * Inferred client module structure
 **/
export type InferredClientModules = GroupByAPI<typeof MODULE_REGISTRY>
