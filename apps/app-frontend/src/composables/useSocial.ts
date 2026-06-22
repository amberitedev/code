import {
	composeSocialSessionState,
	type LiveSocialState,
	type AmberiteUser,
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

import { clearConnectedCore, getConnectedCore, setConnectedCore } from '@/core/connected-core'
import {
	refreshRealtimeConvexAuth,
	useRealtimeConvexClient,
	useSocialClient,
	useSocialClientRaw,
} from '@/composables/useSocialClient'
import { config } from '@/config'

type Role = 'owner' | 'admin' | 'member'
type SessionWire = Omit<DurableSocialSessionState, 'currentUser' | 'group' | 'members' | 'bans' | 'pendingInvites' | 'core'> & {
	currentUser: AmberiteUser
	group: FriendGroupSummary | null
	members: FriendGroupMember[]
	bans: FriendGroupBan[]
	pendingInvites: GroupInviteWithGroup[]
	core: DurableSocialSessionState['core']
}

const sessionStateQuery = makeFunctionReference<'query', Record<string, never>, SessionWire>('social:sessionState')
const currentUser = ref<(AmberiteUser & { userId: string }) | null>(null)
const group = ref<FriendGroupSummary | null>(null)
const members = ref<FriendGroupMember[]>([])
const bans = ref<FriendGroupBan[]>([])
const invites = ref<GroupInviteWithGroup[]>([])
const friends = ref<FriendsListResult | null>(null)
const loading = ref(true)
const error = ref<Error | null>(null)
let subscribed = false
let durableState: SessionWire | null = null
let liveState: LiveSocialState = { users: {}, cores: {} }
let presenceSocket: WebSocket | null = null
let reconnectTimer: ReturnType<typeof setTimeout> | null = null
let reconnectAttempt = 0

function clearUserScopedState(clearUser = false) {
	if (clearUser) currentUser.value = null
	group.value = null
	members.value = []
	bans.value = []
	invites.value = []
	friends.value = null
	clearConnectedCore()
}

function safeSocialError(value: unknown): Error {
	if (value instanceof Error && value.message.toLowerCase().includes('not authenticated')) return new Error('Sign in to Amberite to use this feature.')
	return new Error('Amberite could not reach the account service. Check your connection and try again.')
}

function applyState(next: SessionWire) {
	durableState = next
	const state = composeSocialSessionState(next, liveState)
	currentUser.value = state.currentUser
	group.value = state.group
	members.value = state.members
	bans.value = state.bans
	invites.value = state.pendingInvites
	friends.value = state.friends
		? {
				...state.friends,
				friends: state.friends.friends.map((friend) => ({
					...friend,
					presence: { online: friend.user ? (state.live.users[friend.user.userId]?.online ?? false) : false },
				})),
			}
		: null
	const core = state.core
	if (core?.coreId && core.connectionUrl) {
		const connected = getConnectedCore()
		if (connected?.coreId !== core.coreId || connected.url !== core.connectionUrl) {
			setConnectedCore({ coreId: core.coreId, url: core.connectionUrl, groupId: state.group?.group.id })
		}
	} else {
		clearConnectedCore()
	}
	loading.value = false
	error.value = null
	void connectPresence()
}

async function connectPresence(): Promise<void> {
	if (!config.realtimeUrl || !currentUser.value || presenceSocket?.readyState === WebSocket.OPEN || presenceSocket?.readyState === WebSocket.CONNECTING) return
	try {
		const adapter = useSocialClientRawAdapter()
		const token = await adapter.getCurrentJwt()
		if (!token) return
		const response = await adapter.fetchFn(`${config.realtimeUrl.replace(/\/$/, '')}/v1/desktop-sessions`, {
			method: 'POST',
			headers: { Authorization: `Bearer ${token}`, Origin: 'tauri://localhost' },
		})
		if (!response.ok) throw new Error('realtime session was rejected')
		const session = await response.json() as { ticket: string }
		const url = new URL(`${config.realtimeUrl.replace(/\/$/, '')}/v1/connect`)
		url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
		url.searchParams.set('ticket', session.ticket)
		const socket = new WebSocket(url)
		presenceSocket = socket
		socket.onopen = () => { reconnectAttempt = 0 }
		socket.onmessage = (event) => applyLiveFrame(event.data)
		socket.onclose = () => schedulePresenceReconnect()
		socket.onerror = () => socket.close()
	} catch {
		schedulePresenceReconnect()
	}
}

function useSocialClientRawAdapter() {
	return useSocialClientRaw().adapter
}

function applyLiveFrame(raw: unknown): void {
	if (typeof raw !== 'string') return
	try {
		const frame = JSON.parse(raw) as { type?: string; userId?: string; coreId?: string; online?: boolean; health?: 'healthy' | 'degraded' | 'offline'; diagnostic?: 'none' | 'network' | 'authentication' | 'server'; users?: Record<string, boolean>; cores?: Record<string, { online: boolean }> }
		if (frame.type === 'presence.snapshot') {
			liveState = {
				users: Object.fromEntries(Object.entries(frame.users ?? {}).map(([id, online]) => [id, { online }])),
				cores: Object.fromEntries(Object.entries(frame.cores ?? {}).map(([id, value]) => [id, value])),
			}
		} else if (frame.type === 'presence.user' && frame.userId && typeof frame.online === 'boolean') {
			liveState = { ...liveState, users: { ...liveState.users, [frame.userId]: { online: frame.online } } }
		} else if (frame.type === 'presence.core' && frame.coreId && typeof frame.online === 'boolean') {
			liveState = { ...liveState, cores: { ...liveState.cores, [frame.coreId]: { online: frame.online, health: frame.health, diagnostic: frame.diagnostic } } }
		}
		if (durableState) applyState(durableState)
	} catch {
		return
	}
}

function schedulePresenceReconnect(): void {
	presenceSocket = null
	if (!config.realtimeUrl || !currentUser.value || reconnectTimer) return
	const delay = Math.min(30_000, 500 * 2 ** reconnectAttempt++)
	reconnectTimer = setTimeout(() => {
		reconnectTimer = null
		void connectPresence()
	}, delay)
}

export interface UseSocialReturn {
	currentUser: Ref<(AmberiteUser & { userId: string }) | null>
	group: Ref<FriendGroupSummary | null>
	members: Ref<FriendGroupMember[]>
	bans: Ref<FriendGroupBan[]>
	invites: Ref<GroupInviteWithGroup[]>
	friends: Ref<FriendsListResult | null>
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
	createGroup: (args: { coreId: string; name?: string; setupMode?: 'remote' | 'local'; connectionUrl?: string }) => Promise<void>
	updateGroup: (args: { name?: string; description?: string; banner?: string; subdomain?: string }) => Promise<void>
	inviteToGroup: (args: { inviteeUserId?: string; role?: Role; ttlMs?: number }) => Promise<{ inviteId: string; code?: string }>
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
		loading.value = true
		refreshRealtimeConvexAuth()
	}
	if (!subscribed) {
		subscribed = true
		useRealtimeConvexClient().onUpdate(sessionStateQuery, {}, applyState, (reason) => {
			loading.value = false
			error.value = safeSocialError(reason)
			if (reason.message.toLowerCase().includes('not authenticated')) clearUserScopedState(true)
		})
	}
	return {
		currentUser, group, members, bans, invites, friends, loading, error, myRole, canManage, refresh,
		refreshPresence: async () => undefined,
		searchUsers: (query) => client.searchUsers(query),
		sendFriendRequest: async (args) => { await mutation(() => client.sendFriendRequest(args)) },
		respondFriendRequest: async (requestId, accept) => { await mutation(() => client.respondFriendRequest(requestId, accept)) },
		cancelFriendRequest: async (requestId) => { await mutation(() => client.cancelFriendRequest(requestId)) },
		removeFriend: async (userId) => { await mutation(() => client.removeFriend(userId)) },
		blockUser: async (userId) => { await mutation(() => client.blockUser(userId)) },
		unblockUser: async (userId) => { await mutation(() => client.unblockUser(userId)) },
		createGroup: async (args) => { await mutation(() => client.ensureCoreFriendGroup(args)) },
		updateGroup: async (args) => { await mutation(() => client.updateFriendGroup({ friendGroupId: gid(), ...args })) },
		inviteToGroup: async (args) => (await mutation(() => client.createFriendGroupInvite({ friendGroupId: gid(), ...args }))) ?? { inviteId: '' },
		acceptInvite: async (args) => { await mutation(() => client.acceptFriendGroupInvite(args)) },
		declineInvite: async (inviteId) => { await mutation(() => client.declineFriendGroupInvite(inviteId)) },
		revokeInvite: async (inviteId) => { await mutation(() => client.revokeFriendGroupInvite(inviteId)) },
		setMemberRole: async (userId, role, permissionPreset) => { await mutation(() => client.updateMemberRole({ friendGroupId: gid(), userId, role, permissionPreset })) },
		kickMember: async (userId) => { await mutation(() => client.removeMember(gid(), userId)) },
		banMember: async (userId, reason) => { await mutation(() => client.banMember(gid(), userId, reason)) },
		unbanMember: async (userId) => { await mutation(() => client.unbanMember(gid(), userId)) },
		transferOwnership: async (userId) => { await mutation(() => client.transferOwnership(gid(), userId)) },
		leaveGroup: async () => { await mutation(() => client.leaveGroup(gid())) },
	}
}
