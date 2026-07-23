import { defineEventHandler, getQuery, sendRedirect } from 'h3'

import {
	minecraftRedirectUri,
	normalizeLocalRedirect,
	normalizeMinecraftAuthIntent,
	normalizeMinecraftUuid,
	noStore,
	pkceChallenge,
	randomBase64Url,
	requiredMinecraftClientId,
	setMinecraftAuthFlow,
} from '~/server/utils/amberite-minecraft-auth'

export default defineEventHandler(async (event) => {
	noStore(event)
	const query = getQuery(event)
	const intent = normalizeMinecraftAuthIntent(query.intent)
	const redirect = normalizeLocalRedirect(query.redirect)
	const expectedMinecraftUuid =
		intent === 'continue' ? normalizeMinecraftUuid(query.expectedMinecraftUuid) : undefined
	const state = randomBase64Url(32)
	const verifier = randomBase64Url(64)
	const challenge = await pkceChallenge(verifier)

	await setMinecraftAuthFlow(event, {
		state,
		verifier,
		intent,
		expectedMinecraftUuid,
		redirect,
		expiresAt: Date.now() + 10 * 60 * 1_000,
	})

	const url = new URL('https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize')
	url.searchParams.set('client_id', requiredMinecraftClientId(event))
	url.searchParams.set('response_type', 'code')
	url.searchParams.set('redirect_uri', minecraftRedirectUri(event))
	url.searchParams.set('scope', 'XboxLive.signin offline_access')
	url.searchParams.set('state', state)
	url.searchParams.set('code_challenge', challenge)
	url.searchParams.set('code_challenge_method', 'S256')
	url.searchParams.set('response_mode', 'query')
	url.searchParams.set('prompt', intent === 'use_another_account' ? 'select_account' : 'login')

	return sendRedirect(event, url.toString(), 302)
})
