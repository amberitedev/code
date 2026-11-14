import { backend } from '@amberite/contracts'

import { accountRoutes } from './accounts'
import { ApiError, errorResponse, routePath } from './common/http'
import { noBody, route, serve } from './common/router'
import type { Env } from './common/types'
import { coreRoutes } from './cores'
import { devRoutes } from './dev'
import { handleSharing } from './sharing'
import { socialRoutes } from './social'
export { CoreSignIn } from './cores/sign-in'
export { FriendsHub } from './social/socket'

const routes = [
	route(backend.CheckHealth, noBody, async ({ env }) => {
		await env.DB.prepare('SELECT 1 FROM users LIMIT 1').first()
	}),
	...devRoutes,
	...coreRoutes,
]

const labrinthRoutes = [...accountRoutes, ...socialRoutes]

/**
 * labrinth's routes, under any of the `/v2`, `/v3` and `/_internal` prefixes upstream uses. Any
 * other `/auth/` route is refused here, so no sign-in falls through to another handler.
 */
async function serveLabrinth(request: Request, env: Env): Promise<Response | null> {
	const path = routePath(request).replace(/^\/_internal(?=\/)/, '')
	const response = await serve(labrinthRoutes, request, env, path)
	if (response || !path.startsWith('/auth/')) return response
	throw new ApiError(
		501,
		'unsupported_operation',
		'This authentication method has not been configured on this backend',
	)
}

function corsOrigin(request: Request, env: Env): string | null {
	const origin = request.headers.get('Origin')
	if (!origin) return null
	if ((env.CORS_ORIGINS ?? '').split(',').includes(origin)) return origin
	if (env.LOCAL_DEV === 'true') {
		try {
			const parsed = new URL(origin)
			if (
				['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname) ||
				parsed.hostname.endsWith('.localhost') ||
				origin === 'tauri://localhost'
			)
				return origin
		} catch {
			return null
		}
	}
	return null
}

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		const origin = corsOrigin(request, env)
		let response: Response
		try {
			if (request.method === 'OPTIONS') response = new Response(null, { status: 204 })
			else
				response =
					(await handleSharing(request, env)) ??
					(await serve(routes, request, env)) ??
					(await serveLabrinth(request, env)) ??
					errorResponse(new ApiError(404, 'not_found', 'Not found'))
		} catch (error) {
			if (error instanceof ApiError) response = errorResponse(error)
			else {
				console.error(
					'Backend request failed',
					request.method,
					new URL(request.url).pathname,
					error,
				)
				response = errorResponse(new ApiError(500, 'internal_error', 'An internal error occurred'))
			}
		}
		if (response.status === 101) return response
		if (origin) {
			response.headers.set('Access-Control-Allow-Origin', origin)
			response.headers.set('Vary', 'Origin')
			response.headers.set('Access-Control-Allow-Methods', 'GET, POST, PUT, PATCH, DELETE, OPTIONS')
			response.headers.set(
				'Access-Control-Allow-Headers',
				'Authorization, Content-Type, Range, X-Local-Dev-Secret, X-Requested-With, X-Panel-Version',
			)
			response.headers.set('Access-Control-Expose-Headers', 'Content-Range, Accept-Ranges, ETag')
		}
		return response
	},
} satisfies ExportedHandler<Env>
