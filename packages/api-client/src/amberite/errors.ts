export type RecoveryDisposition = 'preserve_and_retry' | 'clear_session' | 'return_to_provider'
export type NativeAuthOperation = 'sign_in' | 'restore' | 'refresh' | 'sign_out'

export type AuthFailureCode =
	| 'offline'
	| 'timeout'
	| 'provider_unreachable'
	| 'amberite_unreachable'
	| 'invalid_session'
	| 'expired_session'
	| 'revoked_session'
	| 'corrupt_session'
	| 'refresh_reuse'
	| 'identity_mismatch'
	| 'identity_conflict'
	| 'cancelled'
	| 'state_failure'
	| 'xbox_restriction'
	| 'java_profile_missing'
	| 'throttled'
	| 'configuration_failure'
	| 'provider_failure'

export interface AuthFailurePayload {
	code: AuthFailureCode
	message: string
	recovery: RecoveryDisposition
}

export class AmberiteApiError extends Error {
	constructor(
		message: string,
		public readonly recovery: RecoveryDisposition = 'preserve_and_retry',
	) {
		super(message)
		this.name = 'AmberiteApiError'
	}
}

export class AmberiteNetworkError extends AmberiteApiError {
	constructor(
		message: string,
		public readonly code: Extract<
			AuthFailureCode,
			'offline' | 'timeout' | 'provider_unreachable' | 'amberite_unreachable'
		> = 'amberite_unreachable',
	) {
		super(`Network error: ${message}`, 'preserve_and_retry')
		this.name = 'AmberiteNetworkError'
	}
}

export class AmberiteAuthError extends AmberiteApiError {
	constructor(
		message: string,
		public readonly code: AuthFailureCode = 'invalid_session',
		recovery: RecoveryDisposition = 'clear_session',
	) {
		super(`Auth error: ${message}`, recovery)
		this.name = 'AmberiteAuthError'
	}
}

export class AmberiteProviderAuthError extends AmberiteAuthError {
	constructor(message: string, code: AuthFailureCode = 'provider_failure') {
		super(message, code, 'return_to_provider')
		this.name = 'AmberiteProviderAuthError'
	}
}

export class AmberiteConvexError extends AmberiteApiError {
	constructor(
		message: string,
		public readonly status: number,
		public readonly code?: string,
	) {
		super(`Convex error: ${message}`)
		this.name = 'AmberiteConvexError'
	}
}

export function authErrorFromResponse(message: string, status = 401): AmberiteApiError {
	const structured = parseAuthFailurePayload(message)
	if (structured) return authErrorFromPayload(structured)
	const normalized = message.toLowerCase()
	if (normalized.includes('expired')) return new AmberiteAuthError(message, 'expired_session')
	if (normalized.includes('revoked')) return new AmberiteAuthError(message, 'revoked_session')
	if (normalized.includes('refresh') && normalized.includes('reuse'))
		return new AmberiteAuthError(message, 'refresh_reuse')
	if (normalized.includes('ambiguousminecraftidentity'))
		return new AmberiteAuthError(message, 'identity_conflict')
	if (normalized.includes('uuid') && normalized.includes('mismatch'))
		return new AmberiteAuthError(message, 'identity_mismatch')
	if (normalized.includes('cancel')) return new AmberiteProviderAuthError(message, 'cancelled')
	if (normalized.includes('java') && normalized.includes('profile'))
		return new AmberiteProviderAuthError(message, 'java_profile_missing')
	if (normalized.includes('xbox')) return new AmberiteProviderAuthError(message, 'xbox_restriction')
	if (normalized.includes('throttl') || status === 429)
		return new AmberiteProviderAuthError(message, 'throttled')
	if (status === 408 || normalized.includes('timed out'))
		return new AmberiteNetworkError(message, 'timeout')
	if (status === 502 || status === 503 || status === 504)
		return new AmberiteNetworkError(message, 'amberite_unreachable')
	if (normalized.includes('not configured') || normalized.includes('configuration_failure'))
		return new AmberiteProviderAuthError(message, 'configuration_failure')
	if (status === 401 || status === 403 || normalized.includes('not authenticated'))
		return new AmberiteAuthError(message, 'invalid_session')
	return new AmberiteConvexError(message, status)
}

export function authErrorFromNative(
	error: unknown,
	operation: NativeAuthOperation,
): AmberiteApiError {
	const structured = parseAuthFailurePayload(error)
	if (structured) return authErrorFromPayload(structured)
	const message = nativeErrorMessage(error) ?? String(error)
	const normalized = message.toLowerCase()
	if (normalized.includes('cancel')) return new AmberiteProviderAuthError(message, 'cancelled')
	if (normalized.includes('ambiguousminecraftidentity'))
		return new AmberiteAuthError(message, 'identity_conflict')
	if (normalized.includes('uuid mismatch'))
		return new AmberiteAuthError(message, 'identity_mismatch')
	if (normalized.includes('java') && normalized.includes('profile'))
		return new AmberiteProviderAuthError(message, 'java_profile_missing')
	if (normalized.includes('xbox')) return new AmberiteProviderAuthError(message, 'xbox_restriction')
	if (normalized.includes('throttl') || normalized.includes('429'))
		return new AmberiteProviderAuthError(message, 'throttled')
	if (normalized.includes('not configured') || normalized.includes('configuration_failure'))
		return new AmberiteProviderAuthError(message, 'configuration_failure')
	if (
		normalized.includes('network') ||
		normalized.includes('connect') ||
		normalized.includes('timeout') ||
		normalized.includes('offline') ||
		normalized.includes('unreachable')
	)
		return new AmberiteNetworkError(message, 'amberite_unreachable')
	if (normalized.includes('refresh') && normalized.includes('reuse'))
		return new AmberiteAuthError(message, 'refresh_reuse')
	if (normalized.includes('expired')) return new AmberiteAuthError(message, 'expired_session')
	if (normalized.includes('revoked')) return new AmberiteAuthError(message, 'revoked_session')
	if (operation === 'sign_in') return new AmberiteProviderAuthError(message)
	if (operation === 'restore') return new AmberiteAuthError(message, 'corrupt_session')
	return new AmberiteNetworkError(message)
}

export function authErrorFromPayload(payload: AuthFailurePayload): AmberiteApiError {
	if (
		payload.code === 'offline' ||
		payload.code === 'timeout' ||
		payload.code === 'provider_unreachable' ||
		payload.code === 'amberite_unreachable'
	)
		return new AmberiteNetworkError(payload.message, payload.code)
	if (payload.recovery === 'return_to_provider')
		return new AmberiteProviderAuthError(payload.message, payload.code)
	return new AmberiteAuthError(payload.message, payload.code, payload.recovery)
}

export function parseAuthFailurePayload(value: unknown): AuthFailurePayload | null {
	if (isAuthFailurePayload(value)) return value
	const message = nativeErrorMessage(value)
	if (!message) return null
	for (let start = message.indexOf('{'); start >= 0; start = message.indexOf('{', start + 1)) {
		for (let end = message.lastIndexOf('}'); end > start; end = message.lastIndexOf('}', end - 1)) {
			try {
				const parsed: unknown = JSON.parse(message.slice(start, end + 1))
				if (isAuthFailurePayload(parsed)) return parsed
			} catch {
				// Native and Convex transports can prefix the structured payload.
			}
		}
	}
	return null
}

function isAuthFailurePayload(value: unknown): value is AuthFailurePayload {
	if (!value || typeof value !== 'object') return false
	const payload = value as Record<string, unknown>
	return (
		typeof payload.code === 'string' &&
		AUTH_FAILURE_CODES.has(payload.code as AuthFailureCode) &&
		typeof payload.message === 'string' &&
		typeof payload.recovery === 'string' &&
		RECOVERY_DISPOSITIONS.has(payload.recovery as RecoveryDisposition)
	)
}

function nativeErrorMessage(value: unknown): string | null {
	if (typeof value === 'string') return value
	if (value instanceof Error) return value.message
	if (value && typeof value === 'object' && 'message' in value) {
		const message = (value as { message?: unknown }).message
		if (typeof message === 'string') return message
	}
	return null
}

const AUTH_FAILURE_CODES = new Set<AuthFailureCode>([
	'offline',
	'timeout',
	'provider_unreachable',
	'amberite_unreachable',
	'invalid_session',
	'expired_session',
	'revoked_session',
	'corrupt_session',
	'refresh_reuse',
	'identity_mismatch',
	'identity_conflict',
	'cancelled',
	'state_failure',
	'xbox_restriction',
	'java_profile_missing',
	'throttled',
	'configuration_failure',
	'provider_failure',
])
const RECOVERY_DISPOSITIONS = new Set<RecoveryDisposition>([
	'preserve_and_retry',
	'clear_session',
	'return_to_provider',
])
