/**
 * Heartbeat module — Core updates core_registrations.last_seen on a regular
 * interval while running. This timestamp is a UX hint only; it does not drive
 * routing decisions.
 */

import type { SupabaseClient } from '@supabase/supabase-js'
import { NetworkError } from './errors'

const HEARTBEAT_INTERVAL_MS = 30_000

export class CoreHeartbeat {
	private timer: ReturnType<typeof setInterval> | null = null

	constructor(
		private supabase: SupabaseClient,
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
		const { error } = await this.supabase
			.from('core_registrations')
			.update({ last_seen: new Date().toISOString() })
			.eq('id', this.coreId)

		if (error) {
			// Heartbeat failures are non-fatal; just log.
			console.warn('Core heartbeat failed:', error.message)
		}
	}
}
