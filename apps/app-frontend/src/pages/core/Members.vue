<script setup lang="ts">
import { BanIcon, MoreVerticalIcon, SearchIcon, UserPlusIcon, UsersIcon } from '@modrinth/assets'
import { Avatar, Badge, ButtonStyled, NewModal, OverflowMenu, StyledInput } from '@modrinth/ui'
import { computed, ref, useTemplateRef } from 'vue'

import { useCorePreview } from '@/components/core/use-core-preview'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'
import {
	SYNCED_PERMISSION_PRESET_LABELS,
	type SyncedPermissionPreset,
} from '@/pages/instance/synced/use-synced-permissions'

defineOptions({ name: 'CoreMembersPage' })

type Role = 'owner' | 'admin' | 'member'
type Member = (typeof members)['value'][number]

const presets: SyncedPermissionPreset[] = ['admin', 'member', 'client-only', 'viewer']
const {
	group,
	members,
	bans,
	currentUser,
	canManage,
	myRole,
	error,
	setMemberRole,
	kickMember,
	banMember,
	unbanMember,
	transferOwnership,
	leaveGroup,
	inviteToGroup,
} = useSocial()
const { isPreviewConnected } = useCorePreview()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const query = ref('')
const inviteRole = ref<'admin' | 'moderator' | 'member'>('member')
const searching = ref(false)
const results = ref<Awaited<ReturnType<ReturnType<typeof useSocialClient>['searchUsers']>>>([])
const displayGroup = computed(() => group.value ?? (isPreviewConnected.value ? { group: { name: 'Amberite Core' } } : null))
const displayMembers = computed(() =>
	members.value.length
		? members.value
		: [
				{ _id: '1', userId: currentUser.value?.userId ?? '1', role: 'owner' as Role, status: 'active', user: currentUser.value ?? { username: 'ilai', displayName: 'Ilai', image: null } },
				{ _id: '2', userId: '2', role: 'admin' as Role, status: 'active', permissionPreset: 'admin', user: { username: 'maya', displayName: 'Maya', image: null } },
				{ _id: '3', userId: '3', role: 'member' as Role, status: 'active', permissionPreset: 'client-only', user: { username: 'noam', displayName: 'Noam', image: null } },
			],
)
const displayBans = computed(() => bans.value)
const displayCanManage = computed(() => canManage.value || isPreviewConnected.value)
const rank = (role: Role) => (role === 'owner' ? 3 : role === 'admin' ? 2 : 1)
const myRank = computed(() => (myRole.value ? rank(myRole.value) : isPreviewConnected.value ? 3 : 0))

function presetOf(member: Member | (typeof displayMembers)['value'][number]): SyncedPermissionPreset {
	if (member.permissionPreset && (presets as string[]).includes(member.permissionPreset)) {
		return member.permissionPreset as SyncedPermissionPreset
	}
	return member.role === 'owner' ? 'owner' : member.role === 'admin' ? 'admin' : 'member'
}

function roleForPreset(preset: SyncedPermissionPreset): Role {
	return preset === 'admin' ? 'admin' : 'member'
}

function canAct(member: Member | (typeof displayMembers)['value'][number]) {
	return displayCanManage.value && member.userId !== currentUser.value?.userId && rank(member.role) < myRank.value
}

async function search() {
	if (!query.value.trim()) return
	searching.value = true
	try {
		results.value = await useSocialClient().searchUsers(query.value.trim())
	} finally {
		searching.value = false
	}
}

async function invite(userId: string) {
	if (isPreviewConnected.value && !group.value) {
		modal.value?.hide()
		return
	}
	await inviteToGroup({
		inviteeUserId: userId,
		role: inviteRole.value === 'moderator' ? 'member' : inviteRole.value,
	})
	modal.value?.hide()
	query.value = ''
	results.value = []
}
</script>

<template>
	<div v-if="displayGroup" class="flex w-full flex-col gap-5">
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div>
				<h1 class="m-0 text-3xl font-black text-contrast">Members</h1>
				<p class="m-0 text-secondary">Manage invites, roles, bans, and access tiers.</p>
			</div>
			<ButtonStyled v-if="displayCanManage" color="brand">
				<button @click="modal?.show()"><UserPlusIcon /> Invite member</button>
			</ButtonStyled>
		</div>

		<section class="rounded-2xl bg-surface-3 p-5">
			<div class="mb-4 flex items-center gap-2">
				<UsersIcon class="text-brand" />
				<h2 class="m-0 text-xl font-bold text-contrast">Group roster</h2>
				<Badge>{{ displayMembers.length }}</Badge>
			</div>
			<div v-for="member in displayMembers" :key="member._id" class="flex flex-wrap items-center justify-between gap-3 rounded-xl p-3 hover:bg-button-bg">
				<div class="flex items-center gap-3">
					<Avatar :src="member.user?.image" :alt="member.user?.username" size="42px" circle />
					<div>
						<div class="font-bold text-contrast">
							{{ member.user?.displayName ?? member.user?.username ?? member.userId }}
							<span v-if="member.userId === currentUser?.userId" class="text-secondary">(you)</span>
						</div>
						<div class="text-sm text-secondary">{{ SYNCED_PERMISSION_PRESET_LABELS[presetOf(member)] }}</div>
					</div>
				</div>
				<div v-if="canAct(member)" class="flex items-center gap-2">
					<select class="rounded-xl border-0 bg-surface-4 px-3 py-2" :value="presetOf(member)" @change="setMemberRole(member.userId, roleForPreset(($event.target as HTMLSelectElement).value as SyncedPermissionPreset), ($event.target as HTMLSelectElement).value)">
						<option v-for="preset in presets" :key="preset" :value="preset">{{ SYNCED_PERMISSION_PRESET_LABELS[preset] }}</option>
					</select>
					<OverflowMenu :options="[
						{ id: 'owner', shown: myRole === 'owner', action: () => transferOwnership(member.userId) },
						{ id: 'kick', action: () => kickMember(member.userId) },
						{ id: 'ban', color: 'red', action: () => banMember(member.userId) },
					]">
						<MoreVerticalIcon />
						<template #owner>Make owner</template>
						<template #kick>Kick</template>
						<template #ban>Ban</template>
					</OverflowMenu>
				</div>
			</div>
		</section>

		<section v-if="displayCanManage" class="rounded-2xl bg-surface-3 p-5">
			<div class="mb-3 flex items-center gap-2"><BanIcon class="text-red" /><h2 class="m-0 text-xl font-bold text-contrast">Banned users</h2></div>
			<p v-if="!displayBans.length" class="m-0 text-secondary">No bans.</p>
			<div v-for="ban in displayBans" :key="ban._id" class="flex items-center justify-between rounded-xl p-2">
				<span>{{ ban.user?.displayName ?? ban.user?.username ?? ban.userId }}</span>
				<ButtonStyled size="small"><button @click="unbanMember(ban.userId)">Unban</button></ButtonStyled>
			</div>
		</section>

		<ButtonStyled v-if="myRole !== 'owner'" color="red"><button @click="leaveGroup">Leave group</button></ButtonStyled>
		<p v-if="error" class="m-0 text-sm text-red">{{ error.message }}</p>

		<NewModal ref="modal" header="Invite member" max-width="560px">
			<div class="flex flex-col gap-4">
				<div class="flex gap-2">
					<StyledInput v-model="query" :icon="SearchIcon" wrapper-class="flex-1" placeholder="Username or friend code" @keyup.enter="search" />
					<ButtonStyled circular><button :disabled="searching" @click="search"><SearchIcon /></button></ButtonStyled>
				</div>
				<select v-model="inviteRole" class="rounded-xl border-0 bg-surface-4 px-3 py-2">
					<option value="admin">Admin</option>
					<option value="moderator">Moderator</option>
					<option value="member">Member</option>
				</select>
				<div v-for="user in results" :key="user.userId" class="flex items-center justify-between rounded-xl p-2 hover:bg-button-bg">
					<div class="flex items-center gap-3">
						<Avatar :src="user.image" :alt="user.username" size="36px" circle />
						<span class="font-bold">{{ user.displayName ?? user.username }}</span>
					</div>
					<ButtonStyled size="small" color="brand"><button @click="invite(user.userId)">Send invite</button></ButtonStyled>
				</div>
			</div>
		</NewModal>
	</div>
</template>
