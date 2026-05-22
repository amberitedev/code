/**
 * Structured error hierarchy for @amberite/amberite-api.
 *
 * All errors extend AmberiteApiError. Callers can use `instanceof` to distinguish
 * network failures, auth failures, Core being offline, relay timeouts, and
 * typed errors returned by Core itself.
 */

export class AmberiteApiError extends Error {
	constructor(message: string) {
		super(message)
		this.name = 'AmberiteApiError'
	}
}

export class NetworkError extends AmberiteApiError {
	constructor(message: string) {
		super(`Network error: ${message}`)
		this.name = 'NetworkError'
	}
}

export class AuthError extends AmberiteApiError {
	constructor(message: string) {
		super(`Auth error: ${message}`)
		this.name = 'AuthError'
	}
}

export class CoreOfflineError extends AmberiteApiError {
	constructor() {
		super('Core is offline')
		this.name = 'CoreOfflineError'
	}
}

export class RelayTimeoutError extends AmberiteApiError {
	constructor() {
		super('Relay timed out waiting for acknowledgment')
		this.name = 'RelayTimeoutError'
	}
}

export class CoreApiError extends AmberiteApiError {
	status: number
	coreMessage: string

	constructor(status: number, coreMessage: string) {
		super(`Core API ${status}: ${coreMessage}`)
		this.name = 'CoreApiError'
		this.status = status
		this.coreMessage = coreMessage
	}
}
