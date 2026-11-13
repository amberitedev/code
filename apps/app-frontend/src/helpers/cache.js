import { cache } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

// Callers read some answers with Modrinth's own types, which the engine's cache types do not
// match, so those stay untyped, as they were over invoke.

/** @returns {Promise<any>} */
export async function get_project(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedProject, {
		params: { id },
		query: { cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_project_many(ids, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedProjects, {
		query: { ids: JSON.stringify(ids), cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_project_v3(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedProjectV3, {
		params: { id },
		query: { cacheBehaviour },
	})
}

export async function get_project_v3_many(ids, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedProjectsV3, {
		query: { ids: JSON.stringify(ids), cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_version(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedVersion, {
		params: { id },
		query: { cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_version_many(ids, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedVersions, {
		query: { ids: JSON.stringify(ids), cacheBehaviour },
	})
}

export async function get_user(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedUser, {
		params: { id },
		query: { cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_user_many(ids, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedUsers, {
		query: { ids: JSON.stringify(ids), cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_team(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedTeam, {
		params: { id },
		query: { cacheBehaviour },
	})
}

/** @returns {Promise<any>} */
export async function get_organization(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedOrganization, {
		params: { id },
		query: { cacheBehaviour },
	})
}

export async function get_search_results(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedSearchResults, { query: { id, cacheBehaviour } })
}

export async function get_search_results_v3(id, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedSearchResultsV3, {
		query: { id, cacheBehaviour },
	})
}

export async function purge_cache_types(cacheTypes) {
	return await apiClient.local.call(cache.PurgeCache, { body: cacheTypes })
}

/**
 * Get versions for a project (without changelogs for fast loading).
 * Uses the cache system - versions are cached for 30 minutes.
 * @param {string} projectId - The project ID
 * @param {string} [cacheBehaviour] - Cache behaviour ('must_revalidate', etc.)
 * @returns {Promise<Array|null>} Array of version objects (without changelogs) or null
 */
export async function get_project_versions(projectId, cacheBehaviour) {
	return await apiClient.local.call(cache.GetCachedProjectVersions, {
		params: { id: projectId },
		query: { cacheBehaviour },
	})
}
