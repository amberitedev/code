import { invoke } from '@tauri-apps/api/core'

export { getCurrentWindow } from '@tauri-apps/api/window'
export { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
export { saveWindowState, StateFlags } from '@tauri-apps/plugin-window-state'

export function showWindow(): Promise<void> {
	return invoke('show_window')
}
