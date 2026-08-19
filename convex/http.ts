import { httpRouter } from 'convex/server'
import { auth } from './auth'
import { verifyClaimHttp as coreSetupClaim } from './coreSetup'
import { syncSnapshot as coreProjectionSync } from './coreProjection'
import { handle as realtimeBridge } from './realtimeBridge'
import {
	handleDelete as sharedClientsDelete,
	handleGet as sharedClientsGet,
	handlePatch as sharedClientsPatch,
	handlePost as sharedClientsPost,
	handlePut as sharedClientsPut,
} from './sharedClientsHttp'

const http = httpRouter()

auth.addHttpRoutes(http)
http.route({ path: '/realtime/bridge', method: 'POST', handler: realtimeBridge })
http.route({ path: '/core/setup-claim', method: 'POST', handler: coreSetupClaim })
http.route({ path: '/core/projection-sync', method: 'POST', handler: coreProjectionSync })
http.route({ pathPrefix: '/v1/', method: 'GET', handler: sharedClientsGet })
http.route({ pathPrefix: '/v1/', method: 'POST', handler: sharedClientsPost })
http.route({ pathPrefix: '/v1/', method: 'PATCH', handler: sharedClientsPatch })
http.route({ pathPrefix: '/v1/', method: 'DELETE', handler: sharedClientsDelete })
http.route({ pathPrefix: '/v1/', method: 'PUT', handler: sharedClientsPut })

export default http
