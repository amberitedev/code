export class CoreError extends Error {
	constructor(message: string) {
		super(message)
		this.name = 'CoreError'
	}
}

export class CoreNetworkError extends CoreError {
	constructor(message: string) {
		super(`Core network error: ${message}`)
		this.name = 'CoreNetworkError'
	}
}

export class CoreOfflineError extends CoreError {
	constructor() {
		super('Core is offline')
		this.name = 'CoreOfflineError'
	}
}

export class CoreApiError extends CoreError {
	constructor(
		public readonly status: number,
		public readonly coreMessage: string,
	) {
		super(`Core API ${status}: ${coreMessage}`)
		this.name = 'CoreApiError'
	}
}
