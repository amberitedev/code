import { createBrowserCoreHostingBackend, provideHostingBackend } from '@modrinth/ui'

const LOCAL_CORE_URL = 'http://localhost:16662'

export function setupCoreHostingProvider() {
	provideHostingBackend(
		createBrowserCoreHostingBackend({
			coreUrl: LOCAL_CORE_URL,
		}),
	)
}
