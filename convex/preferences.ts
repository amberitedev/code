import { mutation, query } from './_generated/server'
import { getUserPreferences, updateUserPreferences } from './_preferences'
import { partialUserPreferencesValidator, userPreferencesValidator } from './_preferencesModel'
import { requireUserId } from './_socialRules'

export const getCurrent = query({
	args: {},
	returns: userPreferencesValidator,
	handler: async (ctx) => await getUserPreferences(ctx, await requireUserId(ctx)),
})

export const editCurrent = mutation({
	args: { preferences: partialUserPreferencesValidator },
	returns: userPreferencesValidator,
	handler: async (ctx, args) =>
		await updateUserPreferences(ctx, await requireUserId(ctx), args.preferences),
})
