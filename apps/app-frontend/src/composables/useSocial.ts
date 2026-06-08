/**
 * useSocial — reactive façade over the Amberite social/Core Convex backend.
 *
 * Exposes a singleton reactive store (current user, the user's friend group,
 * its members, bans and pending invites) plus actions that mutate the backend
 * and refresh the affected slice. Every action captures errors into `error`
 * so failures never escape into Tauri's uncaught handler.
 *
 * Key members: state refs (currentUser:40, group:41, members:42, bans:43,
 * invites:44, friends:45), refresh:71, action wrappers from 100.
 */
import type {
	AmberiteUser,
	FriendGroupBan,
	FriendGroupMember,
	FriendGroupSummary,
	FriendsListResult,
	GroupInviteWithGroup,
} from '@amberite/amberite-api'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import { CORE_URL_STORAGE_KEY } from '@/adapters/desktop'
import { useSocialClient } from '@/composables/useSocialClient'

type Role = 'owner' | 'admin' | 'member'

const currentUser = ref<(AmberiteUser & { userId: string }) | null>(null)
const group = ref<FriendGroupSummary | null>(null)
const members = ref<FriendGroupMember[]>([])
const bans = ref<FriendGroupBan[]>([])
const invites = ref<GroupInviteWithGroup[]>([])
const friends = ref<FriendsListResult | null>(null)
const loading = ref(false)
const error = ref<Error | null>(null)
let initialized = false

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
	sendFriendRequest: (args: {
		friendCode?: string
		username?: string
		message?: string
	}) => Promise<void>
	respondFriendRequest: (requestId: string, accept: boolean) => Promise<void>
	removeFriend: (userId: string) => Promise<void>
	createGroup: (args: {
		coreId: string
		name?: string
		setupMode?: 'remote' | 'local'
		connectionUrl?: string
	}) => Promise<void>
	updateGroup: (args: {
		name?: string
		description?: string
		banner?: string
		subdomain?: string
	}) => Promise<void>
	inviteToGroup: (args: {
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

	async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
		loading.value = true
		error.value = null
		try {
			return await fn()
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			return undefined
		} finally {
			loading.value = false
		}
	}

	async function refresh(): Promise<void> {
		await run(async () => {
			currentUser.value = await client.currentUser()
			const [groups, friendsList, myInvites] = await Promise.all([
				client.listMyFriendGroups(),
				client.friendsList(),
				client.listMyGroupInvites(),
			])
			friends.value = friendsList
			invites.value = myInvites
			group.value = groups[0] ?? null
			try {
				const connectionUrl = group.value?.core?.connectionUrl
				if (connectionUrl) window.localStorage.setItem(CORE_URL_STORAGE_KEY, connectionUrl)
			} catch {
				// localStorage can be unavailable in tests.
			}
			if (group.value) {
				const gid = group.value.group.id
				const [memberList, banList] = await Promise.all([
					client.listFriendGroupMembers(gid),
					canManage.value ? client.listFriendGroupBans(gid).catch(() => []) : Promise.resolve([]),
				])
				members.value = memberList
				bans.value = banList
			} else {
				members.value = []
				bans.value = []
			}
		})
	}

	function gid(): string {
		const id = group.value?.group.id
		if (!id) throw new Error('no active friend group')
		return id
	}

	async function mutate<T>(fn: () => Promise<T>): Promise<T | undefined> {
		const result = await run(fn)
		await refresh()
		return result
	}

	if (!initialized) {
		initialized = true
		void refresh()
	}

	return {
		currentUser,
		group,
		members,
		bans,
		invites,
		friends,
		loading,
		error,
		myRole,
		canManage,
		refresh,
		sendFriendRequest: (args) => mutate(() => client.sendFriendRequest(args)).then(() => undefined),
		respondFriendRequest: (requestId, accept) =>
			mutate(() => client.respondFriendRequest(requestId, accept)).then(() => undefined),
		removeFriend: (userId) => mutate(() => client.removeFriend(userId)).then(() => undefined),
		createGroup: (args) => mutate(() => client.ensureCoreFriendGroup(args)).then(() => undefined),
		updateGroup: (args) =>
			mutate(() => client.updateFriendGroup({ friendGroupId: gid(), ...args })).then(
				() => undefined,
			),
		inviteToGroup: async (args) => {
			const r = await mutate(() =>
				client.createFriendGroupInvite({ friendGroupId: gid(), ...args }),
			)
			return r ?? { inviteId: '' }
		},
		acceptInvite: (args) =>
			mutate(() => client.acceptFriendGroupInvite(args)).then(() => undefined),
		declineInvite: (inviteId) =>
			mutate(() => client.declineFriendGroupInvite(inviteId)).then(() => undefined),
		revokeInvite: (inviteId) =>
			mutate(() => client.revokeFriendGroupInvite(inviteId)).then(() => undefined),
		setMemberRole: (userId, role, permissionPreset) =>
			mutate(() =>
				client.updateMemberRole({ friendGroupId: gid(), userId, role, permissionPreset }),
			).then(() => undefined),
		kickMember: (userId) => mutate(() => client.removeMember(gid(), userId)).then(() => undefined),
		banMember: (userId, reason) =>
			mutate(() => client.banMember(gid(), userId, reason)).then(() => undefined),
		unbanMember: (userId) => mutate(() => client.unbanMember(gid(), userId)).then(() => undefined),
		transferOwnership: (userId) =>
			mutate(() => client.transferOwnership(gid(), userId)).then(() => undefined),
		leaveGroup: () => mutate(() => client.leaveGroup(gid())).then(() => undefined),
	}
}
