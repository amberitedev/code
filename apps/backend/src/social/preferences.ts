import type { PreferencesUpdate, UserPreferences } from '@amberite/contracts'

import { areFriends } from '../common/auth'
import { invalid, objectBody } from '../common/http'
import type { Env, UserRow } from '../common/types'

export const defaultPreferences: UserPreferences = {
	appearance: { auto: false, theme: 'dark' },
	behavior: {
		minimize_app: false,
		refocus_on_game_close: false,
		hide_right_sidebar: false,
		show_jump_in: true,
		compact_instance_cards: false,
		show_play_time: true,
		hide_nametag: false,
		show_all_screenshots: true,
		show_files_tab_in_instances: true,
		show_worlds_tab_in_instances: true,
		show_screenshots_tab_in_instances: false,
		show_skin_selector_in_sidebar: true,
		quick_instance_count: 20,
		warn_on_unknown_modpacks: true,
		skip_non_essential_warnings: false,
	},
	localization: { locale: 'en-US' },
	layouts: {
		mods: 'grid',
		plugins: 'grid',
		datapacks: 'grid',
		shaders: 'grid',
		resourcepacks: 'grid',
		modpacks: 'grid',
		servers: 'grid',
		users: 'grid',
	},
	sidebars: { right_aligned_search: false, left_aligned_content: false },
	social: {
		friend_privacy: 'everyone',
		shared_instances_privacy: 'friends',
		hosting_access_privacy: 'friends',
	},
}

/** `update` over `base`, section by section. */
function merged(base: UserPreferences, update: PreferencesUpdate): UserPreferences {
	return {
		appearance: { ...base.appearance, ...update.appearance },
		behavior: { ...base.behavior, ...update.behavior },
		localization: { ...base.localization, ...update.localization },
		layouts: { ...base.layouts, ...update.layouts },
		sidebars: { ...base.sidebars, ...update.sidebars },
		social: { ...base.social, ...update.social },
	}
}

export function preferences(user: UserRow): UserPreferences {
	// Only `EditPreferences` writes it, with values `parsePreferencesUpdate` checked.
	const stored: PreferencesUpdate = JSON.parse(user.preferences)
	return merged(defaultPreferences, stored)
}

export function patchPreferences(user: UserRow, update: PreferencesUpdate): UserPreferences {
	return merged(preferences(user), update)
}

export function parsePreferencesUpdate(raw: unknown): PreferencesUpdate {
	const body = objectBody(raw)
	assertPreferencesUpdate(body)
	return body
}

function assertPreferencesUpdate(body: Record<string, unknown>): asserts body is PreferencesUpdate {
	const defaults: Record<string, Record<string, unknown>> = defaultPreferences
	for (const [section, value] of Object.entries(body)) {
		if (
			!Object.hasOwn(defaults, section) ||
			!value ||
			typeof value !== 'object' ||
			Array.isArray(value)
		)
			invalid('Invalid preferences')
		for (const [key, item] of Object.entries(value)) {
			if (!Object.hasOwn(defaults[section], key)) invalid('Invalid preference')
			if (typeof defaults[section][key] !== typeof item) invalid('Invalid preference value')
			if (
				section === 'behavior' &&
				key === 'quick_instance_count' &&
				(typeof item !== 'number' || !Number.isInteger(item) || item < 0 || item > 255)
			)
				invalid('Invalid quick instance count')
			if (
				section === 'appearance' &&
				key === 'theme' &&
				!['light', 'dark', 'oled', 'retro'].includes(String(item))
			)
				invalid('Invalid theme')
			if (section === 'layouts' && !['grid', 'rows'].includes(String(item)))
				invalid('Invalid layout')
			if (
				section === 'social' &&
				!(
					key === 'friend_privacy'
						? ['none', 'mutual', 'everyone']
						: ['none', 'friends', 'everyone']
				).includes(String(item))
			)
				invalid('Invalid privacy setting')
			if (
				section === 'localization' &&
				(typeof item !== 'string' || item.length > 35 || !/^[a-zA-Z0-9-]+$/.test(item))
			)
				invalid('Invalid locale')
		}
	}
}

export async function blockedEitherWay(env: Env, first: string, second: string) {
	return Boolean(
		await env.DB.prepare(
			'SELECT 1 FROM blocks WHERE (user_id = ? AND blocked_id = ?) OR (user_id = ? AND blocked_id = ?)',
		)
			.bind(first, second, second, first)
			.first(),
	)
}

export async function canInviteToSharedInstance(
	env: Env,
	senderId: string,
	recipientId: string,
): Promise<boolean> {
	if (senderId === recipientId || (await blockedEitherWay(env, senderId, recipientId))) return false
	const user = await env.DB.prepare('SELECT * FROM users WHERE id = ?')
		.bind(recipientId)
		.first<UserRow>()
	if (!user) return false
	const privacy = preferences(user).social.shared_instances_privacy
	return (
		privacy === 'everyone' ||
		(privacy === 'friends' && (await areFriends(env, senderId, recipientId)))
	)
}
