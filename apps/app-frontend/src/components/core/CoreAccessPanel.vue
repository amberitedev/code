<script setup lang="ts">
import { FilterIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import {
	AccessTable,
	ButtonStyled,
	Combobox,
	GrantAccessModal,
	StyledInput,
} from '@modrinth/ui'
import type {
	GrantServerAccessPayload,
	ServerAccessInviteSuggestion,
	ServerAccessMember,
	ServerAccessRole,
	ServerAccessRoleOption,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { toAccessMember } from '@/components/core/core-onboarding-members'
import { useCoreActivityLog } from '@/components/core/use-core-activity-log'
import { useSocial } from '@/composables/useSocial'

type RoleFilter = ServerAccessRole | 'all'

const emit = defineEmits<{
	'manage-roles': []
}>()

const social = useSocial()
const { recordUserAccessEvent } = useCoreActivityLog()
const grantAccessModal = ref<InstanceType<typeof GrantAccessModal>>()
const search = ref('')
const roleFilter = ref<RoleFilter>('all')
const roleOptions: ServerAccessRoleOption[] = [
	{ value: 'owner', label: 'Owner', description: 'Controls the Core and group.' },
	{ value: 'editor', label: 'Admin', description: 'Manages members and servers.' },
	{ value: 'viewer', label: 'Member', description: 'Uses shared Core access.' },
]
const roleFilterOptions = [{ value: 'all', label: 'All roles' }, ...roleOptions]
const selectedRoleFilterLabel = computed(
	() => roleFilterOptions.find((option) => option.value === roleFilter.value)?.label ?? 'All roles',
)
const memberRows = computed<ServerAccessMember[]>(() => social.members.value.map(toAccessMember))
const filteredMembers = computed(() => {
	const query = search.value.trim().toLowerCase()
	return memberRows.value.filter((member) => {
		if (roleFilter.value !== 'all' && member.role !== roleFilter.value) return false
		return !query || member.user.username.toLowerCase().includes(query)
	})
})

async function searchUsers(query: string): Promise<ServerAccessInviteSuggestion[]> {
	const users = await social.searchUsers(query)
	return users.map((user) => ({
		id: user.userId,
		username: user.username || user.displayName || user.userId,
		avatarUrl: user.image,
	}))
}

async function grantAccess(payload: GrantServerAccessPayload) {
	await social.inviteToGroup({
		inviteeUserId: payload.user.id,
		role: payload.role === 'editor' ? 'admin' : 'member',
	})
	if (payload.addAsFriend) await social.sendFriendRequest({ targetUserId: payload.user.id })
	recordUserAccessEvent('invited', payload.user.id, payload.user.username, payload.user.avatarUrl, payload.role)
}

async function updateRole(member: ServerAccessMember, role: ServerAccessRole) {
	if (member.isOwner) return
	await social.setMemberRole(member.user.id, role === 'editor' ? 'admin' : 'member')
	recordUserAccessEvent('permission_modified', member.user.id, member.user.username, member.user.avatarUrl, role)
}

async function removeMember(member: ServerAccessMember) {
	if (member.isOwner) return
	await social.kickMember(member.user.id)
	recordUserAccessEvent('removed', member.user.id, member.user.username, member.user.avatarUrl)
}

function showPermissionsModal() {
	grantAccessModal.value?.hide()
	emit('manage-roles')
}
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
					<button class="!h-10 w-full md:w-fit" @click="grantAccessModal?.show()">
						<UserPlusIcon />
						Invite friends
					</button>
				</ButtonStyled>
			</div>
		</div>

		<AccessTable
			:members="filteredMembers"
			:roles="roleOptions"
			:user-link="(username) => `https://modrinth.com/user/${encodeURIComponent(username)}`"
			:can-manage-users="social.canManage.value"
			@update-role="updateRole"
			@remove-member="removeMember"
		/>

		<GrantAccessModal
			ref="grantAccessModal"
			:members="memberRows"
			:search-users="searchUsers"
			open-permissions-in-modal
			permissions-href="#"
			permissions-link-target="_self"
			@grant="grantAccess"
			@view-permissions="showPermissionsModal"
		/>
	</div>
</template>
