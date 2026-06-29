function getCallerLocation(): string {
	try {
		const stack = new Error().stack
		if (!stack) return ''

		const lines = stack.split('\n')
		const callerLine = lines[3]
		if (!callerLine) return ''

		const match = callerLine.match(/(https?:\/\/.+?|file:\/\/.+?|\/.*?):(\d+):\d+/)
		if (!match) return ''

		const [, fullPath, line] = match
		const fileName = fullPath.split('/').pop()?.split('?')[0] || fullPath
		return `${fileName}:${line}`
	} catch {
		return ''
	}
}

type DebugLogger = (...args: unknown[]) => void

const noopDebugLogger: DebugLogger = () => {}
const DEBUG_LOGGING_STORAGE_KEY = 'modrinth:debug-logging'

function isDebugLoggingEnabled() {
	if (import.meta.env?.VITE_DEBUG_LOGGING === 'true') {
		return true
	}

	try {
		return globalThis.localStorage?.getItem(DEBUG_LOGGING_STORAGE_KEY) === 'true'
	} catch {
		return false
	}
}

export function useDebugLogger(namespace: string): DebugLogger {
	if (!isDebugLoggingEnabled()) {
		return noopDebugLogger
	}

	// eslint-disable-next-line
	return (...args: any[]) => {
		const location = getCallerLocation()
		const prefix = location ? `[${namespace}] ${location}` : `[${namespace}]`
		console.debug(prefix, ...args)
	}
}
