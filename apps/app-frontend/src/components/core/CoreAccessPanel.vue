<script setup lang="ts">
import type {
	AmberiteAccessUiRole,
	CoreAccessMember,
	CoreInvitation,
	CoreRole,
} from '@amberite/amberite-api'
import {
	amberiteAccessRoleOptions,
	toAmberiteAccessUiMember,
} from '@amberite/amberite-api'
import { FilterIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import type {
	GrantServerAccessPayload,
	ServerAccessInviteSuggestion,
	ServerAccessMember,
	ServerAccessRole,
} from '@modrinth/ui'
import { AccessTable, ButtonStyled, Combobox, StyledInput } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import CoreInviteMemberModal from '@/components/core/CoreInviteMemberModal.vue'
import { useCoreActivityLog } from '@/components/core/use-core-activity-log'
import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { toInviteSuggestion } from './core-onboarding-members'

type RoleFilter = AmberiteAccessUiRole | 'all'
type CoreAccessTableMember = ServerAccessMember & { inviteId?: string; inviteStatus?: string }

const emit = defineEmits<{
	'manage-roles': []
	'manage-new-roles': []
}>()

const social = useSocial()
const core = useCoreClient()
const { recordUserAccessEvent } = useCoreActivityLog()
const grantAccessModal = ref<InstanceType<typeof CoreInviteMemberModal>>()
const coreRoles = ref<CoreRole[]>([])
const coreMembers = ref<CoreAccessMember[]>([])
const coreInvitations = ref<CoreInvitation[]>([])
const canManageUsers = ref(false)
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
const friendIds = computed(() => inviteSuggestions.value.map((user) => user.id))
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
	recordUserAccessEvent('invited', payload.user.id, payload.user.username, undefined, payload.role)
	await loadCoreState()
}

async function updateRole(member: CoreAccessTableMember, role: ServerAccessRole) {
	if (member.isOwner || role === 'owner') return
	if (member.pending && member.inviteId) {
		await core.revokeCoreInvitation(member.inviteId)
		await core.createCoreInvitation({
			invitee_user_id: member.user.id,
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
	recordUserAccessEvent(
		'permission_modified',
		member.user.id,
		member.user.username,
		member.user.avatarUrl,
		role,
	)
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
	recordUserAccessEvent('removed', member.user.id, member.user.username, member.user.avatarUrl)
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
	try {
		const [roles, access, invitations] = await Promise.all([
			core.getCoreRoles(),
			core.listCoreAccess(),
			core.listCoreInvitations().catch(() => []),
		])
		coreRoles.value = roles.roles
		coreMembers.value = access.members
		canManageUsers.value = access.viewer.can_manage_users
		coreInvitations.value = invitations
	} catch {
		coreRoles.value = []
		coreMembers.value = []
		canManageUsers.value = false
		coreInvitations.value = []
	}
}

async function searchInviteUsers(query: string) {
	const users = await social.searchUsers(query)
	return users.map(toInviteSuggestion).filter((user) => !unavailableInviteUserIds.value.has(user.id))
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
		return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : []
	} catch {
		return []
	}
}

onMounted(loadCoreState)
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
			:user-profile-link="(username) => `https://modrinth.com/user/${encodeURIComponent(username)}`"
			:can-manage-users="canManageUsers"
			@update-role="updateRole"
			@resend-invite="resendInvite"
			@cancel-invite="removeMember"
			@remove-member="removeMember"
		/>
		<section
			v-if="
				coreMembers.some((member) => member.needs_role_reassignment_at) ||
				coreInvitations.some(
					(invite) => !['sent', 'pending_review'].includes(invite.status),
				)
			"
			class="flex flex-col gap-2 rounded-xl bg-surface-2 p-4"
		>
			<span class="font-semibold text-contrast">Core access status</span>
			<div
				v-for="member in coreMembers.filter((item) => item.needs_role_reassignment_at)"
				:key="member.user_id"
				class="flex items-center justify-between gap-3 rounded-lg bg-surface-3 px-3 py-2"
			>
				<span class="text-primary">{{ member.display_name ?? member.user_id }}</span
				><span class="rounded-full bg-orange-highlight px-2 py-1 text-xs font-semibold text-orange"
					>Role removed - reassignment required</span
				>
			</div>
			<div
				v-for="invite in coreInvitations.filter(
					(item) => !['sent', 'pending_review'].includes(item.status),
				)"
				:key="invite.id"
				class="flex items-center justify-between gap-3 rounded-lg bg-surface-3 px-3 py-2 text-sm"
			>
				<span class="text-primary">{{ invite.invitee_display_name ?? invite.invitee_user_id }}</span
				><span class="text-secondary">{{ invite.status.replace('_', ' ') }}</span>
			</div>
		</section>

		<CoreInviteMemberModal
			ref="grantAccessModal"
			:members="memberRows"
			:suggestions="inviteSuggestions"
			:friend-ids="friendIds"
			:search-users="searchInviteUsers"
			:can-grant="canManageUsers"
			@grant="grantAccess"
		/>
	</div>
</template>
