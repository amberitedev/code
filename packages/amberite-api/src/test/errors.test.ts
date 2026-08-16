import { describe, it, expect } from 'vitest'
import {
	AmberiteApiError,
	NetworkError,
	AuthError,
	ProviderAuthError,
	ConvexError,
	CoreOfflineError,
	RelayTimeoutError,
	CoreApiError,
	authErrorFromNative,
	authErrorFromResponse,
	parseAuthFailurePayload,
} from '../errors'

describe('AmberiteApiError', () => {
	it('is an instance of Error', () => {
		expect(new AmberiteApiError('test') instanceof Error).toBe(true)
	})

	it('sets name correctly', () => {
		expect(new AmberiteApiError('test').name).toBe('AmberiteApiError')
	})

	it('preserves the message', () => {
		expect(new AmberiteApiError('something went wrong').message).toBe('something went wrong')
	})
})

describe('NetworkError', () => {
	it('is an instance of AmberiteApiError', () => {
		expect(new NetworkError('timeout') instanceof AmberiteApiError).toBe(true)
	})

	it('sets name correctly', () => {
		expect(new NetworkError('timeout').name).toBe('NetworkError')
	})

	it('prefixes message with "Network error:"', () => {
		expect(new NetworkError('connection refused').message).toBe('Network error: connection refused')
	})
})

describe('AuthError', () => {
	it('is an instance of AmberiteApiError', () => {
		expect(new AuthError('expired') instanceof AmberiteApiError).toBe(true)
	})

	it('sets name correctly', () => {
		expect(new AuthError('expired').name).toBe('AuthError')
	})

	it('prefixes message with "Auth error:"', () => {
		expect(new AuthError('token expired').message).toBe('Auth error: token expired')
	})
})

describe('recovery dispositions', () => {
	it('preserves network failures and clears terminal sessions', () => {
		expect(new NetworkError('offline', 'offline').recovery).toBe('preserve_and_retry')
		expect(new AuthError('revoked', 'revoked_session').recovery).toBe('clear_session')
	})

	it('returns provider failures to Minecraft sign-in', () => {
		expect(new ProviderAuthError('missing Java', 'java_profile_missing').recovery).toBe(
			'return_to_provider',
		)
		expect(new ConvexError('rejected', 500).recovery).toBe('preserve_and_retry')
	})
})

describe('authErrorFromResponse', () => {
	it('classifies provider, identity, and service failures by recovery action', () => {
		expect(authErrorFromResponse('MinecraftProviderUnavailable', 500)).toMatchObject({
			code: 'provider_unreachable',
			recovery: 'preserve_and_retry',
		})
		expect(authErrorFromResponse('MinecraftUuidMismatch', 400)).toMatchObject({
			code: 'identity_mismatch',
			recovery: 'clear_session',
		})
		expect(authErrorFromResponse('MinecraftJavaProfileMissing', 400)).toMatchObject({
			code: 'java_profile_missing',
			recovery: 'return_to_provider',
		})
	})

	it('prefers a structured failure payload over transport prefixes', () => {
		const message =
			'Convex action failed: {"code":"identity_mismatch","message":"Wrong account","recovery":"clear_session"}'
		expect(parseAuthFailurePayload(message)).toEqual({
			code: 'identity_mismatch',
			message: 'Wrong account',
			recovery: 'clear_session',
		})
		expect(authErrorFromResponse(message, 400)).toMatchObject({
			code: 'identity_mismatch',
			recovery: 'clear_session',
		})
	})

	it('distinguishes corrupt duplicate identity claims from choosing the wrong account', () => {
		const message =
			'Convex action failed: {"code":"identity_conflict","message":"Multiple Amberite accounts claim this Minecraft identity","recovery":"clear_session"}'
		expect(parseAuthFailurePayload(message)).toEqual({
			code: 'identity_conflict',
			message: 'Multiple Amberite accounts claim this Minecraft identity',
			recovery: 'clear_session',
		})
		expect(authErrorFromResponse('AmbiguousMinecraftIdentity', 400)).toMatchObject({
			code: 'identity_conflict',
			recovery: 'clear_session',
		})
	})

	it('does not mistake a provider 500 error page for missing configuration', () => {
		const message =
			'Minecraft authentication error during step SisuAuthenticate. Status Code: 500 Internal Server Error. The server encountered an internal error or misconfiguration.'
		expect(authErrorFromResponse(message, 500)).toMatchObject({
			code: 'provider_unreachable',
			recovery: 'preserve_and_retry',
		})
		expect(authErrorFromNative(message, 'sign_in')).toMatchObject({
			code: 'provider_unreachable',
			recovery: 'preserve_and_retry',
		})
	})
})

describe('CoreOfflineError', () => {
	it('is an instance of AmberiteApiError', () => {
		expect(new CoreOfflineError() instanceof AmberiteApiError).toBe(true)
	})

	it('sets name correctly', () => {
		expect(new CoreOfflineError().name).toBe('CoreOfflineError')
	})

	it('has fixed "Core is offline" message', () => {
		expect(new CoreOfflineError().message).toBe('Core is offline')
	})
})

describe('RelayTimeoutError', () => {
	it('is an instance of AmberiteApiError', () => {
		expect(new RelayTimeoutError() instanceof AmberiteApiError).toBe(true)
	})

	it('sets name correctly', () => {
		expect(new RelayTimeoutError().name).toBe('RelayTimeoutError')
	})
})

describe('CoreApiError', () => {
	it('is an instance of AmberiteApiError', () => {
		expect(new CoreApiError(404, 'not found') instanceof AmberiteApiError).toBe(true)
	})

	it('sets name correctly', () => {
		expect(new CoreApiError(404, 'not found').name).toBe('CoreApiError')
	})

	it('exposes status and coreMessage', () => {
		const err = new CoreApiError(422, 'invalid body')
		expect(err.status).toBe(422)
		expect(err.coreMessage).toBe('invalid body')
	})

	it('formats message as "Core API <status>: <coreMessage>"', () => {
		expect(new CoreApiError(404, 'not found').message).toBe('Core API 404: not found')
	})
})
