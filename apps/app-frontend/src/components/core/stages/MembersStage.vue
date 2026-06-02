<script setup lang="ts">
import { LinkIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import { Avatar, Badge, ButtonStyled, StyledInput } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
const { friends } = useSocial()
const inviteRole = ref<'admin' | 'moderator' | 'member'>('member')
const friendList = computed(() => friends.value?.friends ?? [])
const results = computed(() => {
	const q = ctx.inviteQuery.value.trim().toLowerCase()
	if (!q) return friendList.value
	return friendList.value.filter((x) => (x.user?.username ?? '').toLowerCase().includes(q))
})

function invite(userId?: string) {
	if (!userId) return
	void ctx.inviteUser(userId, inviteRole.value === 'moderator' ? 'member' : inviteRole.value)
}

function inviteBackendRole() {
	return inviteRole.value === 'moderator' ? 'member' : inviteRole.value
}
</script>

<template>
	<div class="flex flex-col gap-5">
		<div class="rounded-2xl bg-surface-3 p-5">
			<div class="mb-4 flex flex-wrap items-center justify-between gap-3">
				<div>
					<h2 class="m-0 text-xl font-bold text-contrast">Send member invites</h2>
					<p class="m-0 text-sm text-secondary">Pick friends or search the user database.</p>
				</div>
				<select v-model="inviteRole" class="rounded-xl border-0 bg-surface-4 px-3 py-2 font-bold">
					<option value="admin">Admin</option>
					<option value="moderator">Moderator</option>
					<option value="member">Member</option>
				</select>
			</div>
			<div class="flex gap-2">
				<StyledInput
					v-model="ctx.inviteQuery.value"
					:icon="SearchIcon"
					placeholder="Search username or friend code"
					wrapper-class="flex-1"
					@keyup.enter="ctx.searchUsers"
				/>
				<ButtonStyled circular>
					<button @click="ctx.searchUsers"><SearchIcon /></button>
				</ButtonStyled>
			</div>
		</div>

		<div class="grid gap-4 md:grid-cols-2">
			<div class="rounded-2xl bg-surface-3 p-4">
				<h3 class="m-0 mb-3 text-base font-bold text-contrast">Friends</h3>
				<p v-if="!results.length" class="m-0 text-sm text-secondary">No matching friends.</p>
				<div v-for="friend in results" :key="friend.friendshipId" class="flex items-center justify-between gap-3 rounded-xl p-2 hover:bg-button-bg">
					<div class="flex items-center gap-3">
						<Avatar :src="friend.user?.image" :alt="friend.user?.username" size="36px" circle />
						<span class="font-semibold">{{ friend.user?.displayName ?? friend.user?.username }}</span>
					</div>
					<ButtonStyled size="small" color="brand"><button @click="invite(friend.user?.userId)"><UserPlusIcon /> Invite</button></ButtonStyled>
				</div>
			</div>
			<div class="rounded-2xl bg-surface-3 p-4">
				<h3 class="m-0 mb-3 text-base font-bold text-contrast">Search results</h3>
				<p v-if="ctx.inviteSearchLoading.value" class="m-0 text-sm text-secondary">Searching...</p>
				<p v-else-if="!ctx.inviteSearchResults.value.length" class="m-0 text-sm text-secondary">Search to find users outside your friend list.</p>
				<div v-for="user in ctx.inviteSearchResults.value" :key="user.userId" class="flex items-center justify-between gap-3 rounded-xl p-2 hover:bg-button-bg">
					<div class="flex items-center gap-3">
						<Avatar :src="user.image" :alt="user.username" size="36px" circle />
						<span class="font-semibold">{{ user.displayName ?? user.username }}</span>
					</div>
					<ButtonStyled size="small" color="brand"><button @click="invite(user.userId)"><UserPlusIcon /> Invite</button></ButtonStyled>
				</div>
			</div>
		</div>

		<div class="rounded-2xl bg-surface-3 p-4">
			<div class="mb-3 flex items-center justify-between gap-3">
				<h3 class="m-0 text-base font-bold text-contrast">Invite code</h3>
				<ButtonStyled type="outlined"><button @click="ctx.generateInviteLink(inviteBackendRole())"><LinkIcon /> Generate</button></ButtonStyled>
			</div>
			<code v-if="ctx.generatedInviteCode.value" class="block rounded-xl bg-surface-4 p-3 font-mono">{{ ctx.generatedInviteCode.value }}</code>
			<div v-if="ctx.invitedUsers.value.length" class="mt-3 flex flex-wrap gap-2">
				<Badge v-for="user in ctx.invitedUsers.value" :key="user.userId">{{ user.username }}</Badge>
			</div>
		</div>
	</div>
</template>
