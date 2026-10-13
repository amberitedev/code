import { ModrinthApiError } from '../../core/errors'
import { isModrinthErrorResponse } from '../../types/errors'

/**
 * A response the sharing backend refused. `status` and `code` say why, so a caller can tell a
 * shared instance that is gone (404) from one this account may not change (401, 403) without
 * reading the message.
 */
export class SharedInstancesApiError extends ModrinthApiError {
	readonly status: number
	/** The backend's exact cause (`code`) when it sends one, otherwise its broad kind (`error`). */
	readonly code: string | null

	constructor(error: ModrinthApiError & { statusCode: number }) {
		super(error.message, {
			statusCode: error.statusCode,
			originalError: error.originalError,
			responseData: error.responseData,
			context: error.context,
		})
		this.name = 'SharedInstancesApiError'
		this.status = error.statusCode
		const body = error.responseData
		if (!isModrinthErrorResponse(body)) this.code = null
		else this.code = 'code' in body && typeof body.code === 'string' ? body.code : body.error
	}

	/** Errors without a response, such as a failed connection, are returned unchanged. */
	static from(error: unknown): unknown {
		return error instanceof ModrinthApiError && hasStatus(error)
			? new SharedInstancesApiError(error)
			: error
	}
}

function hasStatus(error: ModrinthApiError): error is ModrinthApiError & { statusCode: number } {
	return error.statusCode !== undefined
}
