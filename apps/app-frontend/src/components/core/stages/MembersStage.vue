<script setup lang="ts">
import { CheckIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled, StyledInput } from '@modrinth/ui'
import { computed, reactive, watch } from 'vue'

import { useSocial } from '@/composables/useSocial'

import { useOnboarding } from '../core-onboarding-context'
import { useCorePreview } from '../use-core-preview'

const ctx = useOnboarding()
const { friends } = useSocial()
const { fakeFriends, fakeUsers } = useCorePreview()
const roleSelections = reactive<Record<string, 'admin' | 'member'>>({})
let searchTimer: ReturnType<typeof window.setTimeout> | null = null

const friendList = computed(() => {
	const realFriends = friends.value?.friends ?? []
	return realFriends.length ? realFriends : fakeFriends
})
const invitedIds = computed(() => new Set(ctx.invitedUsers.value.map((user) => user.userId)))
const query = computed(() => ctx.inviteQuery.value.trim().toLowerCase())
const searchResults = computed(() => {
	const realResults = ctx.inviteSearchResults.value.filter((user) => !invitedIds.value.has(user.userId))
	const fakeResults = fakeUsers
		.slice(4)
		.filter((user) => !invitedIds.value.has(user.userId))
		.filter((user) =>
			query.value
				? `${user.username} ${user.displayName} ${user.friendCode}`.toLowerCase().includes(query.value)
				: false,
		)
	return [...realResults, ...fakeResults]
})
const showSearch = computed(
	() => query.value.length > 0 && (ctx.inviteSearchLoading.value || searchResults.value.length > 0),
)
const rows = computed(() => [
	...ctx.invitedUsers.value.map((user) => ({
		id: user.userId,
		name: user.username,
		image: user.image,
		status: 'Invited',
		role: user.role ?? 'member',
		invited: true,
	})),
	...friendList.value
		.filter((friend) => friend.user?.userId && !invitedIds.value.has(friend.user.userId))
		.filter((friend) =>
			query.value ? (friend.user?.username ?? '').toLowerCase().includes(query.value) : true,
		)
		.map((friend) => ({
			id: friend.user!.userId,
			name: friend.user?.displayName ?? friend.user?.username ?? friend.user!.userId,
			image: friend.user?.image,
			status: 'Friend',
			role: roleSelections[friend.user!.userId] ?? 'member',
			invited: false,
		})),
])

async function invite(userId: string) {
	if (invitedIds.value.has(userId)) return
	await ctx.inviteUser(userId, roleSelections[userId] ?? 'member')
}

watch(
	() => ctx.inviteQuery.value,
	(value) => {
		if (searchTimer) window.clearTimeout(searchTimer)
		if (!value.trim()) {
			ctx.inviteSearchResults.value = []
			return
		}
		searchTimer = window.setTimeout(() => {
			void ctx.searchUsers()
		}, 250)
	},
)
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="relative">
			<StyledInput
				v-model="ctx.inviteQuery.value"
				:icon="SearchIcon"
				placeholder="Search friends or usernames"
				wrapper-class="w-full"
				@keyup.enter="ctx.searchUsers"
			/>
			<div
				v-if="showSearch"
				class="absolute bottom-[calc(100%+0.5rem)] left-0 z-10 flex max-h-56 w-full flex-col overflow-y-auto rounded-2xl border border-solid border-button-border bg-surface-3 shadow-xl"
			>
				<div v-if="ctx.inviteSearchLoading.value" class="px-4 py-3 text-sm text-secondary">
					Searching...
				</div>
				<template v-else>
					<button
						v-for="user in searchResults"
						:key="user.userId"
						type="button"
						class="flex items-center justify-between gap-3 border-0 border-b border-solid border-surface-5 bg-transparent px-4 py-3 text-left text-primary last:border-b-0 hover:bg-button-bg"
						@click="invite(user.userId)"
					>
						<span class="flex min-w-0 items-center gap-3">
							<Avatar :src="user.image" :alt="user.username" size="34px" circle />
							<span class="truncate font-semibold">{{ user.displayName ?? user.username }}</span>
						</span>
						<UserPlusIcon class="size-5 text-secondary" />
					</button>
				</template>
			</div>
		</div>

		<div class="overflow-hidden rounded-2xl border border-solid border-button-border bg-surface-3">
			<div
				class="grid grid-cols-[minmax(0,1fr)_7rem_9rem] gap-4 border-0 border-b border-solid border-surface-5 px-4 py-3 text-sm font-bold text-secondary"
			>
				<span>Name</span>
				<span>Status</span>
				<span class="text-right">Permissions</span>
			</div>
			<div v-if="rows.length === 0" class="px-4 py-8 text-sm text-secondary">
				Search for someone to invite.
			</div>
			<div
				v-for="row in rows"
				:key="row.id"
				class="grid grid-cols-[minmax(0,1fr)_7rem_9rem] items-center gap-4 border-0 border-b border-solid border-surface-5 px-4 py-3 last:border-b-0"
			>
				<button
					type="button"
					class="flex min-w-0 items-center gap-3 border-0 bg-transparent p-0 text-left text-primary"
					:class="row.invited ? 'cursor-default' : 'cursor-pointer'"
					@click="invite(row.id)"
				>
					<Avatar :src="row.image" :alt="row.name" size="38px" circle />
					<span class="min-w-0 truncate font-semibold">{{ row.name }}</span>
				</button>
				<span class="text-sm text-secondary">{{ row.status }}</span>
				<div class="flex items-center justify-end gap-2">
					<select
						:value="row.role"
						class="rounded-xl border-0 bg-surface-4 px-3 py-2 text-sm"
						:disabled="row.invited"
						@change="roleSelections[row.id] = ($event.target as HTMLSelectElement).value as 'admin' | 'member'"
					>
						<option value="member">Member</option>
						<option value="admin">Admin</option>
					</select>
					<ButtonStyled v-if="!row.invited" size="small" color="brand" circular>
						<button @click="invite(row.id)"><UserPlusIcon /></button>
					</ButtonStyled>
					<CheckIcon v-else class="size-5 text-green" />
				</div>
			</div>
		</div>
	</div>
</template>
