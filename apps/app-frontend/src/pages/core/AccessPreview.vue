<script setup lang="ts">
import { FilterIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import {
	AccessTable,
	ButtonStyled,
	Combobox,
	GrantAccessModal,
	RemoveAccessModal,
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

type RoleFilter = ServerAccessRole | 'all'

const roleOptions: ServerAccessRoleOption[] = [
	{ value: 'owner', label: 'Owner', description: 'Controls the Core and friend group.' },
	{ value: 'editor', label: 'Admin', description: 'Can manage members and Core servers.' },
	{ value: 'viewer', label: 'Member', description: 'Can use shared Core access.' },
]
const roleFilterOptions = [
	{ value: 'all', label: 'All roles' },
	...roleOptions.map((role) => ({ value: role.value, label: role.label })),
]
const suggestions: ServerAccessInviteSuggestion[] = [
	{ id: 'nora', username: 'Nora' },
	{ id: 'kai', username: 'Kai' },
	{ id: 'mika', username: 'Mika' },
	{ id: 'sol', username: 'Sol' },
]
const members = ref<ServerAccessMember[]>([
	{
		id: 'amber',
		user: { id: 'amber', username: 'Amber (you)' },
		role: 'owner',
		joinedAt: new Date(Date.now() - 6 * 24 * 60 * 60 * 1000).toISOString(),
		isOwner: true,
	},
	{
		id: 'geode',
		user: { id: 'geode', username: 'Geode' },
		role: 'editor',
		joinedAt: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
	},
	{
		id: 'river',
		user: { id: 'river', username: 'River' },
		role: 'viewer',
		joinedAt: new Date(Date.now() - 18 * 60 * 60 * 1000).toISOString(),
	},
])
const search = ref('')
const roleFilter = ref<RoleFilter>('all')
const grantAccessModal = ref<InstanceType<typeof GrantAccessModal>>()
const removeAccessModal = ref<InstanceType<typeof RemoveAccessModal>>()
const pendingRemoval = ref<ServerAccessMember>()
const shouldCancelInvite = ref(false)
const selectedRoleFilterLabel = computed(
	() => roleFilterOptions.find((option) => option.value === roleFilter.value)?.label ?? 'All roles',
)
const filteredMembers = computed(() => {
	const normalized = search.value.trim().toLowerCase()
	return members.value.filter((member) => {
		if (roleFilter.value !== 'all' && member.role !== roleFilter.value) return false
		return !normalized || member.user.username.toLowerCase().includes(normalized)
	})
})

async function searchUsers(query: string) {
	const normalized = query.trim().toLowerCase()
	return suggestions.filter((user) => user.username.toLowerCase().includes(normalized))
}

function grantAccess(payload: GrantServerAccessPayload) {
	members.value.push({
		id: payload.user.id,
		user: { id: payload.user.id, username: payload.user.username, avatarUrl: payload.user.avatarUrl },
		role: payload.role,
		joinedAt: null,
		pending: true,
	})
}

function updateRole(member: ServerAccessMember, role: ServerAccessRole) {
	if (member.isOwner || role === 'owner') return
	members.value = members.value.map((row) => (row.id === member.id ? { ...row, role } : row))
}

function requestRemove(member: ServerAccessMember) {
	pendingRemoval.value = member
	shouldCancelInvite.value = false
	removeAccessModal.value?.show()
}

function requestCancel(member: ServerAccessMember) {
	pendingRemoval.value = member
	shouldCancelInvite.value = true
	removeAccessModal.value?.show()
}

function confirmRemove() {
	if (!pendingRemoval.value) return
	members.value = members.value.filter((row) => row.id !== pendingRemoval.value?.id)
	pendingRemoval.value = undefined
}
</script>

<template>
	<div class="flex min-h-full flex-col gap-4 p-6">
		<div class="flex flex-col gap-1">
			<h1 class="m-0 text-2xl font-extrabold text-contrast">Friend group access preview</h1>
			<p class="m-0 text-secondary">Mocked hosting access UI for Core friend-group management.</p>
		</div>
		<div class="flex flex-col gap-2 md:flex-row">
			<StyledInput
				v-model="search"
				:icon="SearchIcon"
				:placeholder="`Search ${members.length} members...`"
				wrapper-class="min-w-0 flex-1"
				input-class="!h-10"
				clearable
			/>
			<div class="flex shrink-0 items-center gap-2 flex-wrap md:flex-nowrap">
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
			@update-role="updateRole"
			@cancel-invite="requestCancel"
			@remove-member="requestRemove"
		/>
		<GrantAccessModal
			ref="grantAccessModal"
			:members="members"
			:suggestions="suggestions"
			:search-users="searchUsers"
			@grant="grantAccess"
		/>
		<RemoveAccessModal
			ref="removeAccessModal"
			:username="pendingRemoval?.user.username ?? ''"
			:avatar-url="pendingRemoval?.user.avatarUrl"
			:role="pendingRemoval?.role"
			:joined-at="pendingRemoval?.joinedAt"
			:pending="pendingRemoval?.pending"
			:should-cancel="shouldCancelInvite"
			@remove="confirmRemove"
		/>
	</div>
</template>
