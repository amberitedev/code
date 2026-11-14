import type { ErrorCode } from '@amberite/contracts'

import { ModrinthApiError } from '../core/errors'

/** A route we own answered with an `ApiError`. Switch on `code`; `description` is ready to show. */
export class ApiCallError extends ModrinthApiError {
	readonly status: number
	readonly code: ErrorCode
	readonly description: string

	constructor(status: number, code: ErrorCode, description: string) {
		super(description, { statusCode: status })
		this.name = 'ApiCallError'
		this.status = status
		this.code = code
		this.description = description
	}
}
