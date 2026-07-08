<script lang="ts">
import type {
	FriendGroupMember as CachedFriendGroupMember,
	FriendGroupSummary as CachedFriendGroupSummary,
	GroupInviteWithGroup as CachedGroupInviteWithGroup,
} from '@amberite/amberite-api'

type CoreOverviewCacheEntry = {
	group: CachedFriendGroupSummary
	members: CachedFriendGroupMember[]
	invites: CachedGroupInviteWithGroup[]
	role: CachedFriendGroupSummary['role'] | null
}

const coreOverviewCache = new Map<string, CoreOverviewCacheEntry>()
</script>

<script setup lang="ts">
import type { FriendGroupMember } from '@amberite/amberite-api'
import {
	CalendarIcon,
	ClipboardCopyIcon,
	ServerStackIcon,
	SettingsIcon,
	UserIcon,
	UsersIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	Card,
	ContentPageHeader,
	injectNotificationManager,
	useFormatDateTime,
	useFormatNumber,
	useRelativeTime,
} from '@modrinth/ui'
import { computed, onMounted, ref, watch } from 'vue'

import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'
import { useConnectedCore } from '@/core/connected-core'

const emit = defineEmits<{
	'open-settings': []
}>()

const social = useSocial()
const connection = useCoreConnection()
const connectedCore = useConnectedCore()
const notifications = injectNotificationManager()
const formatNumber = useFormatNumber()
const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({
	timeStyle: 'short',
	dateStyle: 'long',
})

const overviewCacheKey = computed(
	() =>
		connectedCore.value?.coreId ??
		social.group.value?.core?.coreId ??
		social.group.value?.group.coreId ??
		'core',
)
const cachedOverview = ref<CoreOverviewCacheEntry | null>(
	coreOverviewCache.get(overviewCacheKey.value) ?? null,
)
const overview = computed(() => {
	return liveOverviewSnapshot() ?? cachedOverview.value
})
const overviewMembers = computed(() => overview.value?.members ?? [])
const overviewInvites = computed(() => overview.value?.invites ?? [])
const group = computed(() => overview.value?.group.group)
const core = computed(() => overview.value?.group.core)
const coreId = computed(() => core.value?.coreId || connectedCore.value?.coreId || 'Unknown')
const coreUrl = computed(() => core.value?.connectionUrl || connectedCore.value?.url || 'Unknown')
const linkedAt = computed(() => new Date(connectedCore.value?.linkedAt ?? Date.now()))
const groupName = computed(() => group.value?.name || 'Friend group')
const groupDescription = computed(
	() => group.value?.description || 'A shared Amberite group for managing Core access and servers.',
)
const createdAt = computed(() => new Date(group.value?.createdAt ?? Date.now()))
const memberCount = computed(() => overviewMembers.value.length)
const inviteCount = computed(() => overviewInvites.value.length)
const owner = computed(() =>
	overviewMembers.value.find((member) => member.role === 'owner') ?? overviewMembers.value[0],
)
const statusLabel = computed(() => {
	if (!connectedCore.value) return 'No Core linked'
	if (connection.loading.value) return 'Checking'
	return connection.status.value?.state === 'connected' ? 'Online' : 'Offline'
})
const statusClass = computed(() => {
	if (!connectedCore.value) return 'bg-red'
	if (connection.loading.value) return 'bg-orange'
	return connection.status.value?.state === 'connected' ? 'bg-green' : 'bg-red'
})
const roleLabel = computed(() => {
	const role = overview.value?.role
	if (!role) return 'Unknown'
	return role.charAt(0).toUpperCase() + role.slice(1)
})
const visibleMembers = computed(() => overviewMembers.value.slice(0, 8))

function displayName(member: FriendGroupMember) {
	return (
		member.user?.displayName ||
		member.user?.name ||
		member.user?.username ||
		member.userId
	)
}

function avatarUrl(member: FriendGroupMember) {
	return member.user?.avatar_url || member.user?.image
}

function copyGroupId() {
	if (!group.value) return
	void navigator.clipboard.writeText(group.value.id)
	notifications.addNotification({
		type: 'success',
		title: 'Copied group ID',
	})
}

function liveOverviewSnapshot(): CoreOverviewCacheEntry | null {
	if (!social.group.value) return null
	const cached = coreOverviewCache.get(overviewCacheKey.value)
	const sameGroup = cached?.group.group.id === social.group.value.group.id
	const cachedMembers = sameGroup && cached ? cached.members : []
	return {
		group: social.group.value,
		members:
			social.members.value.length > 0 || !sameGroup
				? social.members.value
				: cachedMembers,
		invites: social.invites.value,
		role: social.myRole.value ?? (sameGroup && cached ? cached.role : null),
	}
}

function updateOverviewCache() {
	const next = liveOverviewSnapshot()
	if (next) {
		const current = coreOverviewCache.get(overviewCacheKey.value)
		if (!current || !sameSerialized(current, next)) {
			coreOverviewCache.set(overviewCacheKey.value, next)
		}
		cachedOverview.value = next
		return
	}

	cachedOverview.value = coreOverviewCache.get(overviewCacheKey.value) ?? null
}

function sameSerialized(left: unknown, right: unknown) {
	return JSON.stringify(left) === JSON.stringify(right)
}

watch(
	() => [
		overviewCacheKey.value,
		social.group.value,
		social.members.value,
		social.invites.value,
		social.myRole.value,
	],
	updateOverviewCache,
	{ immediate: true },
)

onMounted(() => {
	void social.refresh().catch(() => undefined)
})
</script>

<template>
	<Teleport v-if="group" defer to="#sidebar-teleport-target">
		<div class="flex flex-col gap-4">
			<Card>
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Group info</h2>
				</template>
				<div class="flex flex-col gap-3 text-sm">
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Your role</span>
						<span class="font-semibold text-contrast">{{ roleLabel }}</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Core status</span>
						<span class="flex items-center gap-2 font-semibold text-contrast">
							<span class="size-2.5 rounded-full" :class="statusClass" aria-hidden="true" />
							{{ statusLabel }}
						</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Core ID</span>
						<span class="break-all text-right font-semibold text-contrast">
							{{ core?.coreId || group.coreId || 'Unknown' }}
						</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Group ID</span>
						<button
							class="m-0 cursor-pointer border-0 bg-transparent p-0 text-right text-sm font-semibold text-contrast hover:underline"
							@click="copyGroupId"
						>
							{{ group.id }}
						</button>
					</div>
				</div>
			</Card>

			<Card v-if="visibleMembers.length > 0">
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Members</h2>
				</template>
				<div class="flex flex-wrap gap-2">
					<Avatar
						v-for="member in visibleMembers"
						:key="member.userId"
						v-tooltip="displayName(member)"
						:src="avatarUrl(member)"
						:alt="displayName(member)"
						size="3rem"
						circle
					/>
				</div>
			</Card>
		</div>
	</Teleport>

	<div v-if="group" class="flex flex-col gap-4">
		<ContentPageHeader>
			<template #icon>
				<Avatar :src="group.banner" :alt="groupName" size="96px" circle />
			</template>
			<template #title>
				{{ groupName }}
			</template>
			<template #summary>
				{{ groupDescription }}
			</template>
			<template #stats>
				<div class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold">
					<UsersIcon class="h-6 w-6 text-secondary" />
					{{ formatNumber(memberCount) }}
					{{ memberCount === 1 ? 'member' : 'members' }}
				</div>
				<div class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold">
					<ServerStackIcon class="h-6 w-6 text-secondary" />
					{{ statusLabel }}
				</div>
				<div v-tooltip="formatDateTime(createdAt)" class="flex items-center gap-2 font-semibold">
					<CalendarIcon class="h-6 w-6 text-secondary" />
					Created {{ formatRelativeTime(createdAt) }}
				</div>
			</template>
			<template #actions>
				<ButtonStyled size="large" circular type="transparent">
					<button v-tooltip="'Copy group ID'" aria-label="Copy group ID" @click="copyGroupId">
						<ClipboardCopyIcon aria-hidden="true" />
					</button>
				</ButtonStyled>
				<ButtonStyled size="large" circular type="transparent">
					<button v-tooltip="'Settings'" aria-label="Settings" @click="emit('open-settings')">
						<SettingsIcon aria-hidden="true" />
					</button>
				</ButtonStyled>
			</template>
		</ContentPageHeader>

		<div class="grid gap-4 lg:grid-cols-3">
			<Card class="lg:col-span-2">
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">About</h2>
				</template>
				<p class="m-0 text-primary">{{ groupDescription }}</p>
			</Card>

			<Card>
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Owner</h2>
				</template>
				<div v-if="owner" class="flex items-center gap-3">
					<Avatar :src="avatarUrl(owner)" :alt="displayName(owner)" size="3rem" circle />
					<div class="min-w-0">
						<p class="m-0 truncate font-semibold text-contrast">{{ displayName(owner) }}</p>
						<p class="m-0 text-sm capitalize text-secondary">{{ owner.role }}</p>
					</div>
				</div>
				<div v-else class="flex items-center gap-3 text-secondary">
					<UserIcon class="size-5" />
					No owner loaded
				</div>
			</Card>

			<Card>
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Invites</h2>
				</template>
				<p class="m-0 text-primary">
					{{ inviteCount === 0 ? 'No pending invites.' : `${formatNumber(inviteCount)} pending invites.` }}
				</p>
			</Card>

			<Card class="lg:col-span-2">
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Connection</h2>
				</template>
				<div class="flex flex-col gap-2 text-sm">
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Address</span>
						<span class="break-all text-right font-semibold text-contrast">
							{{ core?.connectionUrl || 'No Core connection saved' }}
						</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Public subdomain</span>
						<span class="break-all text-right font-semibold text-contrast">
							{{ group.subdomain || 'Not set' }}
						</span>
					</div>
				</div>
			</Card>
		</div>
	</div>

	<div v-else-if="connectedCore" class="flex flex-col gap-4">
		<ContentPageHeader>
			<template #icon>
				<div
					class="flex size-24 shrink-0 items-center justify-center rounded-lg border border-solid border-divider bg-surface-3"
				>
					<ServerStackIcon class="size-12 text-secondary" />
				</div>
			</template>
			<template #title>Core linked</template>
			<template #summary>
				Friend group details are not available for this linked Core yet.
			</template>
			<template #stats>
				<div class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold">
					<ServerStackIcon class="h-6 w-6 text-secondary" />
					{{ statusLabel }}
				</div>
				<div v-tooltip="formatDateTime(linkedAt)" class="flex items-center gap-2 font-semibold">
					<CalendarIcon class="h-6 w-6 text-secondary" />
					Linked {{ formatRelativeTime(linkedAt) }}
				</div>
			</template>
			<template #actions>
				<ButtonStyled size="large" circular type="transparent">
					<button v-tooltip="'Settings'" aria-label="Settings" @click="emit('open-settings')">
						<SettingsIcon aria-hidden="true" />
					</button>
				</ButtonStyled>
			</template>
		</ContentPageHeader>

		<div class="grid gap-4 lg:grid-cols-3">
			<Card>
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Core status</h2>
				</template>
				<span class="flex items-center gap-2 font-semibold text-contrast">
					<span class="size-2.5 rounded-full" :class="statusClass" aria-hidden="true" />
					{{ statusLabel }}
				</span>
			</Card>

			<Card class="lg:col-span-2">
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Connection</h2>
				</template>
				<div class="flex flex-col gap-2 text-sm">
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Core ID</span>
						<span class="break-all text-right font-semibold text-contrast">
							{{ coreId }}
						</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Address</span>
						<span class="break-all text-right font-semibold text-contrast">
							{{ coreUrl }}
						</span>
					</div>
				</div>
			</Card>
		</div>
	</div>

</template>
