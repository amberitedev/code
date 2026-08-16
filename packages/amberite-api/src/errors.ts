/** Structured, recovery-aware errors for @amberite/amberite-api. */

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

export class NetworkError extends AmberiteApiError {
	constructor(
		message: string,
		public readonly code: Extract<
			AuthFailureCode,
			'offline' | 'timeout' | 'provider_unreachable' | 'amberite_unreachable'
		> = 'amberite_unreachable',
	) {
		super(`Network error: ${message}`, 'preserve_and_retry')
		this.name = 'NetworkError'
	}
}

export class AuthError extends AmberiteApiError {
	constructor(
		message: string,
		public readonly code: AuthFailureCode = 'invalid_session',
		recovery: RecoveryDisposition = 'clear_session',
	) {
		super(`Auth error: ${message}`, recovery)
		this.name = 'AuthError'
	}
}

export class ProviderAuthError extends AuthError {
	constructor(message: string, code: AuthFailureCode = 'provider_failure') {
		super(message, code, 'return_to_provider')
		this.name = 'ProviderAuthError'
	}
}

export class ConvexError extends AmberiteApiError {
	constructor(
		message: string,
		public readonly status: number,
		public readonly code?: string,
	) {
		super(`Convex error: ${message}`, 'preserve_and_retry')
		this.name = 'ConvexError'
	}
}

export class CoreOfflineError extends AmberiteApiError {
	constructor() {
		super('Core is offline', 'preserve_and_retry')
		this.name = 'CoreOfflineError'
	}
}

export class RelayTimeoutError extends AmberiteApiError {
	constructor() {
		super('Relay timed out waiting for acknowledgment', 'preserve_and_retry')
		this.name = 'RelayTimeoutError'
	}
}

export class CoreApiError extends AmberiteApiError {
	constructor(
		public readonly status: number,
		public readonly coreMessage: string,
	) {
		super(`Core API ${status}: ${coreMessage}`)
		this.name = 'CoreApiError'
	}
}

export function authErrorFromResponse(message: string, status = 401): AmberiteApiError {
	const structured = parseAuthFailurePayload(message)
	if (structured) return authErrorFromPayload(structured)

	const normalized = message.toLowerCase()
	if (normalized.includes('expired')) return new AuthError(message, 'expired_session')
	if (normalized.includes('revoked')) return new AuthError(message, 'revoked_session')
	if (normalized.includes('refresh') && normalized.includes('reuse'))
		return new AuthError(message, 'refresh_reuse')
	if (
		(normalized.includes('uuid') && normalized.includes('mismatch')) ||
		normalized.includes('minecrafthandleconflict')
	) {
		return new AuthError(message, 'identity_mismatch')
	}
	if (
		normalized.includes('ambiguousminecraftidentity') ||
		normalized.includes('multiple amberite accounts claim')
	)
		return new AuthError(message, 'identity_conflict')
	if (normalized.includes('cancel')) return new ProviderAuthError(message, 'cancelled')
	if (normalized.includes('state') && normalized.includes('fail'))
		return new ProviderAuthError(message, 'state_failure')
	if (normalized.includes('java') && normalized.includes('profile'))
		return new ProviderAuthError(message, 'java_profile_missing')
	if (normalized.includes('xbox')) return new ProviderAuthError(message, 'xbox_restriction')
	if (normalized.includes('throttl') || status === 429)
		return new ProviderAuthError(message, 'throttled')
	if (
		(status >= 500 &&
			status < 600 &&
			(normalized.includes('minecraft authentication') ||
				normalized.includes('minecraft provider') ||
				normalized.includes('sisuauthenticate'))) ||
		/status code:\s*5\d{2}\b/.test(normalized) ||
		normalized.includes('internal server error')
	)
		return new NetworkError(message, 'provider_unreachable')
	if (
		normalized.includes('configuration_failure') ||
		normalized.includes('not configured') ||
		normalized.includes('missing required environment variable') ||
		normalized.includes('client id') ||
		normalized.includes('minecraftidentitymigrationrequired')
	) {
		return new ProviderAuthError(message, 'configuration_failure')
	}
	if (normalized.includes('minecraftproviderunavailable'))
		return new NetworkError(message, 'provider_unreachable')
	if (status === 408 || normalized.includes('timed out'))
		return new NetworkError(message, 'timeout')
	if (status === 502 || status === 503 || status === 504)
		return new NetworkError(message, 'amberite_unreachable')
	if (
		normalized.includes('minecraftaccesstokeninvalid') ||
		normalized.includes('invalid minecraft uuid') ||
		normalized.includes('invalid minecraft handle')
	) {
		return new ProviderAuthError(message, 'provider_failure')
	}
	if (status === 401 || status === 403 || normalized.includes('not authenticated'))
		return new AuthError(message, 'invalid_session')
	return new ConvexError(message, status)
}

export function authErrorFromNative(
	error: unknown,
	operation: NativeAuthOperation,
): AmberiteApiError {
	const structured = parseAuthFailurePayload(error)
	if (structured) return authErrorFromPayload(structured)
	const message = nativeErrorMessage(error) ?? String(error)
	const normalized = message.toLowerCase()
	if (normalized.includes('cancel')) return new ProviderAuthError(message, 'cancelled')
	if (
		operation === 'sign_in' &&
		(normalized.includes('oauth state') || normalized.includes('state mismatch'))
	)
		return new ProviderAuthError(message, 'state_failure')
	if (
		normalized.includes('ambiguousminecraftidentity') ||
		normalized.includes('multiple amberite accounts claim')
	)
		return new AuthError(message, 'identity_conflict')
	if (normalized.includes('uuid mismatch')) return new AuthError(message, 'identity_mismatch')
	if (normalized.includes('java') && normalized.includes('profile'))
		return new ProviderAuthError(message, 'java_profile_missing')
	if (normalized.includes('throttl') || normalized.includes('429'))
		return new ProviderAuthError(message, 'throttled')
	if (/status code:\s*5\d{2}\b/.test(normalized) || normalized.includes('internal server error'))
		return new NetworkError(message, 'provider_unreachable')
	if (normalized.includes('xbox')) return new ProviderAuthError(message, 'xbox_restriction')
	if (
		normalized.includes('configuration_failure') ||
		normalized.includes('not configured') ||
		normalized.includes('missing required environment variable') ||
		normalized.includes('client id')
	)
		return new ProviderAuthError(message, 'configuration_failure')
	if (
		normalized.includes('network') ||
		normalized.includes('connect') ||
		normalized.includes('timeout') ||
		normalized.includes('offline') ||
		normalized.includes('unreachable') ||
		normalized.includes('error sending request')
	)
		return new NetworkError(message, 'amberite_unreachable')
	if (
		normalized.includes('corrupt') ||
		normalized.includes('decrypt') ||
		normalized.includes('keyring') ||
		normalized.includes('missing key') ||
		normalized.includes('incomplete') ||
		normalized.includes('bundle authentication')
	)
		return new AuthError(message, 'corrupt_session')
	if (normalized.includes('refresh') && normalized.includes('reuse'))
		return new AuthError(message, 'refresh_reuse')
	if (normalized.includes('expired')) return new AuthError(message, 'expired_session')
	if (normalized.includes('revoked')) return new AuthError(message, 'revoked_session')
	if (
		normalized.includes('not authenticated') ||
		normalized.includes('invalid session') ||
		normalized.includes('invalid refresh') ||
		normalized.includes('401') ||
		normalized.includes('403')
	)
		return new AuthError(message, 'invalid_session')
	if (operation === 'sign_in') return new ProviderAuthError(message, 'provider_failure')
	if (operation === 'restore') return new AuthError(message, 'corrupt_session')
	return new NetworkError(message, 'amberite_unreachable')
}

export function authErrorFromPayload(payload: AuthFailurePayload): AmberiteApiError {
	if (
		payload.code === 'offline' ||
		payload.code === 'timeout' ||
		payload.code === 'provider_unreachable' ||
		payload.code === 'amberite_unreachable'
	) {
		return new NetworkError(payload.message, payload.code)
	}
	if (payload.recovery === 'return_to_provider') {
		return new ProviderAuthError(payload.message, payload.code)
	}
	return new AuthError(payload.message, payload.code, payload.recovery)
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
				// Convex and Tauri can prefix structured payloads with transport context.
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
