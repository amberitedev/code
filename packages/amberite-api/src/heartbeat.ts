import type { PlatformAdapter } from './adapter'
import { heartbeatCore } from './transport'

const HEARTBEAT_INTERVAL_MS = 30_000

export class CoreHeartbeat {
	private timer: ReturnType<typeof setInterval> | null = null

	constructor(
		private adapter: PlatformAdapter,
		private coreId: string,
	) {}

	start(): void {
		this.stop()
		this.timer = setInterval(() => this.beat(), HEARTBEAT_INTERVAL_MS)
		this.beat()
	}

	stop(): void {
		if (this.timer) {
			clearInterval(this.timer)
			this.timer = null
		}
	}

	private async beat(): Promise<void> {
		try {
			await heartbeatCore(this.adapter, this.coreId, 'online')
		} catch (error) {
			console.warn('Core heartbeat failed:', error)
		}
	}
}
