import {
	composeSocialSessionState,
	RealtimePresenceSession,
	type RealtimeFrame,
	type LiveSocialState,
	type AmberiteUser,
	type CoreListEntry,
	type CorePresence,
	type DurableSocialSessionState,
	type FriendGroupBan,
	type FriendGroupMember,
	type FriendGroupSummary,
	type FriendsListResult,
	type GroupInviteWithGroup,
} from '@amberite/amberite-api'
import { makeFunctionReference } from 'convex/server'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { clearConnectedCore, getConnectedCore, setConnectedCore } from '@/core/connected-core'
import {
	refreshRealtimeConvexAuth,
	useRealtimeConvexClient,
	useSocialClient,
	useSocialClientRaw,
} from '@/composables/useSocialClient'
import { config } from '@/config'

type Role = 'owner' | 'admin' | 'member'
const INITIAL_SOCIAL_STATE_TIMEOUT_MS = 15_000
type SessionWire = DurableSocialSessionState & {
	currentUser: AmberiteUser
	group?: FriendGroupSummary | null
	members?: FriendGroupMember[]
	bans?: FriendGroupBan[]
	pendingInvites?: GroupInviteWithGroup[]
	core?: CorePresence | null
}

const sessionStateQuery = makeFunctionReference<'query', Record<string, never>, SessionWire>(
	'social:sessionState',
)
const currentUser = ref<(AmberiteUser & { userId: string }) | null>(null)
const group = ref<FriendGroupSummary | null>(null)
const members = ref<FriendGroupMember[]>([])
const bans = ref<FriendGroupBan[]>([])
const invites = ref<GroupInviteWithGroup[]>([])
const friends = ref<FriendsListResult | null>(null)
const coreLinks = ref<CoreListEntry[]>([])
const loading = ref(true)
const error = ref<Error | null>(null)
let subscribed = false
let durableState: SessionWire | null = null
let liveState: LiveSocialState = { users: {} }
let presenceSession: RealtimePresenceSession | null = null
let initialStateReceived = false
let initialStateTimer: ReturnType<typeof setTimeout> | null = null
let stateWaiters: Array<() => void> = []

function clearUserScopedState(clearUser = false) {
	if (clearUser) {
		currentUser.value = null
		liveState = { users: {} }
		presenceSession?.dispose()
		presenceSession = null
	}
	group.value = null
	members.value = []
	bans.value = []
	invites.value = []
	friends.value = null
	coreLinks.value = []
	clearConnectedCore()
}

function safeSocialError(value: unknown): Error {
	if (value instanceof Error && value.message.toLowerCase().includes('not authenticated'))
		return new Error('Sign in to Amberite to use this feature.')
	return new Error(
		'Amberite could not reach the account service. Check your connection and try again.',
	)
}

function settleStateWaiters() {
	const waiters = stateWaiters
	stateWaiters = []
	for (const resolve of waiters) resolve()
}

function waitForSocialState(): Promise<void> {
	return new Promise((resolve) => {
		let timeout: ReturnType<typeof setTimeout> | null = null
		const done = () => {
			if (timeout) clearTimeout(timeout)
			resolve()
		}
		timeout = setTimeout(() => {
			stateWaiters = stateWaiters.filter((waiter) => waiter !== done)
			done()
		}, INITIAL_SOCIAL_STATE_TIMEOUT_MS)
		stateWaiters.push(done)
	})
}

function handleSubscriptionError(reason: unknown) {
	if (initialStateTimer) {
		clearTimeout(initialStateTimer)
		initialStateTimer = null
	}
	loading.value = false
	error.value = safeSocialError(reason)
	settleStateWaiters()
	if (reason instanceof Error && reason.message.toLowerCase().includes('not authenticated'))
		clearUserScopedState(true)
}

function applyState(next: SessionWire) {
	durableState = next
	const state = composeSocialSessionState(next, liveState)
	currentUser.value = state.currentUser
	group.value = next.group ?? null
	members.value = next.members ?? []
	bans.value = next.bans ?? []
	invites.value = next.pendingInvites ?? []
	coreLinks.value = state.coreLinks
	friends.value = state.friends
		? {
				...state.friends,
				friends: state.friends.friends.map((friend) => ({
					...friend,
					presence: {
						online: friend.user ? (state.live.users[friend.user.userId]?.online ?? false) : false,
					},
				})),
			}
		: null
	const core =
		next.core ??
		state.coreLinks.find((entry) => entry.linkState === 'linked' && entry.connectionUrl) ??
		null
	syncConnectedCoreLink(core, state.coreLinks, next.group?.group.id)
	loading.value = false
	error.value = null
	initialStateReceived = true
	if (initialStateTimer) {
		clearTimeout(initialStateTimer)
		initialStateTimer = null
	}
	settleStateWaiters()
	connectPresence()
}

function syncConnectedCoreLink(
	core: CorePresence | CoreListEntry | null,
	coreLinks: CoreListEntry[],
	groupId?: string,
) {
	const connected = getConnectedCore()
	if (core?.coreId && core.connectionUrl) {
		if (connected?.coreId !== core.coreId || connected.url !== core.connectionUrl) {
			setConnectedCore({
				coreId: core.coreId,
				url: core.connectionUrl,
				groupId,
			})
		}
		return
	}

	if (
		connected &&
		!coreLinks.some(
			(entry) =>
				entry.coreId === connected.coreId && entry.linkState === 'linked' && !!entry.connectionUrl,
		)
	) {
		clearConnectedCore()
		useCoreClient().clearCoreUrlCache()
	}
}

function connectPresence(): void {
	if (!config.realtimeUrl || !currentUser.value) return
	const adapter = useSocialClientRawAdapter()
	if (!presenceSession) {
		presenceSession = new RealtimePresenceSession({
			endpoint: config.realtimeUrl,
			fetchFn: adapter.fetchFn,
			createWebSocket: (url) => new WebSocket(url),
			getJwt: () => adapter.getCurrentJwt(),
			origin: 'tauri://localhost',
			onFrame: applyLiveFrame,
			onInvalidated: () => clearUserScopedState(true),
		})
	} else {
		presenceSession.setEndpoint(config.realtimeUrl)
	}
	void presenceSession.connect()
}

function useSocialClientRawAdapter() {
	return useSocialClientRaw().adapter
}

function applyLiveFrame(frame: RealtimeFrame): void {
	if (frame.type === 'presence.snapshot') {
		liveState = {
			users: Object.fromEntries(
				Object.entries(frame.users).map(([id, online]) => [id, { online }]),
			),
		}
	} else if (frame.type === 'presence.user') {
		liveState = {
			...liveState,
			users: { ...liveState.users, [frame.userId]: { online: frame.online } },
		}
	}
	if (durableState) applyState(durableState)
}

export interface UseSocialReturn {
	currentUser: Ref<(AmberiteUser & { userId: string }) | null>
	group: Ref<FriendGroupSummary | null>
	members: Ref<FriendGroupMember[]>
	bans: Ref<FriendGroupBan[]>
	invites: Ref<GroupInviteWithGroup[]>
	friends: Ref<FriendsListResult | null>
	coreLinks: Ref<CoreListEntry[]>
	loading: Ref<boolean>
	error: Ref<Error | null>
	myRole: ComputedRef<Role | null>
	canManage: ComputedRef<boolean>
	refresh: () => Promise<void>
	refreshPresence: () => Promise<void>
	searchUsers: (query: string) => Promise<AmberiteUser[]>
	sendFriendRequest: (args: { targetUserId: string; message?: string }) => Promise<void>
	respondFriendRequest: (requestId: string, accept: boolean) => Promise<void>
	cancelFriendRequest: (requestId: string) => Promise<void>
	removeFriend: (userId: string) => Promise<void>
	blockUser: (userId: string) => Promise<void>
	unblockUser: (userId: string) => Promise<void>
	createGroup: (args: {
		coreId: string
		name?: string
		setupMode?: 'remote' | 'local'
		connectionUrl?: string
	}) => Promise<{ friendGroupId: string } | undefined>
	updateGroup: (args: {
		name?: string
		description?: string
		banner?: string
		subdomain?: string
	}) => Promise<void>
	inviteToGroup: (args: {
		friendGroupId?: string
		inviteeUserId?: string
		role?: Role
		ttlMs?: number
	}) => Promise<{ inviteId: string; code?: string }>
	acceptInvite: (args: { inviteId?: string; code?: string }) => Promise<void>
	declineInvite: (inviteId: string) => Promise<void>
	revokeInvite: (inviteId: string) => Promise<void>
	setMemberRole: (userId: string, role: Role, permissionPreset?: string) => Promise<void>
	kickMember: (userId: string) => Promise<void>
	banMember: (userId: string, reason?: string) => Promise<void>
	unbanMember: (userId: string) => Promise<void>
	transferOwnership: (userId: string) => Promise<void>
	leaveGroup: () => Promise<void>
}

export function useSocial(): UseSocialReturn {
	const client = useSocialClient()
	const myRole = computed<Role | null>(() => group.value?.role ?? null)
	const canManage = computed(() => myRole.value === 'owner' || myRole.value === 'admin')
	const gid = () => {
		const id = group.value?.group.id
		if (!id) throw new Error('no active friend group')
		return id
	}
	async function mutation<T>(fn: () => Promise<T>): Promise<T | undefined> {
		loading.value = true
		error.value = null
		try {
			return await fn()
		} catch (reason) {
			error.value = safeSocialError(reason)
			return undefined
		} finally {
			loading.value = false
		}
	}
	async function refresh(): Promise<void> {
		error.value = null
		const stateReady = waitForSocialState()
		refreshRealtimeConvexAuth()
		await stateReady
	}
	if (!subscribed) {
		subscribed = true
		initialStateTimer = setTimeout(() => {
			if (!initialStateReceived)
				handleSubscriptionError(new Error('Timed out waiting for social state.'))
		}, INITIAL_SOCIAL_STATE_TIMEOUT_MS)
		useRealtimeConvexClient().onUpdate(
			sessionStateQuery,
			{},
			(next) => {
				try {
					applyState(next)
				} catch (reason) {
					handleSubscriptionError(reason)
				}
			},
			handleSubscriptionError,
		)
	}
	return {
		currentUser,
		group,
		members,
		bans,
		invites,
		friends,
		coreLinks,
		loading,
		error,
		myRole,
		canManage,
		refresh,
		refreshPresence: async () => undefined,
		searchUsers: (query) => client.searchUsers(query),
		sendFriendRequest: async (args) => {
			await mutation(() => client.sendFriendRequest(args))
		},
		respondFriendRequest: async (requestId, accept) => {
			await mutation(() => client.respondFriendRequest(requestId, accept))
		},
		cancelFriendRequest: async (requestId) => {
			await mutation(() => client.cancelFriendRequest(requestId))
		},
		removeFriend: async (userId) => {
			await mutation(() => client.removeFriend(userId))
		},
		blockUser: async (userId) => {
			await mutation(() => client.blockUser(userId))
		},
		unblockUser: async (userId) => {
			await mutation(() => client.unblockUser(userId))
		},
		createGroup: async (args) => {
			return await mutation(() => client.ensureCoreFriendGroup(args))
		},
		updateGroup: async (args) => {
			await mutation(() => client.updateFriendGroup({ friendGroupId: gid(), ...args }))
		},
		inviteToGroup: async ({ friendGroupId, ...args }) =>
			(await mutation(() =>
				client.createFriendGroupInvite({ friendGroupId: friendGroupId ?? gid(), ...args }),
			)) ?? {
				inviteId: '',
			},
		acceptInvite: async (args) => {
			await mutation(() => client.acceptFriendGroupInvite(args))
		},
		declineInvite: async (inviteId) => {
			await mutation(() => client.declineFriendGroupInvite(inviteId))
		},
		revokeInvite: async (inviteId) => {
			await mutation(() => client.revokeFriendGroupInvite(inviteId))
		},
		setMemberRole: async (userId, role, permissionPreset) => {
			await mutation(() =>
				client.updateMemberRole({ friendGroupId: gid(), userId, role, permissionPreset }),
			)
		},
		kickMember: async (userId) => {
			await mutation(() => client.removeMember(gid(), userId))
		},
		banMember: async (userId, reason) => {
			await mutation(() => client.banMember(gid(), userId, reason))
		},
		unbanMember: async (userId) => {
			await mutation(() => client.unbanMember(gid(), userId))
		},
		transferOwnership: async (userId) => {
			await mutation(() => client.transferOwnership(gid(), userId))
		},
		leaveGroup: async () => {
			await mutation(() => client.leaveGroup(gid()))
		},
	}
}
