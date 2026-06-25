import { describe, expect, it } from 'vitest'

import { composeSocialSessionState } from '../social-session'

describe('composeSocialSessionState', () => {
	it('prunes live entities no longer authorized by durable state', () => {
		const result = composeSocialSessionState({
			currentUser: { userId: 'me' },
			friends: { friends: [{ friendshipId: 'friendship', user: { userId: 'friend' }, createdAt: 0 }], incoming: [], outgoing: [], blocks: [] },
			coreLinks: [{ coreId: 'core', ownerUserId: 'me', linkState: 'linked', createdAt: 0, lastSeenAt: 0, projectionRevision: 0, syncedAt: 0, isOwner: true, memberUserIds: ['core-member'] }],
		}, {
			users: { friend: { online: true }, 'core-member': { online: true }, removed: { online: true } },
		})

		expect(result.live).toEqual({ users: { friend: { online: true }, 'core-member': { online: true } } })
	})
})
