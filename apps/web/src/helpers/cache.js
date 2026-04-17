/**
 * Mocked cache helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

export async function get_project(_id, _cacheBehaviour) {
	// TODO: connect to backend API - fetch project from API
	return null
}

export async function get_project_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_project_v3(_id, _cacheBehaviour) {
	return null
}

export async function get_project_v3_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_version(_id, _cacheBehaviour) {
	return null
}

export async function get_version_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_user(_id, _cacheBehaviour) {
	return null
}

export async function get_user_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_team(_id, _cacheBehaviour) {
	return null
}

export async function get_team_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_organization(_id, _cacheBehaviour) {
	return null
}

export async function get_organization_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_search_results(_query, _cacheBehaviour) {
	// TODO: connect to backend API - fetch search results from API
	// Return proper structure to match expected { result: { hits: [...] } }
	return { result: { hits: [] } }
}

export async function get_search_results_many(_ids, _cacheBehaviour) {
	return []
}

export async function get_search_results_v3(_id, _cacheBehaviour) {
	return []
}

export async function get_search_results_v3_many(_ids, _cacheBehaviour) {
	return []
}

export async function purge_cache_types(_cacheTypes) {
	// no-op
}

export async function get_project_versions(_projectId, _cacheBehaviour) {
	return []
}
