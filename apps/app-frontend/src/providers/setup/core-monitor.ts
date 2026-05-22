import { CoreApiClient, CoreConnectionMonitor, drainQueue } from '@amberite/amberite-api'

import { getDesktopAdapter } from '@/adapters/desktop'

/**
 * Start the Core connectivity monitor and wire the Mode 1a queue drain.
 *
 * Called once at app startup (from setupApp). If Core is not yet reachable or
 * paired, the monitor starts in 'connecting' state and transitions when Core
 * comes online. The monitor is intentionally never stopped — it runs for the
 * full app lifetime.
 */
export async function setupCoreMonitor(): Promise<void> {
	const adapter = getDesktopAdapter()
	const client = new CoreApiClient(adapter)

	let coreId = ''
	try {
		const status = await client.getSetupStatus()
		if (status.paired) coreId = status.core_id
	} catch {
		// Core not reachable at startup — monitor will still track direct health
		// and trigger the drain when Core eventually comes online.
	}

	const monitor = new CoreConnectionMonitor(adapter, coreId, '')
	monitor.onStateChange((state) => {
		if (state === 'online-direct') {
			drainQueue(adapter).catch((err) => {
				console.warn('[core-monitor] queue drain failed', err)
			})
		}
	})
	monitor.start()
}
