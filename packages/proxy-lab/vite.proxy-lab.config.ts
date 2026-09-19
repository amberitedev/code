import baseConfig from '../../apps/app-frontend/vite.config.ts'

const originalCsp = String(baseConfig.server?.headers?.['content-security-policy'] ?? '')
const proxyLabSources = 'http://127.0.0.1:8000 ws://127.0.0.1:8000'
const contentSecurityPolicy = originalCsp.replace('connect-src ', `connect-src ${proxyLabSources} `)

export default {
	...baseConfig,
	server: {
		...baseConfig.server,
		headers: {
			...baseConfig.server?.headers,
			'content-security-policy': contentSecurityPolicy,
		},
	},
}
