import { tokenFrom } from '../common/auth'
import { serve } from '../common/router'
import type { Env } from '../common/types'
import { coreKeyRoutes, readTicketRoutes } from './core'
import { memberRoutes, publicInviteRoutes } from './invites'
import { moderationRoutes } from './moderation'
import { playerRoutes } from './servers'
import { publicShareRoutes, shareRoutes } from './shares'
import { versionRoutes } from './versions'

// Sharing metadata, as Modrinth's App expects it. The files themselves live on Cores; see pool.ts.

export { forgetServer } from './servers'
export { sweepFiles } from './shares'
export { versionReady } from './versions'

/** Anyone may call these, signed in or not. */
const publicRoutes = [...publicInviteRoutes, ...publicShareRoutes]

const accountRoutes = [
	...shareRoutes,
	...memberRoutes,
	...versionRoutes,
	...readTicketRoutes,
	...moderationRoutes,
	...playerRoutes,
]

/**
 * The shared instance routes. A Core key reaches only its own (`coreKeyRoutes`), dispatched before
 * any account check.
 */
export async function handleSharing(request: Request, env: Env): Promise<Response | null> {
	return (
		(await serve(publicRoutes, request, env)) ??
		(await serve(
			tokenFrom(request).startsWith('amc_') ? coreKeyRoutes : accountRoutes,
			request,
			env,
		))
	)
}
