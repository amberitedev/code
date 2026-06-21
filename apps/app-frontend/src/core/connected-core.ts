import { readonly, ref } from 'vue'

export interface ConnectedCore {
	coreId: string
	url: string
	linkedAt: number
	groupId?: string
}

export const CONNECTED_CORE_STORAGE_KEY = 'amberite:connected-core'

const connectedCore = ref<ConnectedCore | null>(readConnectedCore())

export function useConnectedCore() {
	return readonly(connectedCore)
}

export function getConnectedCore(): ConnectedCore | null {
	return connectedCore.value
}

export function setConnectedCore(core: Omit<ConnectedCore, 'linkedAt'> & { linkedAt?: number }): ConnectedCore {
	if (!core.coreId.trim()) throw new Error('A connected Core must have an identity.')
	const next: ConnectedCore = {
		coreId: core.coreId,
		url: normalizeCoreUrl(core.url),
		linkedAt: core.linkedAt ?? Date.now(),
		...(core.groupId ? { groupId: core.groupId } : {}),
	}
	connectedCore.value = next
	persist(next)
	return next
}

export function clearConnectedCore(): void {
	connectedCore.value = null
	if (typeof window === 'undefined') return
	try {
		window.localStorage.removeItem(CONNECTED_CORE_STORAGE_KEY)
	} catch {
		// Ignore unavailable storage.
	}
}

function readConnectedCore(): ConnectedCore | null {
	if (typeof window === 'undefined') return null
	try {
		const raw = window.localStorage.getItem(CONNECTED_CORE_STORAGE_KEY)
		if (!raw) return null
		const parsed = JSON.parse(raw) as Partial<ConnectedCore>
		if (typeof parsed.coreId !== 'string' || typeof parsed.url !== 'string') return null
		return {
			coreId: parsed.coreId,
			url: normalizeCoreUrl(parsed.url),
			linkedAt: typeof parsed.linkedAt === 'number' ? parsed.linkedAt : Date.now(),
			...(typeof parsed.groupId === 'string' ? { groupId: parsed.groupId } : {}),
		}
	} catch {
		return null
	}
}

function persist(core: ConnectedCore): void {
	if (typeof window === 'undefined') return
	try {
		window.localStorage.setItem(CONNECTED_CORE_STORAGE_KEY, JSON.stringify(core))
	} catch {
		// Keep the in-memory link when storage is unavailable.
	}
}

function normalizeCoreUrl(value: string): string {
	const url = new URL(value)
	if (url.protocol !== 'http:' && url.protocol !== 'https:') {
		throw new Error('Core URLs must use HTTP or HTTPS.')
	}
	if (url.username || url.password || url.search || url.hash || url.pathname !== '/') {
		throw new Error('Core URL must be a plain HTTP origin.')
	}
	return url.origin
}
