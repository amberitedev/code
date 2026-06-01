<script setup lang="ts">
import { Avatar, ButtonStyled } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'
import {
	SYNCED_PERMISSION_PRESET_LABELS,
	type SyncedPermissionPreset,
} from '@/pages/instance/synced/use-synced-permissions'

defineOptions({ name: 'CoreMembersPage' })

type Role = 'owner' | 'admin' | 'member'
type Member = (typeof members)['value'][number]

/** Presets an owner/admin may assign. Ownership is changed via transfer, not here. */
const ASSIGNABLE_PRESETS: SyncedPermissionPreset[] = ['admin', 'member', 'client-only', 'viewer']

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

const inviteInput = ref('')
const inviteRole = ref<Role>('member')
const inviteSending = ref(false)

const rank = (r: Role) => (r === 'owner' ? 3 : r === 'admin' ? 2 : 1)
const myRank = computed(() => (myRole.value ? rank(myRole.value) : 0))

function canActOn(role: Role, userId: string): boolean {
	if (!canManage.value) return false
	if (userId === currentUser.value?.userId) return false
	return rank(role) < myRank.value
}

/** Effective synced permission preset for a member (stored preset or role default). */
function presetOf(member: Member): SyncedPermissionPreset {
	const stored = member.permissionPreset
	if (stored && (ASSIGNABLE_PRESETS as string[]).includes(stored)) {
		return stored as SyncedPermissionPreset
	}
	if (member.role === 'owner') return 'owner'
	if (member.role === 'admin') return 'admin'
	return 'member'
}

function roleForPreset(preset: SyncedPermissionPreset): Role {
	if (preset === 'owner') return 'owner'
	if (preset === 'admin') return 'admin'
	return 'member'
}

async function setPreset(member: Member, preset: SyncedPermissionPreset) {
	await setMemberRole(member.userId, roleForPreset(preset), preset)
}

async function invite() {
	const value = inviteInput.value.trim()
	if (!value) return
	inviteSending.value = true
	// Try to find user by code/username first, then invite by userId
	try {
		const client = (await import('@/composables/useSocialClient')).useSocialClient()
		const results = await client.searchUsers(value)
		const match = results[0]
		if (match?.userId) {
			await inviteToGroup({ inviteeUserId: match.userId, role: inviteRole.value })
		} else {
			// Fallback: treat as direct userId
			await inviteToGroup({ inviteeUserId: value, role: inviteRole.value })
		}
	} catch {
		// Fallback: treat as direct userId
		await inviteToGroup({ inviteeUserId: value, role: inviteRole.value })
	}
	inviteSending.value = false
	inviteInput.value = ''
}
</script>

<template>
	<div v-if="group" class="flex flex-col gap-4 w-full">
		<div class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-4">
			<span class="font-bold text-lg">Members ({{ members.length }})</span>
			<div
				v-for="member in members"
				:key="member._id"
				class="flex items-center justify-between gap-2 border-b border-surface-5 last:border-0 pb-3 last:pb-0"
			>
				<div class="flex items-center gap-3">
					<Avatar :src="member.user?.image" :alt="member.user?.username" size="40px" circle />
					<div class="flex flex-col">
						<span class="font-semibold">
							{{ member.user?.displayName ?? member.user?.username ?? member.userId }}
							<span v-if="member.userId === currentUser?.userId" class="text-secondary">(you)</span>
						</span>
						<span class="text-secondary text-xs">
							{{ SYNCED_PERMISSION_PRESET_LABELS[presetOf(member)] }}
						</span>
					</div>
				</div>
				<div v-if="canActOn(member.role, member.userId)" class="flex gap-1 flex-wrap justify-end">
					<select
						:value="presetOf(member)"
						class="rounded-lg bg-bg-input px-2 py-1 text-sm"
						@change="
							setPreset(
								member,
								($event.target as HTMLSelectElement).value as SyncedPermissionPreset,
							)
						"
					>
						<option v-for="preset in ASSIGNABLE_PRESETS" :key="preset" :value="preset">
							{{ SYNCED_PERMISSION_PRESET_LABELS[preset] }}
						</option>
					</select>
					<ButtonStyled v-if="myRole === 'owner'" size="small">
						<button @click="transferOwnership(member.userId)">Make owner</button>
					</ButtonStyled>
					<ButtonStyled size="small">
						<button @click="kickMember(member.userId)">Kick</button>
					</ButtonStyled>
					<ButtonStyled size="small" color="red">
						<button @click="banMember(member.userId)">Ban</button>
					</ButtonStyled>
				</div>
			</div>
		</div>

		<div v-if="canManage" class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-3">
			<span class="font-bold text-lg">Add Member to Friend Group</span>
			<p class="text-secondary text-sm m-0">
				Enter a friend code (AMB-XXXXXX) or a username to invite them.
			</p>
			<div class="flex gap-2">
				<input
					v-model="inviteInput"
					class="flex-1 rounded-lg bg-bg-input px-3 py-2"
					placeholder="AMB-ABC123 or username"
					@keyup.enter="invite"
				/>
				<select v-model="inviteRole" class="rounded-lg bg-bg-input px-3 py-2">
					<option value="member">member</option>
					<option value="admin">admin</option>
				</select>
				<ButtonStyled color="brand" :disabled="inviteSending" @click="invite">
					{{ inviteSending ? 'Inviting…' : 'Invite' }}
				</ButtonStyled>
			</div>
		</div>

		<div
			v-if="canManage && bans.length > 0"
			class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-3"
		>
			<span class="font-bold text-lg">Banned ({{ bans.length }})</span>
			<div v-for="ban in bans" :key="ban._id" class="flex items-center justify-between gap-2">
				<div class="flex items-center gap-2">
					<Avatar :src="ban.user?.image" :alt="ban.user?.username" size="32px" circle />
					<span>{{ ban.user?.displayName ?? ban.user?.username ?? ban.userId }}</span>
				</div>
				<ButtonStyled size="small">
					<button @click="unbanMember(ban.userId)">Unban</button>
				</ButtonStyled>
			</div>
		</div>

		<div class="flex">
			<ButtonStyled v-if="myRole !== 'owner'" color="red">
				<button @click="leaveGroup">Leave group</button>
			</ButtonStyled>
		</div>

		<p v-if="error" class="text-red text-sm m-0">{{ error.message }}</p>
	</div>
</template>
