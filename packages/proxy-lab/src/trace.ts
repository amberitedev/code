import { appendFile, mkdir } from 'node:fs/promises'
import { dirname } from 'node:path'

import type { IncomingHttpHeaders } from 'node:http'

import type { TraceEntry } from './types.ts'

const REDACTED_HEADERS = new Set(['authorization', 'cookie', 'set-cookie', 'x-api-key'])

export class TraceWriter {
	private pending = Promise.resolve()
	private readonly path: string

	constructor(path: string) {
		this.path = path
	}

	write(entry: TraceEntry): Promise<void> {
		this.pending = this.pending.then(async () => {
			await mkdir(dirname(this.path), { recursive: true })
			await appendFile(this.path, `${JSON.stringify(entry)}\n`, 'utf8')
		})
		return this.pending
	}

	close(): Promise<void> {
		return this.pending
	}
}

export function redactHeaders(headers: IncomingHttpHeaders): Record<string, string> {
	return Object.fromEntries(
		Object.entries(headers).map(([name, value]) => [
			name,
			REDACTED_HEADERS.has(name.toLowerCase()) ? '<redacted>' : formatHeader(value),
		]),
	)
}

function formatHeader(value: string | string[] | undefined): string {
	if (value === undefined) return ''
	return Array.isArray(value) ? value.join(', ') : value
}
