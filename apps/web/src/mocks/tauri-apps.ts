// ============================================================
// Tauri API mock — all functions are no-ops or return stub data
// Used to replace @tauri-apps/* imports in the web-only build
// ============================================================

// ---------- @tauri-apps/api/core ----------
export async function invoke(_command: string, _payload?: Record<string, unknown>): Promise<unknown> {
	return null
}

export function convertFileSrc(path: string): string {
	// TODO: connect to backend API - need endpoint to serve local file assets
	// For now, return empty to prevent broken image display
	if (!path) return ''
	return ''
}

// ---------- @tauri-apps/api/app ----------
export async function getVersion(): Promise<string> {
	return '1.0.0-web'
}

export async function getName(): Promise<string> {
	return 'modrinth-app-ui'
}

export async function getTauriVersion(): Promise<string> {
	return 'N/A'
}

// ---------- @tauri-apps/api/window ----------
const mockWindow = {
	label: 'main',
	async listen(_event: string, _handler: (...args: unknown[]) => void) {
		return () => {}
	},
	async setTitle(_title: string) {},
	async show() {},
	async hide() {},
	async close() {},
	async minimize() {},
	async maximize() {},
	async unmaximize() {},
	async isMaximized() {
		return false
	},
	async isMinimized() {
		return false
	},
	async isFullscreen() {
		return false
	},
	async setFocus() {},
	async setAlwaysOnTop(_alwaysOnTop: boolean) {},
	async setSize(_size: { width: number; height: number }) {},
	async setMinSize(_size: { width: number; height: number }) {},
	async setMaxSize(_size: { width: number; height: number }) {},
	async setPosition(_position: { x: number; y: number }) {},
	async setResizable(_resizable: boolean) {},
	async setDecorations(_decorations: boolean) {},
	async setShadow(_shadow: boolean) {},
	async startDragging() {},
	async onResized(_handler: () => void) {
		return () => {}
	},
	async toggleMaximize() {},
}

export function getCurrentWindow() {
	return mockWindow
}

export function getAllWindows() {
	return [mockWindow]
}

// ---------- @tauri-apps/api/event ----------
export async function listen<T>(
	_event: string,
	_handler: (event: { payload: T }) => void,
): Promise<() => void> {
	return () => {}
}

export async function once<T>(
	_event: string,
	_handler: (event: { payload: T }) => void,
): Promise<() => void> {
	return () => {}
}

export function emit(_event: string, _payload?: unknown): Promise<void> {
	return Promise.resolve()
}

// ---------- @tauri-apps/api/webview ----------
const mockWebview = {
	label: 'main',
	async listen(_event: string, _handler: (...args: unknown[]) => void) {
		return () => {}
	},
	async setZoom(_zoom: number) {},
	async print() {},
	async close() {},
	async show() {},
	async hide() {},
	async setSize(_size: unknown) {},
	async setPosition(_position: unknown) {},
	async setFocus() {},
	async onDragDropEvent(_handler: (event: unknown) => void) {
		return () => {}
	},
}

export function getCurrentWebview() {
	return mockWebview
}

export function getAllWebviews() {
	return [mockWebview]
}

// ---------- @tauri-apps/plugin-opener ----------
export async function openUrl(url: string): Promise<void> {
	window.open(url, '_blank')
}

// ---------- @tauri-apps/plugin-dialog ----------
export async function open(_options?: unknown): Promise<string | string[] | null> {
	return null
}

export async function save(_options?: unknown): Promise<string | null> {
	return null
}

export async function message(_options?: unknown): Promise<void> {}

export async function ask(_options?: unknown): Promise<boolean> {
	return false
}

export async function confirm(_options?: unknown): Promise<boolean> {
	return false
}

// ---------- @tauri-apps/plugin-os ----------
export function platform(): string {
	return 'web'
}

export async function version(): Promise<string> {
	return ''
}

export async function arch(): Promise<string> {
	return 'web'
}

export async function locale(): Promise<string> {
	return 'en-US'
}

export function type(): string {
	return 'web'
}

// ---------- @tauri-apps/plugin-fs ----------
export async function readTextFile(_path: string): Promise<string> {
	return ''
}

export async function readFile(_path: string): Promise<Uint8Array> {
	return new Uint8Array()
}

export async function writeTextFile(_path: string, _contents: string): Promise<void> {}

export async function writeFile(_path: string, _contents: Uint8Array): Promise<void> {}

export async function mkdir(_path: string, _options?: { recursive?: boolean }): Promise<void> {}

export async function remove(_path: string, _options?: { recursive?: boolean }): Promise<void> {}

export async function rename(_oldPath: string, _newPath: string): Promise<void> {}

export async function copyFile(_srcPath: string, _destPath: string): Promise<void> {}

export async function exists(_path: string): Promise<boolean> {
	return false
}

export async function readDir(_path: string): Promise<unknown[]> {
	return []
}

export async function stat(_path: string): Promise<unknown> {
	return {}
}

// ---------- @tauri-apps/plugin-window-state ----------
export async function saveWindowState(_flags?: unknown): Promise<void> {}

export async function restoreStateCurrent(_window: unknown): Promise<void> {}

export const StateFlags = {
	ALL: 0,
	MAXIMIZED: 1,
	FULLSCREEN: 2,
}
