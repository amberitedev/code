import { defineEventHandler, getQuery, sendRedirect, setCookie } from 'h3'

import {
	MINECRAFT_AUTH_MODE_COOKIE,
	MINECRAFT_AUTH_REDIRECT_COOKIE,
	MINECRAFT_AUTH_STATE_COOKIE,
	MINECRAFT_AUTH_VERIFIER_COOKIE,
	minecraftRedirectUri,
	normalizeLocalRedirect,
	normalizeMinecraftAuthMode,
	pkceChallenge,
	randomBase64Url,
	requiredMinecraftClientId,
	temporaryCookieOptions,
} from '~/server/utils/amberite-minecraft-auth'

export default defineEventHandler(async (event) => {
	const query = getQuery(event)
	const mode = normalizeMinecraftAuthMode(query.mode)
	const redirect = normalizeLocalRedirect(query.redirect)
	const state = randomBase64Url(32)
	const verifier = randomBase64Url(64)
	const challenge = await pkceChallenge(verifier)
	const cookieOptions = temporaryCookieOptions(event)

	setCookie(event, MINECRAFT_AUTH_STATE_COOKIE, state, cookieOptions)
	setCookie(event, MINECRAFT_AUTH_VERIFIER_COOKIE, verifier, cookieOptions)
	setCookie(event, MINECRAFT_AUTH_MODE_COOKIE, mode, cookieOptions)
	setCookie(event, MINECRAFT_AUTH_REDIRECT_COOKIE, redirect, cookieOptions)

	const url = new URL('https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize')
	url.searchParams.set('client_id', requiredMinecraftClientId(event))
	url.searchParams.set('response_type', 'code')
	url.searchParams.set('redirect_uri', minecraftRedirectUri(event))
	url.searchParams.set('scope', 'XboxLive.signin')
	url.searchParams.set('state', state)
	url.searchParams.set('code_challenge', challenge)
	url.searchParams.set('code_challenge_method', 'S256')
	url.searchParams.set('response_mode', 'query')
	url.searchParams.set('prompt', 'select_account')

	return sendRedirect(event, url.toString(), 302)
})
