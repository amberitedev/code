export function getHostingServerAddress(
	net: { ip: string | null; port: number; domain?: string | null } | null | undefined,
	subdomain?: string,
) {
	const domain = net?.domain || subdomain
	if (domain) return domain.includes('.') ? domain : `${domain}.modrinth.gg`
	if (!net?.ip) return ''
	const host = net.ip.includes(':') && !net.ip.startsWith('[') ? `[${net.ip}]` : net.ip
	return net.port && net.port !== 25565 ? `${host}:${net.port}` : host
}
