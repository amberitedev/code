import type { ErrorCode } from '@amberite/contracts/protocol'

import { ModrinthApiError } from '../core/errors'
import { isModrinthErrorResponse } from '../types/errors'

/**
 * A call to a server we own failed. Switch on `code`; `description` is ready to show. `status` is
 * 0 when no HTTP answer explains it, such as a call that was cancelled or never answered.
 */
export class ApiCallError extends ModrinthApiError {
	readonly status: number
	readonly code: ErrorCode
	readonly description: string

	constructor(status: number, code: ErrorCode, description: string) {
		super(description, { statusCode: status || undefined })
		this.name = 'ApiCallError'
		this.status = status
		this.code = code
		this.description = description
	}

	/**
	 * An answer an upstream module's request was refused with, typed by its `ApiError` body.
	 * Errors without an answer, such as a failed connection, are returned unchanged.
	 */
	static from(error: unknown): unknown {
		if (!(error instanceof ModrinthApiError) || error.statusCode === undefined) return error
		if (error instanceof ApiCallError) return error
		const body = error.responseData
		return isModrinthErrorResponse(body)
			? // An unknown code is passed on as is; Rust reads it as `unknown`.
				new ApiCallError(error.statusCode, body.error as ErrorCode, body.description)
			: new ApiCallError(error.statusCode, 'unknown', error.message)
	}
}
