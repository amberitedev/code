/**
 * Synced profile -> Core instance linkage.
 *
 * A synced profile surfaces a client side (the app-lib profile, keyed by its
 * path) and a server side (an Copal instance). For profiles created via
 * the "Synced" creation flow the profile is named after the Core instance id, so
 * `profilePath === coreInstanceId` and no mapping is needed.
 *
 * Profiles converted from an existing client profile keep their original path,
 * which can never equal a freshly generated Core UUID. For those we persist an
 * explicit `profilePath -> coreInstanceId` mapping so the server side can resolve
 * the right Core instance. The map is frontend-local (localStorage) so it needs
 * no app-lib or Core backend changes.
 */

const STORAGE_KEY = 'amberite:synced-links'

type SyncedLinkMap = Record<string, string>

function readMap(): SyncedLinkMap {
	try {
		const raw = localStorage.getItem(STORAGE_KEY)
		if (!raw) return {}
		const parsed = JSON.parse(raw) as unknown
		return parsed && typeof parsed === 'object' ? (parsed as SyncedLinkMap) : {}
	} catch {
		return {}
	}
}

function writeMap(map: SyncedLinkMap): void {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
	} catch {
		// Ignore storage failures — the server side just falls back to the path.
	}
}

/** Record that a synced profile's server side lives at `coreInstanceId`. */
export function setLinkedServerId(profilePath: string, coreInstanceId: string): void {
	if (profilePath === coreInstanceId) {
		clearLinkedServerId(profilePath)
		return
	}
	const map = readMap()
	map[profilePath] = coreInstanceId
	writeMap(map)
}

/**
 * Resolve the Core instance id for a synced profile, falling back to the profile
 * path itself (the created-synced identity case).
 */
export function getLinkedServerId(profilePath: string): string {
	return readMap()[profilePath] ?? profilePath
}

/** Drop a synced profile's server link (e.g. when reverting to a client profile). */
export function clearLinkedServerId(profilePath: string): void {
	const map = readMap()
	if (profilePath in map) {
		const { [profilePath]: _removed, ...rest } = map
		writeMap(rest)
	}
}
