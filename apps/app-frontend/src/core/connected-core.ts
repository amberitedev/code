import { readonly, ref } from 'vue'

export interface ConnectedCore {
	coreId: string
	url: string
	linkedAt: number
}

const STORAGE_KEY = 'amberite:connected-core'
const connectedCore = ref<ConnectedCore | null>(readConnectedCore())

export function useConnectedCore() {
	return readonly(connectedCore)
}

export function getConnectedCore() {
	return connectedCore.value
}

export function setConnectedCore(core: Omit<ConnectedCore, 'linkedAt'> & { linkedAt?: number }) {
	const next = {
		coreId: core.coreId.trim(),
		url: normalizeCoreUrl(core.url),
		linkedAt: core.linkedAt ?? Date.now(),
	}
	if (!next.coreId) throw new Error('A connected Core must have an identity.')
	connectedCore.value = next
	window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
	return next
}

export function clearConnectedCore() {
	connectedCore.value = null
	window.localStorage.removeItem(STORAGE_KEY)
}

function readConnectedCore(): ConnectedCore | null {
	if (typeof window === 'undefined') return null
	try {
		const value: unknown = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? 'null')
		if (!value || typeof value !== 'object') return null
		const record = value as Record<string, unknown>
		if (typeof record.coreId !== 'string' || typeof record.url !== 'string') return null
		return {
			coreId: record.coreId,
			url: normalizeCoreUrl(record.url),
			linkedAt: typeof record.linkedAt === 'number' ? record.linkedAt : Date.now(),
		}
	} catch {
		return null
	}
}

function normalizeCoreUrl(value: string) {
	const url = new URL(value)
	if (url.protocol !== 'http:' && url.protocol !== 'https:')
		throw new Error('Core URLs must use HTTP or HTTPS.')
	if (url.username || url.password || url.search || url.hash || url.pathname !== '/')
		throw new Error('Core URL must be a plain HTTP origin.')
	return url.origin
}
