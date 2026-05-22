import { emit, listen } from '@tauri-apps/api/event'
import { reactive } from 'vue'

const STORAGE_KEY = 'theme-editor-overrides'
const EVENT_CHANGE = 'tw:var-change'
const EVENT_RESET = 'tw:var-reset-all'

export const overrides = reactive<Record<string, string>>({})

function applyToDocument(name: string, value: string) {
	document.documentElement.style.setProperty(name, value)
}

function removeFromDocument(name: string) {
	document.documentElement.style.removeProperty(name)
}

function persist() {
	localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...overrides }))
}

function loadSaved() {
	try {
		const raw = localStorage.getItem(STORAGE_KEY)
		if (!raw) return
		const saved = JSON.parse(raw) as Record<string, string>
		for (const [name, value] of Object.entries(saved)) {
			overrides[name] = value
			applyToDocument(name, value)
		}
	} catch {
		// ignore malformed data
	}
}

loadSaved()

export function setVar(name: string, value: string) {
	overrides[name] = value
	applyToDocument(name, value)
	persist()
	emit(EVENT_CHANGE, { name, value }).catch(console.warn)
}

export function resetVar(name: string) {
	Reflect.deleteProperty(overrides, name)
	removeFromDocument(name)
	persist()
	emit(EVENT_CHANGE, { name, value: null }).catch(console.warn)
}

export function resetAll() {
	for (const name of Object.keys(overrides)) {
		Reflect.deleteProperty(overrides, name)
		removeFromDocument(name)
	}
	localStorage.removeItem(STORAGE_KEY)
	emit(EVENT_RESET, null).catch(console.warn)
}

export async function listenForThemeChanges(): Promise<() => void> {
	const applied = new Set<string>(Object.keys(overrides))

	const stopChange = await listen<{ name: string; value: string | null }>(EVENT_CHANGE, (e) => {
		const { name, value } = e.payload
		if (value === null) {
			removeFromDocument(name)
			applied.delete(name)
		} else {
			applyToDocument(name, value)
			applied.add(name)
		}
	})

	const stopReset = await listen(EVENT_RESET, () => {
		for (const name of applied) {
			removeFromDocument(name)
		}
		applied.clear()
	})

	return () => {
		stopChange()
		stopReset()
	}
}

export function useThemeEditorComms() {
	return { overrides, setVar, resetVar, resetAll, listenForThemeChanges }
}
