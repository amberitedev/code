/**
 * Synced profile -> Core instance linkage.
 *
 * A synced profile surfaces a client side (the app-lib profile, keyed by its
 * path) and a server side (a Copal instance). The app-lib profile path stays
 * readable, while this local map points it at the Core UUID used by durable sync
 * records. A second local display map caches the Core public path so synced
 * server tabs can use readable path-based URLs before the next SSE snapshot.
 */

const ID_STORAGE_KEY = 'amberite:synced-links'
const PATH_STORAGE_KEY = 'amberite:synced-paths'

type SyncedLinkMap = Record<string, string>

function readMap(storageKey: string): SyncedLinkMap {
	try {
		const raw = localStorage.getItem(storageKey)
		if (!raw) return {}
		const parsed = JSON.parse(raw) as unknown
		return parsed && typeof parsed === 'object' ? (parsed as SyncedLinkMap) : {}
	} catch {
		return {}
	}
}

function writeMap(storageKey: string, map: SyncedLinkMap): void {
	try {
		localStorage.setItem(storageKey, JSON.stringify(map))
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
	const map = readMap(ID_STORAGE_KEY)
	map[profilePath] = coreInstanceId
	writeMap(ID_STORAGE_KEY, map)
}

/**
 * Resolve the Core instance id for a synced profile, falling back to the profile
 * path for older local mappings.
 */
export function getLinkedServerId(profilePath: string): string {
	return readMap(ID_STORAGE_KEY)[profilePath] ?? profilePath
}

/** Cache a synced profile's Core public path for display and path-based calls. */
export function setLinkedServerPath(profilePath: string, coreInstancePath: string): void {
	if (profilePath === coreInstancePath) {
		clearLinkedServerPath(profilePath)
		return
	}
	const map = readMap(PATH_STORAGE_KEY)
	map[profilePath] = coreInstancePath
	writeMap(PATH_STORAGE_KEY, map)
}

/** Resolve the cached public Core path for a synced profile, when known. */
export function getLinkedServerPath(profilePath: string): string | null {
	return readMap(PATH_STORAGE_KEY)[profilePath] ?? null
}

/** Drop a synced profile's server link (e.g. when reverting to a client profile). */
export function clearLinkedServerId(profilePath: string): void {
	const map = readMap(ID_STORAGE_KEY)
	if (profilePath in map) {
		const { [profilePath]: _removed, ...rest } = map
		writeMap(ID_STORAGE_KEY, rest)
	}
	clearLinkedServerPath(profilePath)
}

export function clearLinkedServerPath(profilePath: string): void {
	const map = readMap(PATH_STORAGE_KEY)
	if (profilePath in map) {
		const { [profilePath]: _removed, ...rest } = map
		writeMap(PATH_STORAGE_KEY, rest)
	}
}
