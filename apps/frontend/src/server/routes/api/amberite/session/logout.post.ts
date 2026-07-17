import { defineEventHandler } from 'h3'

import { signOutBrowserSession } from '~/server/utils/amberite-minecraft-auth'

export default defineEventHandler(async (event) => {
	await signOutBrowserSession(event)
	return { ok: true }
})
