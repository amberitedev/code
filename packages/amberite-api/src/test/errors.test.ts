import { describe, it, expect } from 'vitest'
import {
	AmberiteApiError,
	NetworkError,
	AuthError,
	CoreOfflineError,
	RelayTimeoutError,
	CoreApiError,
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
