<script setup lang="ts">
import { LinkIcon, PlusIcon, ServerStackIcon, UsersIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled } from '@modrinth/ui'
import { computed, useTemplateRef } from 'vue'
import { useRouter } from 'vue-router'

import CoreOnboardingModal from '@/components/core/CoreOnboardingModal.vue'
import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CoreOverviewPage' })

const router = useRouter()
const { group, members, invites, friends, currentUser, acceptInvite, declineInvite } = useSocial()
const setupModal = useTemplateRef<InstanceType<typeof CoreOnboardingModal>>('setupModal')
const connectModal = useTemplateRef<InstanceType<typeof CoreOnboardingModal>>('connectModal')

const core = computed(() => group.value?.core ?? null)
const coreOnline = computed(() => {
	const seen = core.value?.lastSeenAt
	return seen ? Date.now() - seen < 60_000 : false
})
const incomingFriendRequests = computed(() => friends.value?.incoming ?? [])
</script>

<template>
	<div class="flex flex-col gap-6 w-full">
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

		<div v-else class="flex flex-col gap-4">
			<div class="text-center max-w-xl mx-auto py-4">
				<h2 class="m-0 mb-2 text-2xl">Get started with Amberite Core</h2>
				<p class="m-0 text-secondary">
					Pair a Core server you've already set up, or create a brand-new one.
				</p>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-4 max-w-3xl w-full mx-auto">
				<button
					type="button"
					class="text-left rounded-2xl bg-bg-raised p-6 flex flex-col gap-3 transition-all border-2 border-transparent hover:border-brand cursor-pointer"
					@click="connectModal?.show()"
				>
					<div class="flex items-center gap-3">
						<span
							class="w-10 h-10 rounded-xl bg-brand/20 text-brand flex items-center justify-center"
						>
							<LinkIcon class="w-5 h-5" />
						</span>
						<div class="flex flex-col">
							<span class="font-bold text-lg">Connect to a Core</span>
							<span class="text-secondary text-xs">Join an existing friend group</span>
						</div>
					</div>
					<p class="m-0 text-secondary text-sm">
						Already have a Core running? Enter its pairing code to join its friend group and start
						syncing servers.
					</p>
					<div class="flex items-center gap-1 text-brand font-semibold text-sm mt-auto">
						Connect <span aria-hidden>→</span>
					</div>
				</button>

				<button
					type="button"
					class="text-left rounded-2xl bg-bg-raised p-6 flex flex-col gap-3 transition-all border-2 border-transparent hover:border-brand cursor-pointer"
					@click="setupModal?.show()"
				>
					<div class="flex items-center gap-3">
						<span
							class="w-10 h-10 rounded-xl bg-brand/20 text-brand flex items-center justify-center"
						>
							<PlusIcon class="w-5 h-5" />
						</span>
						<div class="flex flex-col">
							<span class="font-bold text-lg">Set up a new Core</span>
							<span class="text-secondary text-xs">Create a fresh friend group</span>
						</div>
					</div>
					<p class="m-0 text-secondary text-sm">
						Start from scratch — pair a new Core server, name your group, and invite your first
						members.
					</p>
					<div class="flex items-center gap-1 text-brand font-semibold text-sm mt-auto">
						Set up <span aria-hidden>→</span>
					</div>
				</button>
			</div>

			<div
				v-if="invites.length > 0"
				class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3 max-w-3xl w-full mx-auto"
			>
				<div class="flex items-center gap-2">
					<ServerStackIcon class="w-5 h-5 text-brand" />
					<span class="font-bold">Group invites</span>
				</div>
				<div
					v-for="entry in invites"
					:key="entry.invite._id"
					class="flex items-center justify-between gap-2 border-b border-surface-5 last:border-0 pb-2 last:pb-0"
				>
					<span class="text-sm"
						>{{ (entry.group as any)?.name ?? 'A Core group' }} · role:
						{{ entry.invite.role }}</span
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
		</div>

		<div
			v-if="incomingFriendRequests.length > 0 && group"
			class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-2"
		>
			<span class="font-bold">Friend requests</span>
			<span class="text-secondary text-sm"
				>{{ incomingFriendRequests.length }} pending — manage them in Members.</span
			>
		</div>

		<CoreOnboardingModal ref="connectModal" mode="connect" />
		<CoreOnboardingModal ref="setupModal" mode="setup" />
	</div>
</template>
