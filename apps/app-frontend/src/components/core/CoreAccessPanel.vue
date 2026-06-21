<script setup lang="ts">
import type { CoreAccessMember, CoreInvitation, CoreRole } from '@amberite/amberite-api'
import { FilterIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import type {
	ServerAccessMember,
	ServerAccessRole,
	ServerAccessRoleOption,
} from '@modrinth/ui'
import {
	AccessTable,
	ButtonStyled,
	Combobox,
	StyledInput,
} from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { toAccessMember } from '@/components/core/core-onboarding-members'
import CoreInviteMemberModal from '@/components/core/CoreInviteMemberModal.vue'
import { useCoreActivityLog } from '@/components/core/use-core-activity-log'
import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'

type RoleFilter = ServerAccessRole | 'all'

const emit = defineEmits<{
	'manage-roles': []
	'manage-new-roles': []
}>()

const social = useSocial()
const socialClient = useSocialClient()
const core = useCoreClient()
const { recordUserAccessEvent } = useCoreActivityLog()
const grantAccessModal = ref<InstanceType<typeof CoreInviteMemberModal>>()
const coreRoles = ref<CoreRole[]>([])
const coreMembers = ref<CoreAccessMember[]>([])
const coreInvitations = ref<CoreInvitation[]>([])
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

async function grantAccess(user: { id: string; username: string }, roleId: string) {
	const invite = await core.createCoreInvitation({
		invitee_user_id: user.id,
		invitee_display_name: user.username,
		role_id: roleId,
	})
	if (invite.status === 'sent') {
		const [metadata, coreUrl] = await Promise.all([core.getCoreMetadata(), core.adapter.getCoreUrl()])
		if (coreUrl) await socialClient.notifyCoreInvite({ recipientUserId: user.id, coreId: metadata.core_id, coreUrl, inviteId: invite.id, expiresAt: Date.parse(invite.expires_at) })
	}
	recordUserAccessEvent('invited', user.id, user.username, undefined, 'viewer')
	await loadCoreState()
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

async function loadCoreState() {
	try {
		const [roles, access, invitations] = await Promise.all([
			core.getCoreRoles(),
			core.listCoreAccess(),
			core.listCoreInvitations().catch(() => []),
		])
		coreRoles.value = roles.roles
		coreMembers.value = access.members
		coreInvitations.value = invitations
	} catch {
		coreRoles.value = []
		coreMembers.value = []
		coreInvitations.value = []
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
		<section v-if="coreMembers.some((member) => member.needs_role_reassignment_at) || coreInvitations.length" class="flex flex-col gap-2 rounded-xl bg-surface-2 p-4">
			<span class="font-semibold text-contrast">Core access status</span>
			<div v-for="member in coreMembers.filter((item) => item.needs_role_reassignment_at)" :key="member.user_id" class="flex items-center justify-between gap-3 rounded-lg bg-surface-3 px-3 py-2"><span class="text-primary">{{ member.display_name ?? member.user_id }}</span><span class="rounded-full bg-orange-highlight px-2 py-1 text-xs font-semibold text-orange">Role removed — reassignment required</span></div>
			<div v-for="invite in coreInvitations" :key="invite.id" class="flex items-center justify-between gap-3 rounded-lg bg-surface-3 px-3 py-2 text-sm"><span class="text-primary">{{ invite.invitee_display_name ?? invite.invitee_user_id }}</span><span class="text-secondary">{{ invite.status.replace('_', ' ') }}</span></div>
		</section>

		<CoreInviteMemberModal
			ref="grantAccessModal"
			:roles="coreRoles"
			@invite="grantAccess"
			@open-role="emit('manage-new-roles')"
		/>
	</div>
</template>
