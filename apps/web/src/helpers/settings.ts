/**
 * Mocked settings helper - returns default settings
 * TODO: connect to backend API - replace with real settings persistence
 */

import { DEFAULT_FEATURE_FLAGS } from '@/store/theme'
import type { ColorTheme, FeatureFlag } from '@/store/theme'

export type AppSettings = {
	max_concurrent_downloads: number
	max_concurrent_writes: number

	theme: ColorTheme
	locale: string
	default_page: 'home' | 'library'
	collapsed_navigation: boolean
	hide_nametag_skins_page: boolean
	advanced_rendering: boolean
	native_decorations: boolean
	toggle_sidebar: boolean

	telemetry: boolean
	discord_rpc: boolean
	personalized_ads: boolean

	onboarded: boolean

	extra_launch_args: string[]
	custom_env_vars: [string, string][]
	memory: MemorySettings
	force_fullscreen: boolean
	game_resolution: WindowSize
	hide_on_process_start: boolean
	hooks: Hooks

	custom_dir?: string | null
	prev_custom_dir?: string | null
	migrated: boolean

	developer_mode: boolean
	feature_flags: Record<FeatureFlag, boolean>

	skipped_update: string | null
	pending_update_toast_for_version: string | null
	auto_download_updates: boolean | null

	version: number
}

export const DEFAULT_SETTINGS: AppSettings = {
	max_concurrent_downloads: 4,
	max_concurrent_writes: 2,

	theme: 'dark' as const,
	locale: 'en-US',
	default_page: 'home',
	collapsed_navigation: false,
	hide_nametag_skins_page: false,
	advanced_rendering: false,
	native_decorations: false,
	toggle_sidebar: false,

	telemetry: false,
	discord_rpc: false,
	personalized_ads: false,

	onboarded: true,

	extra_launch_args: [],
	custom_env_vars: [],
	memory: { maximum: 4096 },
	force_fullscreen: false,
	game_resolution: { width: 1920, height: 1080 },
	hide_on_process_start: false,
	hooks: {},

	developer_mode: false,
	feature_flags: DEFAULT_FEATURE_FLAGS,

	skipped_update: null,
	pending_update_toast_for_version: null,
	auto_download_updates: null,

	version: 1,
}

export async function get() {
	return DEFAULT_SETTINGS
}

export async function set(_settings: AppSettings) {
	// no-op
}

export async function cancel_directory_change(): Promise<void> {
	// no-op
}
