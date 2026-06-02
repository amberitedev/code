<script setup lang="ts">
import {
	ChartIcon,
	LinkIcon,
	PlusIcon,
	ServerStackIcon,
	UsersIcon,
} from '@modrinth/assets'
import { Avatar, Badge, ButtonStyled } from '@modrinth/ui'
import { computed, onMounted, useTemplateRef, watch } from 'vue'
import { useRouter } from 'vue-router'

import CoreOnboardingModal from '@/components/core/CoreOnboardingModal.vue'
import { useCorePreview } from '@/components/core/use-core-preview'
import { useSocial } from '@/composables/useSocial'
import { useSyncedServers } from '@/composables/useSyncedServers'

defineOptions({ name: 'CoreOverviewPage' })

const router = useRouter()
const { group, members, invites, currentUser, acceptInvite, declineInvite } = useSocial()
const { profiles, refresh } = useSyncedServers()
const { isPreviewConnected, isPreviewLocal } = useCorePreview()
const setupModal = useTemplateRef<InstanceType<typeof CoreOnboardingModal>>('setupModal')
const connectModal = useTemplateRef<InstanceType<typeof CoreOnboardingModal>>('connectModal')
const previewCore = computed(() => ({
	setupMode: isPreviewLocal.value ? 'local' : 'external',
	lastSeenAt: Date.now(),
}))
const displayGroup = computed(() =>
	group.value ??
	(isPreviewConnected.value
		? { group: { id: 'preview-core', name: 'Amberite Core', description: 'Servers, sync, and members in one place.' }, core: previewCore.value }
		: null),
)
const displayMembers = computed(() =>
	members.value.length
		? members.value
		: [
				{ _id: '1', userId: '1', role: 'owner', status: 'active', user: { username: 'ilai', displayName: 'Ilai', image: null } },
				{ _id: '2', userId: '2', role: 'admin', status: 'active', user: { username: 'maya', displayName: 'Maya', image: null } },
				{ _id: '3', userId: '3', role: 'member', status: 'active', user: { username: 'noam', displayName: 'Noam', image: null } },
			],
)
const displayProfiles = computed(() =>
	profiles.value.length
		? profiles.value
		: [
				{ _id: '1', name: 'Survival SMP', status: 'active', gameVersion: '1.21.1', loader: 'Fabric' },
				{ _id: '2', name: 'Creative Lab', status: 'idle', gameVersion: '1.20.6', loader: 'NeoForge' },
			],
)
const core = computed(() => displayGroup.value?.core ?? null)
const groupId = computed(() => group.value?.group.id ?? null)
const online = computed(() => {
	const seen = core.value?.lastSeenAt
	return seen ? Date.now() - seen < 60_000 : false
})
const activeServers = computed(() => displayProfiles.value.filter((x) => x.status === 'active').length)
const activeMembers = computed(() => displayMembers.value.filter((x) => x.status !== 'banned').length)

async function loadServers() {
	if (groupId.value) await refresh(groupId.value)
}

onMounted(loadServers)
watch(groupId, loadServers)
</script>

<template>
	<div class="flex w-full flex-col gap-5">
		<section v-if="!displayGroup" class="overflow-hidden rounded-[28px] bg-surface-3">
			<div class="grid gap-6 p-6 lg:grid-cols-[1.1fr_0.9fr] lg:p-8">
				<div class="flex flex-col justify-between gap-10">
					<div>
						<Badge>Amberite Core</Badge>
						<h1 class="mb-3 mt-4 max-w-2xl text-4xl font-black text-contrast">
							Host the server manager your friends actually use.
						</h1>
						<p class="m-0 max-w-xl text-lg text-secondary">
							Connect to a Core already running elsewhere, or set up a local Core with a guided
							install, invites, permissions, and runtime settings.
						</p>
					</div>
					<div v-if="currentUser" class="flex items-center gap-3">
						<Avatar :src="currentUser.image" :alt="currentUser.username" size="42px" circle />
						<div>
							<div class="font-bold text-contrast">{{ currentUser.displayName ?? currentUser.username }}</div>
							<div class="text-sm text-secondary">Friend code: {{ currentUser.friendCode ?? 'Unavailable' }}</div>
						</div>
					</div>
				</div>
				<div class="grid gap-4">
					<button class="core-choice" type="button" @click="connectModal?.show()">
						<LinkIcon class="text-brand" />
						<span>
							<strong>Connect existing Core</strong>
							<small>Use the six-digit terminal pairing code.</small>
						</span>
					</button>
					<button class="core-choice featured" type="button" @click="setupModal?.show()">
						<PlusIcon class="text-brand" />
						<span>
							<strong>Set up local Core</strong>
							<small>Install, verify, link, and invite your first members.</small>
						</span>
					</button>
				</div>
			</div>
		</section>

		<template v-else>
			<section class="overflow-hidden rounded-[28px] bg-surface-3">
				<div class="flex flex-wrap items-center justify-between gap-4 p-6">
					<div>
						<div class="mb-2 flex items-center gap-2">
							<Badge>{{ online ? 'Online' : 'Offline' }}</Badge>
							<Badge>{{ core?.setupMode ?? 'local' }}</Badge>
						</div>
						<h1 class="m-0 text-3xl font-black text-contrast">{{ displayGroup.group.name ?? 'Your Core' }}</h1>
						<p class="m-0 mt-2 text-secondary">{{ displayGroup.group.description ?? 'Servers, sync, and members in one place.' }}</p>
					</div>
					<div class="flex gap-2">
						<ButtonStyled><button @click="router.push('/core/members')"><UsersIcon /> Members</button></ButtonStyled>
						<ButtonStyled color="brand"><button @click="router.push('/core/servers')"><ServerStackIcon /> Servers</button></ButtonStyled>
					</div>
				</div>
			</section>

			<div class="grid gap-4 md:grid-cols-3">
				<div class="stat-card"><ServerStackIcon /><strong>{{ displayProfiles.length }}</strong><span>Synced servers</span></div>
				<div class="stat-card"><ChartIcon /><strong>{{ activeServers }}</strong><span>Active syncs</span></div>
				<div class="stat-card"><UsersIcon /><strong>{{ activeMembers }}</strong><span>Active members</span></div>
			</div>

			<div class="grid gap-4 lg:grid-cols-[1fr_0.8fr]">
				<section class="rounded-2xl bg-surface-3 p-5">
					<h2 class="m-0 mb-3 text-xl font-bold text-contrast">Server glance</h2>
					<p v-if="!displayProfiles.length" class="m-0 text-secondary">No synced servers yet.</p>
					<div v-for="profile in displayProfiles" :key="profile._id" class="flex items-center justify-between rounded-xl p-3 hover:bg-button-bg">
						<div class="font-bold text-contrast">{{ profile.name }}</div>
						<div class="text-sm text-secondary">{{ profile.gameVersion }} {{ profile.loader }}</div>
					</div>
				</section>
				<section class="rounded-2xl bg-surface-3 p-5">
					<h2 class="m-0 mb-3 text-xl font-bold text-contrast">Online members</h2>
					<div v-for="member in displayMembers.slice(0, 5)" :key="member._id" class="flex items-center gap-3 rounded-xl p-2">
						<Avatar :src="member.user?.image" :alt="member.user?.username" size="34px" circle />
						<div>
							<div class="font-bold text-contrast">{{ member.user?.displayName ?? member.user?.username ?? member.userId }}</div>
							<div class="text-xs text-secondary">{{ member.role }}</div>
						</div>
					</div>
				</section>
			</div>
		</template>

		<section v-if="invites.length" class="rounded-2xl bg-surface-3 p-5">
			<h2 class="m-0 mb-3 text-xl font-bold text-contrast">Group invites</h2>
			<div v-for="entry in invites" :key="entry.invite._id" class="flex items-center justify-between gap-3 rounded-xl p-2">
				<span>{{ (entry.group as any)?.name ?? 'Core group' }} - {{ entry.invite.role }}</span>
				<div class="flex gap-2">
					<ButtonStyled color="brand"><button @click="acceptInvite({ inviteId: entry.invite._id })">Accept</button></ButtonStyled>
					<ButtonStyled><button @click="declineInvite(entry.invite._id)">Decline</button></ButtonStyled>
				</div>
			</div>
		</section>

		<CoreOnboardingModal ref="connectModal" mode="connect" />
		<CoreOnboardingModal ref="setupModal" mode="setup" />
	</div>
</template>

<style scoped>
.core-choice {
	@apply flex min-h-36 cursor-pointer items-center gap-4 rounded-2xl border-2 border-transparent bg-surface-4 p-5 text-left transition-all hover:border-brand;
}
.core-choice.featured {
	@apply bg-brand-highlight;
}
.core-choice strong {
	@apply block text-lg text-contrast;
}
.core-choice small {
	@apply text-sm text-secondary;
}
.stat-card {
	@apply flex flex-col gap-2 rounded-2xl bg-surface-3 p-5 text-secondary;
}
.stat-card strong {
	@apply text-3xl text-contrast;
}
</style>
