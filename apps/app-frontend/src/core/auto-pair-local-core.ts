import type { AmberiteAccountUser } from '@modrinth/api-client'

import { config } from '@/config'
import { setConnectedCore } from '@/core/connected-core'
import { getDevAppConfig } from '@/dev/runtime'
import { amberite } from '@/services/amberite'
import { apiClient } from '@/services/api-client'

let pairing: Promise<void> | null = null

export function autoPairLocalDevelopmentCore(user: AmberiteAccountUser) {
	const devConfig = getDevAppConfig()
	if (devConfig?.authMode !== 'dev' || devConfig.username !== 'scenario_1') return Promise.resolve()
	if (!pairing) pairing = pair(devConfig.coreUrl, user).finally(() => (pairing = null))
	return pairing
}

async function pair(coreUrl: string, user: AmberiteAccountUser) {
	const linked = (await apiClient.amberite.cores_v1.list()).find(
		(core) => core.isOwner && core.connectionUrl === coreUrl,
	)
	const localSetupSecret = await waitForLocalSetupSecret()
	if (!localSetupSecret) {
		if (linked) connect(linked.coreId, coreUrl)
		return
	}

	const setup = await amberite.core.completeSetupAt(coreUrl, {
		local_setup_secret: localSetupSecret,
		convex_url: config.convexUrl,
		auth_jwks_url: `${config.convexSiteUrl.replace(/\/$/, '')}/.well-known/jwks.json`,
		auth_audience: 'convex',
		owner_user_id: user.userId,
		owner_display_name: user.name,
	})
	await apiClient.amberite.cores_v1.linkLocalDevelopmentCore(setup.core_id, coreUrl)
	connect(setup.core_id, coreUrl)
}

async function waitForLocalSetupSecret() {
	for (let attempt = 0; attempt < 8; attempt++) {
		const secret = await amberite.core.adapter.getLocalSetupSecret?.()
		if (secret) return secret
		await new Promise((resolve) => setTimeout(resolve, 250))
	}
	return null
}

function connect(coreId: string, coreUrl: string) {
	setConnectedCore({ coreId, url: coreUrl })
	amberite.core.clearCoreUrlCache()
}
