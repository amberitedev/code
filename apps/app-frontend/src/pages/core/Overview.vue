<script setup lang="ts">
import { PlusIcon, UsersIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled } from '@modrinth/ui'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CoreOverviewPage' })

const router = useRouter()
const { group, members, invites, friends, currentUser, acceptInvite, declineInvite } = useSocial()

const core = computed(() => group.value?.core ?? null)
const coreOnline = computed(() => {
	const seen = core.value?.lastSeenAt
	return seen ? Date.now() - seen < 60_000 : false
})
const incomingFriendRequests = computed(() => friends.value?.incoming ?? [])
</script>

<template>
	<div class="flex flex-col gap-4 w-full">
		<div v-if="currentUser" class="flex items-center gap-3">
			<Avatar :src="currentUser.image" :alt="currentUser.username" size="48px" circle />
			<div class="flex flex-col">
				<span class="font-bold text-lg">{{ currentUser.displayName ?? currentUser.username }}</span>
				<span class="text-secondary text-sm">Friend code: {{ currentUser.friendCode ?? '—' }}</span>
			</div>
		</div>

		<div v-if="group" class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3">
			<div class="flex items-center justify-between">
				<div class="flex flex-col">
					<span class="font-bold text-lg">{{ group.group.name ?? 'Your Core' }}</span>
					<span class="text-secondary text-sm"
						>{{ members.length }} member(s) · your role: {{ group.role }}</span
					>
				</div>
				<span
					class="px-2 py-1 rounded-full text-xs font-bold"
					:class="coreOnline ? 'bg-green/20 text-green' : 'bg-surface-5 text-secondary'"
				>
					{{ coreOnline ? 'Online' : 'Offline' }}
				</span>
			</div>
			<p v-if="group.group.description" class="m-0 text-secondary">{{ group.group.description }}</p>
			<div class="flex gap-2">
				<ButtonStyled>
					<button @click="router.push('/core/members')"><UsersIcon /> Members</button>
				</ButtonStyled>
				<ButtonStyled v-if="group.role === 'owner' || group.role === 'admin'">
					<button @click="router.push('/core/settings')">Core Settings</button>
				</ButtonStyled>
			</div>
		</div>

		<div v-else class="rounded-2xl bg-bg-raised p-6 flex flex-col items-center gap-3 text-center">
			<UsersIcon class="w-16 h-16 text-secondary" />
			<h3 class="m-0">You're not in a Core group yet</h3>
			<p class="m-0 text-secondary">Pair a Core server or accept an invite to get started.</p>
			<ButtonStyled color="brand">
				<button @click="router.push('/core/setup')"><PlusIcon /> Set up a Core</button>
			</ButtonStyled>
		</div>

		<div v-if="invites.length > 0" class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3">
			<span class="font-bold">Group invites</span>
			<div
				v-for="entry in invites"
				:key="entry.invite._id"
				class="flex items-center justify-between gap-2"
			>
				<span
					>{{ (entry.group as any)?.name ?? 'A Core group' }} · role: {{ entry.invite.role }}</span
				>
				<div class="flex gap-2">
					<ButtonStyled color="brand">
						<button @click="acceptInvite({ inviteId: entry.invite._id })">Accept</button>
					</ButtonStyled>
					<ButtonStyled>
						<button @click="declineInvite(entry.invite._id)">Decline</button>
					</ButtonStyled>
				</div>
			</div>
		</div>

		<div
			v-if="incomingFriendRequests.length > 0"
			class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-2"
		>
			<span class="font-bold">Friend requests</span>
			<span class="text-secondary text-sm"
				>{{ incomingFriendRequests.length }} pending — manage them in Members.</span
			>
		</div>
	</div>
</template>
