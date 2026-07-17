const whitelistedParams = ['flow', 'error']

export default defineNuxtRouteMiddleware(async (to) => {
	const auth = await useAuth()
	if (auth.value.status === 'authenticated') return
	if (auth.value.status === 'restoring' || auth.value.status === 'retryableOffline') return

	const url = new URL(to.fullPath, useRuntimeConfig().public.siteUrl)
	const extractedParams = Object.create(null) as Record<string, string>
	for (const param of whitelistedParams) {
		const value = url.searchParams.get(param)
		if (value != null) {
			extractedParams[param] = value
			url.searchParams.delete(param)
		}
	}

	return await navigateTo(
		{
			path: '/auth/sign-in',
			query: {
				redirect: `${url.pathname}${url.search}`,
				...extractedParams,
			},
		},
		{ replace: true },
	)
})
