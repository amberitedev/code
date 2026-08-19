<script setup lang="ts">
import type { AmberiteAccessUiRole } from '@amberite/amberite-api'
import { amberiteAccessRoleOptions, toAmberiteAccessUiMember } from '@amberite/amberite-api'
import type { CoreAccessMember, CoreInvitation, CoreRole } from '@modrinth/api-client'
import { FilterIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import type {
	GrantServerAccessPayload,
	ServerAccessInviteSuggestion,
	ServerAccessMember,
	ServerAccessRole,
} from '@modrinth/ui'
import {
	AccessTable,
	ButtonStyled,
	Combobox,
	injectNotificationManager,
	StyledInput,
} from '@modrinth/ui'
import { computed, onMounted, ref, watch } from 'vue'

import { coreAccessCache } from '@/components/core/core-panel-cache'
import CoreInviteMemberModal from '@/components/core/CoreInviteMemberModal.vue'
import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { useConnectedCore } from '@/core/connected-core'

import { toInviteSuggestion } from './core-onboarding-members'

type RoleFilter = AmberiteAccessUiRole | 'all'
type CoreAccessTableMember = ServerAccessMember & { inviteId?: string; inviteStatus?: string }

const emit = defineEmits<{
	'manage-roles': []
	'manage-new-roles': []
	'open-permissions-settings': []
}>()

const social = useSocial()
const core = useCoreClient()
const connectedCore = useConnectedCore()
const { addNotification } = injectNotificationManager()
const grantAccessModal = ref<InstanceType<typeof CoreInviteMemberModal>>()
const coreAccessCacheKey = computed(() => connectedCore.value?.coreId ?? 'core')
const cachedCoreAccess = coreAccessCache.get(coreAccessCacheKey.value)
const coreRoles = ref<CoreRole[]>(cachedCoreAccess?.roles ?? [])
const coreMembers = ref<CoreAccessMember[]>(cachedCoreAccess?.members ?? [])
const coreInvitations = ref<CoreInvitation[]>(cachedCoreAccess?.invitations ?? [])
const canManageUsers = ref(cachedCoreAccess?.canManageUsers ?? false)
const search = ref('')
const roleFilter = ref<RoleFilter>('all')
const roleOptions = amberiteAccessRoleOptions
const roleFilterOptions = [{ value: 'all', label: 'All roles' }, ...roleOptions]
const selectedRoleFilterLabel = computed(
	() => roleFilterOptions.find((option) => option.value === roleFilter.value)?.label ?? 'All roles',
)
const activeInvitations = computed(() =>
	coreInvitations.value.filter(
		(invite) => invite.status === 'sent' || invite.status === 'pending_review',
	),
)
const memberRows = computed<CoreAccessTableMember[]>(() => {
	const rows: CoreAccessTableMember[] = coreMembers.value.map(toAmberiteAccessUiMember)
	const memberIds = new Set(coreMembers.value.map((member) => member.user_id))
	for (const invite of activeInvitations.value) {
		if (memberIds.has(invite.invitee_user_id)) continue
		rows.push({
			id: `invite-${invite.id}`,
			user: {
				id: invite.invitee_user_id,
				username: invite.invitee_display_name ?? invite.invitee_user_id,
			},
			role: roleIdToAccessRole(invite.role_id),
			joinedAt: null,
			pending: true,
			inviteId: invite.id,
			inviteStatus: invite.status,
		})
	}
	return rows
})
const filteredMembers = computed(() => {
	const query = search.value.trim().toLowerCase()
	return memberRows.value.filter((member) => {
		if (roleFilter.value !== 'all' && member.role !== roleFilter.value) return false
		return !query || member.user.username.toLowerCase().includes(query)
	})
})
const inviteSuggestions = computed<ServerAccessInviteSuggestion[]>(() =>
	(social.friends.value?.friends ?? [])
		.map((friend) => friend.user)
		.filter((user): user is NonNullable<typeof user> => !!user)
		.map(toInviteSuggestion)
		.filter((user) => !unavailableInviteUserIds.value.has(user.id)),
)
const friendIds = computed(() =>
	(social.friends.value?.friends ?? [])
		.map((friend) => friend.user?.userId)
		.filter((id): id is string => !!id),
)
const pendingFriendRequestIds = computed(() => [
	...(social.friends.value?.incoming ?? []).map((request) => request.request.fromUserId),
	...(social.friends.value?.outgoing ?? []).map((request) => request.request.toUserId),
])
const unavailableInviteUserIds = computed(
	() =>
		new Set(
			[
				social.currentUser.value?.userId,
				...coreMembers.value.map((member) => member.user_id),
				...activeInvitations.value.map((invite) => invite.invitee_user_id),
			].filter((id): id is string => !!id),
		),
)

async function grantAccess(payload: GrantServerAccessPayload) {
	const roleId = accessRoleToRoleId(payload.role)
	await core.createCoreInvitation({
		invitee_user_id: payload.user.id,
		invitee_display_name: payload.user.username,
		role_id: roleId,
	})
	if (payload.addAsFriend) {
		await social.sendFriendRequest({ targetUserId: payload.user.id })
		if (social.error.value) throw social.error.value
	}
	await loadCoreState()
}

async function updateRole(member: CoreAccessTableMember, role: ServerAccessRole) {
	if (member.isOwner || role === 'owner') return
	if (member.pending && member.inviteId) {
		await core.updateCoreInvitation(member.inviteId, {
			invitee_display_name: member.user.username,
			role_id: accessRoleToRoleId(role),
		})
		await loadCoreState()
		return
	}
	await core.updateCoreAccess(member.user.id, {
		role: role === 'editor' ? 'admin' : 'member',
		permission_preset: role === 'editor' ? 'admin' : 'member',
	})
	await loadCoreState()
}

async function removeMember(member: CoreAccessTableMember) {
	if (member.isOwner) return
	if (member.pending && member.inviteId) {
		await core.revokeCoreInvitation(member.inviteId)
		await loadCoreState()
		return
	}
	await core.removeCoreAccess(member.user.id)
	await loadCoreState()
}

async function resendInvite(member: CoreAccessTableMember) {
	if (!member.pending || !member.inviteId) return
	await core.createCoreInvitation({
		invitee_user_id: member.user.id,
		invitee_display_name: member.user.username,
		role_id: accessRoleToRoleId(member.role),
	})
	await loadCoreState()
}

async function loadCoreState() {
	const requestedCacheKey = coreAccessCacheKey.value
	const [rolesResult, accessResult, invitationsResult] = await Promise.allSettled([
		core.getCoreRoles(),
		core.listCoreAccess(),
		core.listCoreInvitations(),
	])

	if (requestedCacheKey !== coreAccessCacheKey.value) return
	let refreshed = false

	if (accessResult.status === 'fulfilled') {
		const access = accessResult.value
		coreMembers.value = access.members
		canManageUsers.value = access.viewer.can_manage_users
		refreshed = true
	} else {
		addNotification({
			type: 'error',
			title: 'Failed to load Core members',
			text: errorMessage(accessResult.reason),
		})
	}

	if (rolesResult.status === 'fulfilled') {
		const roles = rolesResult.value
		coreRoles.value = roles.roles
		refreshed = true
	}

	if (invitationsResult.status === 'fulfilled') {
		coreInvitations.value = invitationsResult.value
		refreshed = true
	}

	if (refreshed) cacheCoreAccessState()
}

async function searchInviteUsers(query: string) {
	const users = await social.searchUsers(query)
	return users
		.map(toInviteSuggestion)
		.filter((user) => !unavailableInviteUserIds.value.has(user.id))
}

function openPermissionsSettings(event: MouseEvent) {
	event.preventDefault()
	emit('open-permissions-settings')
}

function accessRoleToRoleId(role: ServerAccessRole) {
	return role === 'editor' ? 'role-admin' : 'role-member'
}

function roleIdToAccessRole(roleId: string): AmberiteAccessUiRole {
	if (roleId === 'role-admin') return 'editor'
	const role = coreRoles.value.find((item) => item.id === roleId)
	if (!role) return 'viewer'
	const grants = parseRoleGrants(role)
	return grants.some((grant) =>
		[
			'invite-members',
			'remove-members',
			'ban-members',
			'manage-roles',
			'edit-member-roles',
			'approve-invites',
			'manage-instances',
			'edit-settings',
		].includes(grant),
	)
		? 'editor'
		: 'viewer'
}

function parseRoleGrants(role: CoreRole) {
	try {
		const value = JSON.parse(role.grants_json)
		return Array.isArray(value)
			? value.filter((item): item is string => typeof item === 'string')
			: []
	} catch {
		return []
	}
}

function errorMessage(reason: unknown): string {
	return reason instanceof Error ? reason.message : String(reason)
}

function applyCachedCoreAccessState() {
	const cached = coreAccessCache.get(coreAccessCacheKey.value)
	coreRoles.value = cached?.roles ?? []
	coreMembers.value = cached?.members ?? []
	coreInvitations.value = cached?.invitations ?? []
	canManageUsers.value = cached?.canManageUsers ?? false
}

function cacheCoreAccessState() {
	const next = {
		roles: coreRoles.value,
		members: coreMembers.value,
		invitations: coreInvitations.value,
		canManageUsers: canManageUsers.value,
	}
	const current = coreAccessCache.get(coreAccessCacheKey.value)
	if (!current || !sameSerialized(current, next)) {
		coreAccessCache.set(coreAccessCacheKey.value, next)
	}
}

function sameSerialized(left: unknown, right: unknown) {
	return JSON.stringify(left) === JSON.stringify(right)
}

watch(coreAccessCacheKey, () => {
	applyCachedCoreAccessState()
	void loadCoreState()
})

onMounted(() => void loadCoreState())
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="flex flex-col gap-2 md:flex-row">
			<StyledInput
				v-model="search"
				:icon="SearchIcon"
				:placeholder="`Search ${memberRows.length} members...`"
				wrapper-class="min-w-0 flex-1"
				input-class="!h-10"
				clearable
			/>
			<div class="flex shrink-0 flex-wrap items-center gap-2 md:flex-nowrap">
				<Combobox
					v-model="roleFilter"
					:options="roleFilterOptions"
					:display-value="selectedRoleFilterLabel"
					trigger-class="min-w-[225px] !h-10 !min-h-10 !py-0"
				>
					<template #prefix><FilterIcon class="size-5 text-secondary" /></template>
				</Combobox>
				<ButtonStyled color="brand">
					<button
						class="!h-10 w-full md:w-fit"
						:disabled="!canManageUsers"
						@click="grantAccessModal?.show()"
					>
						<UserPlusIcon />
						Invite friends
					</button>
				</ButtonStyled>
			</div>
		</div>

		<AccessTable
			:members="filteredMembers"
			:roles="roleOptions"
			:can-manage-users="canManageUsers"
			@update-role="updateRole"
			@resend-invite="resendInvite"
			@cancel-invite="removeMember"
			@remove-member="removeMember"
		/>

		<CoreInviteMemberModal
			ref="grantAccessModal"
			:members="memberRows"
			:suggestions="inviteSuggestions"
			:friend-ids="friendIds"
			:friend-request-unavailable-ids="pendingFriendRequestIds"
			:search-users="searchInviteUsers"
			:can-grant="canManageUsers"
			target-label="Username"
			target-placeholder="Search Amberite or Minecraft username"
			target-help="Use their unique Amberite username or their Minecraft username."
			permissions-help="See or edit the permissions <link>here</link>."
			permissions-help-href="#core-permissions"
			permissions-help-target="_self"
			@grant="grantAccess"
			@permissions-help-click="openPermissionsSettings"
		/>
	</div>
</template>
