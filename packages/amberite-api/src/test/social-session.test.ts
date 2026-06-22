import { describe, expect, it } from 'vitest'

import { composeSocialSessionState } from '../social-session'

describe('composeSocialSessionState', () => {
	it('prunes live entities no longer authorized by durable state', () => {
		const result = composeSocialSessionState({
			currentUser: { userId: 'me' },
			friends: { friends: [{ friendshipId: 'friendship', user: { userId: 'friend' }, createdAt: 0 }], incoming: [], outgoing: [], blocks: [] },
			group: null,
			members: [],
			bans: [],
			pendingInvites: [],
			core: null,
		}, {
			users: { friend: { online: true }, removed: { online: true } },
			cores: { removedCore: { online: true } },
		})

		expect(result.live).toEqual({ users: { friend: { online: true } }, cores: {} })
	})
})
