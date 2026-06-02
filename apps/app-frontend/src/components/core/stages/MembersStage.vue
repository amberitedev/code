<script setup lang="ts">
import { LinkIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled } from '@modrinth/ui'
import { computed } from 'vue'

import { useSocial } from '@/composables/useSocial'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
const { friends } = useSocial()

const friendList = computed(() => friends.value?.friends ?? [])
const filteredFriends = computed(() => {
	const q = ctx.inviteQuery.value.trim().toLowerCase()
	if (!q) return friendList.value
	return friendList.value.filter((f) => (f.user?.username ?? '').toLowerCase().includes(q))
})
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="flex flex-col gap-2">
			<span class="font-semibold">Invite members</span>
			<p class="text-secondary text-sm m-0">
				Search users or paste a friend code to invite people to your group.
			</p>
		</div>

		<div class="flex gap-2">
			<input
				v-model="ctx.inviteQuery.value"
				class="flex-1 rounded-lg bg-bg-input px-3 py-2"
				placeholder="Search by username..."
				@keyup.enter="ctx.searchUsers"
			/>
			<ButtonStyled circular @click="ctx.searchUsers">
				<SearchIcon />
			</ButtonStyled>
		</div>

		<div v-if="ctx.inviteSearchLoading.value" class="text-secondary text-sm">Searching...</div>
		<div
			v-else-if="ctx.inviteSearchResults.value.length"
			class="flex flex-col gap-2 max-h-40 overflow-y-auto"
		>
			<div
				v-for="user in ctx.inviteSearchResults.value"
				:key="user.userId"
				class="flex items-center justify-between gap-2 p-2 rounded-lg hover:bg-button-bg transition-colors"
			>
				<div class="flex items-center gap-2">
					<Avatar :src="user.image" :alt="user.username" size="32px" circle />
					<span class="text-sm">{{ user.displayName ?? user.username }}</span>
				</div>
				<ButtonStyled size="small" color="brand" @click="ctx.inviteUser(user.userId, 'member')">
					<UserPlusIcon />
				</ButtonStyled>
			</div>
		</div>

		<div v-if="filteredFriends.length" class="flex flex-col gap-2">
			<span class="text-xs font-semibold text-secondary">Quick invite — friends</span>
			<div
				v-for="f in filteredFriends"
				:key="f.friendshipId"
				class="flex items-center justify-between gap-2 p-2 rounded-lg hover:bg-button-bg transition-colors"
			>
				<div class="flex items-center gap-2">
					<Avatar :src="f.user?.image" :alt="f.user?.username" size="32px" circle />
					<span class="text-sm">{{ f.user?.displayName ?? f.user?.username }}</span>
				</div>
				<ButtonStyled size="small" color="brand" @click="ctx.inviteUser(f.user?.userId, 'member')">
					<UserPlusIcon />
				</ButtonStyled>
			</div>
		</div>

		<div class="border-t border-surface-5 pt-3 flex flex-col gap-2">
			<span class="font-semibold text-sm">Or generate an invite link</span>
			<div class="flex gap-2">
				<ButtonStyled type="outlined" @click="ctx.generateInviteLink('member')">
					<LinkIcon /> Generate link
				</ButtonStyled>
			</div>
			<div v-if="ctx.generatedInviteCode.value" class="flex items-center gap-2">
				<code class="rounded-lg bg-bg-input px-3 py-2 font-mono text-sm flex-1">{{
					ctx.generatedInviteCode.value
				}}</code>
				<ButtonStyled
					size="small"
					@click="
						() => {
							navigator.clipboard.writeText(ctx.generatedInviteCode.value!)
						}
					"
				>
					Copy
				</ButtonStyled>
			</div>
		</div>

		<div v-if="ctx.invitedUsers.value.length" class="flex flex-col gap-2">
			<span class="text-xs font-semibold text-secondary">Invited</span>
			<div
				v-for="u in ctx.invitedUsers.value"
				:key="u.userId"
				class="flex items-center gap-2 p-2 rounded-lg bg-bg-input"
			>
				<Avatar :src="u.image" :alt="u.username" size="24px" circle />
				<span class="text-sm">{{ u.username }}</span>
			</div>
		</div>
	</div>
</template>
