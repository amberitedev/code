<script setup lang="ts">
import {
	FilterIcon,
	LinkIcon,
	SearchIcon,
	ServerStackIcon,
	SettingsIcon,
	UserPlusIcon,
} from '@modrinth/assets'
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

import CoreOnboardingModal from '@/components/core/CoreOnboardingModal.vue'
import CoreSetupPanel from '@/components/core/CoreSetupPanel.vue'
import { mockInviteUsers, toAccessMember } from '@/components/core/core-onboarding-members'
import CoreHostingSettingsModal from '@/components/core/settings/CoreHostingSettingsModal.vue'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'

type RoleFilter = ServerAccessRole | 'all'

const social = useSocial()
const connection = useCoreConnection()
const onboardingModal = ref<InstanceType<typeof CoreOnboardingModal>>()
const settingsModal = ref<InstanceType<typeof CoreHostingSettingsModal>>()
const grantAccessModal = ref<InstanceType<typeof GrantAccessModal>>()
const search = ref('')
const roleFilter = ref<RoleFilter>('all')
const setupVisible = ref(false)
const hasGroup = computed(() => !!social.group.value)
const group = computed(() => social.group.value?.group)
const core = computed(() => social.group.value?.core)
const roleOptions: ServerAccessRoleOption[] = [
	{ value: 'owner', label: 'Owner', description: 'Controls the Core and group.' },
	{ value: 'editor', label: 'Admin', description: 'Manages members and servers.' },
	{ value: 'viewer', label: 'Member', description: 'Uses shared Core access.' },
]
const roleFilterOptions = [{ value: 'all', label: 'All roles' }, ...roleOptions]
const selectedRoleFilterLabel = computed(
	() => roleFilterOptions.find((option) => option.value === roleFilter.value)?.label ?? 'All roles',
)
const groupName = computed(() => group.value?.name || 'Amberite Core')
const memberRows = computed<ServerAccessMember[]>(() => social.members.value.map(toAccessMember))
const filteredMembers = computed(() => {
	const query = search.value.trim().toLowerCase()
	return memberRows.value.filter((member) => {
		if (roleFilter.value !== 'all' && member.role !== roleFilter.value) return false
		return !query || member.user.username.toLowerCase().includes(query)
	})
})
const statusClass = computed(() =>
	connection.status.value?.state === 'connected' ? 'bg-green' : 'bg-red',
)
const statusTooltip = computed(() =>
	connection.status.value?.state === 'connected' ? 'Core online' : 'Core offline',
)

async function searchUsers(query: string): Promise<ServerAccessInviteSuggestion[]> {
	const normalized = query.trim().toLowerCase()
	return mockInviteUsers.filter((user) => user.username.toLowerCase().includes(normalized))
}

async function grantAccess(payload: GrantServerAccessPayload) {
	await social.inviteToGroup({
		inviteeUserId: payload.user.id,
		role: payload.role === 'editor' ? 'admin' : 'member',
	})
	if (payload.addAsFriend) await social.sendFriendRequest({ username: payload.user.username })
}

async function updateRole(member: ServerAccessMember, role: ServerAccessRole) {
	if (member.isOwner) return
	await social.setMemberRole(member.user.id, role === 'editor' ? 'admin' : 'member')
}

async function removeMember(member: ServerAccessMember) {
	if (member.isOwner) return
	await social.kickMember(member.user.id)
}
</script>

<template>
	<div class="relative flex min-h-full flex-col p-6">
		<CoreOnboardingModal ref="onboardingModal" />
		<CoreHostingSettingsModal
			ref="settingsModal"
			:name="groupName"
			:connection-url="core?.connectionUrl"
		/>
		<CoreSetupPanel
			v-if="!hasGroup || setupVisible"
			@create="onboardingModal?.show('create')"
			@connect="onboardingModal?.show('connect')"
		/>
		<div v-else class="flex w-full flex-1 flex-col gap-4">
			<div class="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
				<div class="flex min-w-0 flex-col gap-1">
					<div class="flex min-w-0 items-center gap-3">
						<h1 class="m-0 truncate text-3xl font-semibold text-contrast">
							{{ groupName }}
						</h1>
						<span
							v-tooltip="statusTooltip"
							class="mt-1 size-2.5 shrink-0 rounded-full"
							:class="statusClass"
							aria-label="Core status"
						/>
					</div>
					<p class="m-0 max-w-3xl text-secondary">
						{{ group?.description || core?.connectionUrl || 'Friend group access and Core connection overview.' }}
					</p>
				</div>
				<ButtonStyled>
					<button @click="settingsModal?.show()">
						<SettingsIcon />
						Settings
					</button>
				</ButtonStyled>
			</div>
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
				:suggestions="mockInviteUsers"
				:search-users="searchUsers"
				@grant="grantAccess"
			/>
		</div>
		<Teleport to="body">
			<div
				v-if="hasGroup"
				class="fixed z-20"
				style="right: 1.25rem; bottom: 1.25rem"
			>
				<ButtonStyled circular>
					<button
						v-tooltip="setupVisible ? 'Show dashboard' : 'Show setup'"
						class="!h-10 !w-10"
						:aria-label="setupVisible ? 'Show dashboard' : 'Show setup'"
						@click="setupVisible = !setupVisible"
					>
						<component :is="setupVisible ? ServerStackIcon : LinkIcon" />
					</button>
				</ButtonStyled>
			</div>
		</Teleport>
	</div>
</template>
