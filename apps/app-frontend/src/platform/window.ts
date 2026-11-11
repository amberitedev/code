import { isTauri } from '@tauri-apps/api/core'

export { getCurrentWindow } from '@tauri-apps/api/window'
export { saveWindowState, StateFlags } from '@tauri-apps/plugin-window-state'

export const isNativeWindow = isTauri()
