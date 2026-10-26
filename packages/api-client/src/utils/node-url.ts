const NODE_FS_PATH_REGEX = /\/modrinth\/v\d+\/fs\/?$/
const HTTP_SCHEME_REGEX = /^https?:\/\//i
const WS_SCHEME_REGEX = /^wss?:\/\//i
const HTTP_SECURE_SCHEME_REGEX = /^https:\/\//i
const HTTP_INSECURE_SCHEME_REGEX = /^http:\/\//i

export function getNodeBaseUrl(url: string): string {
	const baseUrl = url.replace(NODE_FS_PATH_REGEX, '')
	return HTTP_SCHEME_REGEX.test(baseUrl) ? baseUrl : `https://${baseUrl}`
}

export function getNodeWebSocketUrl(url: string): string {
	if (WS_SCHEME_REGEX.test(url)) return url
	if (HTTP_SECURE_SCHEME_REGEX.test(url)) return url.replace(HTTP_SECURE_SCHEME_REGEX, 'wss://')
	if (HTTP_INSECURE_SCHEME_REGEX.test(url)) return url.replace(HTTP_INSECURE_SCHEME_REGEX, 'ws://')

	return `wss://${url}`
}

/**
 * Where a Core serves a server's node routes (files, downloads, the console socket): under
 * `/servers/{id}` of the base its requests go to, which is the App's local proxy, with its scheme
 * and its capability path. A Core names the host it was reached at instead, which the App cannot
 * use, so the client puts this in its place.
 */
export function coreNodeUrl(base: string, serverId: string): string {
	return `${base.replace(/\/$/, '')}/servers/${serverId}`
}
