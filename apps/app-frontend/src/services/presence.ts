import { RealtimePresenceSession } from '@modrinth/api-client'
import { readonly, ref } from 'vue'

import { config } from '@/config'
import { amberite } from '@/services/amberite'

const users = ref<Record<string, boolean>>({})
let session: RealtimePresenceSession | null = null

export const onlineUsers = readonly(users)

export function startPresence() {
	if (session || !config.realtimeUrl) return
	session = new RealtimePresenceSession({
		endpoint: config.realtimeUrl,
		fetchFn: amberite.adapter.fetchFn,
		createWebSocket: (url) => new WebSocket(url),
		getJwt: () => amberite.adapter.getCurrentJwt(),
		origin: window.location.origin,
		onFrame(frame) {
			if (frame.type === 'presence.snapshot') users.value = frame.users
			if (frame.type === 'presence.user')
				users.value = { ...users.value, [frame.userId]: frame.online }
		},
		onInvalidated: stopPresence,
	})
	void session.connect()
}

export function stopPresence() {
	session?.dispose()
	session = null
	users.value = {}
}
