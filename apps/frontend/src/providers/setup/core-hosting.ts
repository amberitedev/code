import { createBrowserCoreHostingBackend, provideHostingBackend } from '@modrinth/ui'

export function setupCoreHostingProvider(auth: Awaited<ReturnType<typeof useAuth>>) {
	const config = useRuntimeConfig()
	provideHostingBackend(
		createBrowserCoreHostingBackend({
			coreUrl: () => config.public.amberiteCoreUrl,
			jwt: () => auth.value.token || config.public.amberiteCoreJwt || null,
			connectedCoreId: () => config.public.amberiteConnectedCoreId || null,
			convexUrl: config.public.amberiteConvexUrl,
		}),
	)
}
