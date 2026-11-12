import { invoke } from '@tauri-apps/api/core'

/** Writes the icon bytes to the app's icon cache and returns the cached file path. */
export function cacheIconBytes(iconBytes: number[]): Promise<string> {
	return invoke('plugin:instance|instance_cache_icon', { iconBytes })
}
