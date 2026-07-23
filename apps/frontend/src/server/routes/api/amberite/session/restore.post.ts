import { defineEventHandler } from 'h3'

import { restoreBrowserSession } from '~/server/utils/amberite-minecraft-auth'

export default defineEventHandler(async (event) => {
	return await restoreBrowserSession(event)
})
