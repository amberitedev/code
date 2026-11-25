import { createConsoleState } from '@modrinth/ui'

import { clear_log_buffer, get_live_log_buffer } from '@/helpers/logs'

type ConsoleState = ReturnType<typeof createConsoleState>

/** Each instance's live console, kept while the App runs so reopening the tab shows it at once. */
const consoles = new Map<string, ConsoleState>()

function liveConsoleOf(instanceId: string): ConsoleState {
	let live = consoles.get(instanceId)
	if (!live) consoles.set(instanceId, (live = createConsoleState()))
	return live
}

/** An instance's live console: what its running game printed, filled from the App's buffer. */
export function useInstanceConsole(instanceId: string) {
	const liveConsole = liveConsoleOf(instanceId)
	return {
		liveConsole,
		/** Fills an empty live console with the run's buffered output. */
		async hydrate() {
			if (liveConsole.output.value.length > 0) return
			const buffer = await get_live_log_buffer(instanceId)
			if (buffer) liveConsole.addLegacyLog(buffer)
		},
		async clearLive() {
			liveConsole.clear()
			await clear_log_buffer(instanceId).catch(() => {})
		},
		async destroy() {
			consoles.delete(instanceId)
			await clear_log_buffer(instanceId).catch(() => {})
		},
	}
}
