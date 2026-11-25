export type LogLevel = 'error' | 'warn' | 'info' | 'debug' | 'trace'

export interface LogLine {
	text: string
	level: LogLevel | null
}

/** A structured log line, as the engine's `log` event carries it. */
export interface Log4jEvent {
	logger_name?: string | null
	level?: string | null
	thread_name?: string | null
	timestamp_millis?: number | null
	message?: string | null
	throwable?: string | null
}

export interface LogSource {
	id: string
	name: string
	live: boolean
}
