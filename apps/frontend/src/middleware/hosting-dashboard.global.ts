export default defineNuxtRouteMiddleware((to) => {
	if (to.path === '/hosting' || to.path === '/hosting/') {
		return navigateTo('/hosting/manage', { redirectCode: 302 })
	}
})
