import { SERVER_SHARE_FOLDER, SHAREABLE_CONFIG_EXTENSIONS } from '@modrinth/api-client'

// Mirrors `ShareScope::Server` in packages/sharing-client/src/rules/mod.rs, which decides what a
// server's share carries; the extension list is generated from it.
const extensions = new Set(SHAREABLE_CONFIG_EXTENSIONS)

/** Whether a file or folder name inside `config/` can be included in a shared instance push. */
export function isShareableConfigEntryName(name: string) {
	return !name.startsWith('.') && !/[/\\:]/.test(name) && !/[. ]$/.test(name)
}

export function hasShareableConfigExtension(name: string) {
	return extensions.has(name.split('.').pop()?.toLowerCase() ?? '')
}

/**
 * Normalizes a server file path to the `config/...` form accepted by the share endpoint,
 * or returns `undefined` when the file can't be shared as a config file.
 */
export function toShareableConfigPath(path: string): string | undefined {
	const segments = path.split('/').filter(Boolean)
	if (segments.length < 2 || segments[0] !== SERVER_SHARE_FOLDER) return
	const entries = segments.slice(1)
	if (!entries.every(isShareableConfigEntryName)) return
	if (!hasShareableConfigExtension(entries[entries.length - 1])) return
	return segments.join('/')
}
