import { areFriends } from '../common/auth'
import { invalid } from '../common/http'
import type { Env, UserRow } from '../common/types'

export const defaultPreferences = {
	appearance: { auto: true, theme: 'dark' },
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

export function preferences(user: UserRow): typeof defaultPreferences {
	const stored: Record<string, Record<string, unknown>> = JSON.parse(user.preferences)
	return {
		appearance: { ...defaultPreferences.appearance, ...stored.appearance },
		localization: { ...defaultPreferences.localization, ...stored.localization },
		layouts: { ...defaultPreferences.layouts, ...stored.layouts },
		sidebars: { ...defaultPreferences.sidebars, ...stored.sidebars },
		social: { ...defaultPreferences.social, ...stored.social },
	}
}

export function patchPreferences(user: UserRow, body: Record<string, unknown>) {
	const current = preferences(user)
	const next: Record<string, Record<string, unknown>> = structuredClone(current)
	for (const [section, value] of Object.entries(body)) {
		if (
			!Object.hasOwn(next, section) ||
			!value ||
			typeof value !== 'object' ||
			Array.isArray(value)
		)
			invalid('Invalid preferences')
		for (const [key, item] of Object.entries(value)) {
			if (!Object.hasOwn(next[section], key)) invalid('Invalid preference')
			const previous = next[section][key]
			if (typeof previous !== typeof item) invalid('Invalid preference value')
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
			next[section][key] = item
		}
	}
	return next
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
