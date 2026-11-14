/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { tags } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

// Callers read some answers with Modrinth's own types, which the engine's cache types do not
// match, so those stay untyped, as they were over invoke.

// Gets cached category tags
export async function get_categories() {
	return await apiClient.local.call(tags.GetCategoryTags)
}

// Gets cached loaders tags
/** @returns {Promise<any>} */
export async function get_loaders() {
	return await apiClient.local.call(tags.GetLoaderTags)
}

// Gets cached game_versions tags
/** @returns {Promise<any>} */
export async function get_game_versions() {
	return await apiClient.local.call(tags.GetGameVersionTags)
}
