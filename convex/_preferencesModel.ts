import { type Infer, v } from 'convex/values'

const themeValidator = v.union(
	v.literal('light'),
	v.literal('dark'),
	v.literal('oled'),
	v.literal('retro'),
)
const layoutOptionValidator = v.union(v.literal('grid'), v.literal('rows'))
export const friendPrivacyValidator = v.union(
	v.literal('none'),
	v.literal('mutual'),
	v.literal('everyone'),
)
export const invitePrivacyValidator = v.union(
	v.literal('none'),
	v.literal('friends'),
	v.literal('everyone'),
)

const appearancePreferencesValidator = v.object({ auto: v.boolean(), theme: themeValidator })
const localizationPreferencesValidator = v.object({ locale: v.string() })
const layoutPreferencesValidator = v.object({
	mods: layoutOptionValidator,
	plugins: layoutOptionValidator,
	datapacks: layoutOptionValidator,
	shaders: layoutOptionValidator,
	resourcepacks: layoutOptionValidator,
	modpacks: layoutOptionValidator,
	servers: layoutOptionValidator,
	users: layoutOptionValidator,
})
const sidebarPreferencesValidator = v.object({
	right_aligned_search: v.boolean(),
	left_aligned_content: v.boolean(),
})
const socialPreferencesValidator = v.object({
	friend_privacy: friendPrivacyValidator,
	shared_instances_privacy: invitePrivacyValidator,
	hosting_access_privacy: invitePrivacyValidator,
})

export const userPreferencesValidator = v.object({
	appearance: appearancePreferencesValidator,
	localization: localizationPreferencesValidator,
	layouts: layoutPreferencesValidator,
	sidebars: sidebarPreferencesValidator,
	social: socialPreferencesValidator,
})

export const partialUserPreferencesValidator = v.object({
	appearance: v.optional(
		v.object({ auto: v.optional(v.boolean()), theme: v.optional(themeValidator) }),
	),
	localization: v.optional(v.object({ locale: v.optional(v.string()) })),
	layouts: v.optional(
		v.object({
			mods: v.optional(layoutOptionValidator),
			plugins: v.optional(layoutOptionValidator),
			datapacks: v.optional(layoutOptionValidator),
			shaders: v.optional(layoutOptionValidator),
			resourcepacks: v.optional(layoutOptionValidator),
			modpacks: v.optional(layoutOptionValidator),
			servers: v.optional(layoutOptionValidator),
			users: v.optional(layoutOptionValidator),
		}),
	),
	sidebars: v.optional(
		v.object({
			right_aligned_search: v.optional(v.boolean()),
			left_aligned_content: v.optional(v.boolean()),
		}),
	),
	social: v.optional(
		v.object({
			friend_privacy: v.optional(friendPrivacyValidator),
			shared_instances_privacy: v.optional(invitePrivacyValidator),
			hosting_access_privacy: v.optional(invitePrivacyValidator),
		}),
	),
})

export type UserPreferences = Infer<typeof userPreferencesValidator>
export type PartialUserPreferences = Infer<typeof partialUserPreferencesValidator>

export const DEFAULT_USER_PREFERENCES: UserPreferences = {
	appearance: { auto: false, theme: 'dark' },
	localization: { locale: 'en-US' },
	layouts: {
		mods: 'rows',
		plugins: 'rows',
		datapacks: 'rows',
		shaders: 'grid',
		resourcepacks: 'grid',
		modpacks: 'rows',
		servers: 'rows',
		users: 'rows',
	},
	sidebars: { right_aligned_search: false, left_aligned_content: false },
	social: {
		friend_privacy: 'everyone',
		shared_instances_privacy: 'everyone',
		hosting_access_privacy: 'everyone',
	},
}

export function mergeUserPreferences(
	current: UserPreferences,
	patch: PartialUserPreferences,
): UserPreferences {
	return {
		appearance: { ...current.appearance, ...patch.appearance },
		localization: { ...current.localization, ...patch.localization },
		layouts: { ...current.layouts, ...patch.layouts },
		sidebars: { ...current.sidebars, ...patch.sidebars },
		social: { ...current.social, ...patch.social },
	}
}
