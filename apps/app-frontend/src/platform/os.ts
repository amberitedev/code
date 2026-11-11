export { getVersion } from '@tauri-apps/api/app'
// Components read these during setup, so they stay synchronous.
export { platform, type, version } from '@tauri-apps/plugin-os'
