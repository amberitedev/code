/** Structured, recovery-aware errors for @amberite/amberite-api. */

export type RecoveryDisposition = 'preserve_and_retry' | 'clear_session' | 'return_to_provider'

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
	| 'cancelled'
	| 'state_failure'
	| 'xbox_restriction'
	| 'java_profile_missing'
	| 'throttled'
	| 'configuration_failure'
	| 'provider_failure'

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
	const normalized = message.toLowerCase()
	if (normalized.includes('expired')) return new AuthError(message, 'expired_session')
	if (normalized.includes('revoked')) return new AuthError(message, 'revoked_session')
	if (normalized.includes('refresh') && normalized.includes('reuse'))
		return new AuthError(message, 'refresh_reuse')
	if (
		(normalized.includes('uuid') && normalized.includes('mismatch')) ||
		normalized.includes('ambiguousminecraftidentity') ||
		normalized.includes('minecrafthandleconflict')
	) {
		return new AuthError(message, 'identity_mismatch')
	}
	if (normalized.includes('cancel')) return new ProviderAuthError(message, 'cancelled')
	if (normalized.includes('state') && normalized.includes('fail'))
		return new ProviderAuthError(message, 'state_failure')
	if (normalized.includes('java') && normalized.includes('profile'))
		return new ProviderAuthError(message, 'java_profile_missing')
	if (normalized.includes('xbox')) return new ProviderAuthError(message, 'xbox_restriction')
	if (normalized.includes('throttl') || status === 429)
		return new ProviderAuthError(message, 'throttled')
	if (
		normalized.includes('configur') ||
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
