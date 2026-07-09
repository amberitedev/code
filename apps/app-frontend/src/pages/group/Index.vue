<script setup lang="ts">
import type { FriendGroupMember, FriendGroupPublicProfile } from '@amberite/amberite-api'
import {
	CalendarIcon,
	ClipboardCopyIcon,
	CrownIcon,
	LinkIcon,
	LogOutIcon,
	MoreVerticalIcon,
	ServerStackIcon,
	SettingsIcon,
	ShieldIcon,
	UserPlusIcon,
	UsersIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	Card,
	commonMessages,
	ContentPageHeader,
	EmptyState,
	injectNotificationManager,
	LoadingIndicator,
	OverflowMenu,
	SidebarCard,
	useCompactNumber,
	useFormatDateTime,
	useFormatNumber,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useSocialClient } from '@/composables/useSocialClient'
import { useBreadcrumbs } from '@/store/breadcrumbs'

const roleOrder = {
	owner: 0,
	admin: 1,
	member: 2,
}

const social = useSocialClient()
const queryClient = useQueryClient()
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const notifications = injectNotificationManager()
const { formatMessage } = useVIntl()
const formatNumber = useFormatNumber()
const { formatCompactNumber } = useCompactNumber()
const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({
	timeStyle: 'short',
	dateStyle: 'long',
})

const actionPending = ref<string | null>(null)
const notifiedError = ref<unknown>(null)

const routeGroup = computed(() => String(route.params.group ?? ''))
const now = Date.now()
const mockMembers: FriendGroupMember[] = [
	mockMember('mock-owner', 'owner', 'Ari', 'ari'),
	mockMember('mock-admin', 'admin', 'Noa', 'noa'),
	mockMember('mock-member', 'member', 'Mika', 'mika'),
	mockMember('mock-member-2', 'member', 'Devon', 'devon'),
]
const mockProfile = computed<FriendGroupPublicProfile | null>(() => {
	if (routeGroup.value !== 'mock' && routeGroup.value !== 'mock-public') return null

	const isPublicPreview = routeGroup.value === 'mock-public'
	const groupId = isPublicPreview ? 'mock-public-group' : 'mock-group'
	return {
		group: {
			_id: groupId,
			id: groupId,
			name: isPublicPreview ? 'Public Preview Group' : 'Amberite Mock Group',
			description:
				'A temporary profile preview with a banner, members, ownership, Core status, and overflow actions.',
			banner: 'https://cdn.modrinth.com/data/AANobbMI/images/6c212c3f628d1267c5d1773697df661dd5395784.png',
			subdomain: routeGroup.value,
			coreId: 'mock-core',
			ownerUserId: 'mock-owner',
			createdAt: now - 1000 * 60 * 60 * 24 * 45,
			updatedAt: now - 1000 * 60 * 60 * 6,
		},
		core: {
			coreId: 'mock-core',
			ownerUserId: 'mock-owner',
			friendGroupId: groupId,
			connectionUrl: 'http://localhost:16662',
			lastSeenAt: now - 1000 * 60 * 8,
			status: 'paired',
			name: 'Mock Core',
			subdomain: routeGroup.value,
			setupMode: 'local',
		},
		members: mockMembers,
		viewer: {
			isMember: !isPublicPreview,
			role: isPublicPreview ? null : 'admin',
			permissionPreset: isPublicPreview ? null : 'admin',
			canManage: !isPublicPreview,
			isOwner: false,
			banned: false,
		},
		actions: {
			manage: !isPublicPreview,
			leave: !isPublicPreview,
			createInvite: !isPublicPreview,
		},
	}
})

const groupQuery = useQuery({
	queryKey: computed(() => ['group-profile', routeGroup.value]),
	queryFn: () => social.getPublicFriendGroupProfile(routeGroup.value),
	enabled: () => routeGroup.value.length > 0 && !mockProfile.value,
	retry: false,
})

const profile = computed(() => mockProfile.value ?? groupQuery.data.value ?? null)
const group = computed(() => profile.value?.group ?? null)
const core = computed(() => profile.value?.core ?? null)
const members = computed(() => profile.value?.members ?? [])
const viewer = computed(() => profile.value?.viewer ?? null)
const actions = computed(() => profile.value?.actions ?? {})
const loadError = computed(() => (mockProfile.value ? null : groupQuery.error.value))
const isLoading = computed(
	() => !mockProfile.value && (groupQuery.isLoading.value || groupQuery.isFetching.value),
)
const groupName = computed(() => group.value?.name || 'Friend group')
const groupDescription = computed(
	() => group.value?.description || 'A shared Amberite group for managing Core access and servers.',
)
const createdAt = computed(() => new Date(group.value?.createdAt ?? Date.now()))
const owner = computed(() => sortedMembers.value.find((member) => member.role === 'owner') ?? null)
const groupHandle = computed(() => group.value?.subdomain || group.value?.id || routeGroup.value)
const groupUrlPath = computed(() => `/group/${encodeURIComponent(groupHandle.value)}`)
const statusLabel = computed(() => {
	if (!core.value) return 'No Core'
	if (!core.value.lastSeenAt) return 'Unknown'
	return 'Linked Core'
})
const statusTooltip = computed(() => {
	if (!core.value) return 'This group does not have a linked Core.'
	if (!core.value.lastSeenAt) return 'Core status has not been reported yet.'
	return `Last seen ${formatDateTime(new Date(core.value.lastSeenAt))}`
})
const joinTooltip = computed(() =>
	viewer.value?.banned ? 'You cannot join this group.' : 'Joining requires an invite.',
)
const leaveTooltip = computed(() =>
	viewer.value?.isOwner ? 'Transfer ownership before leaving this group.' : undefined,
)
const sortedMembers = computed(() =>
	[...members.value].sort((first, second) => {
		const roleDiff = roleOrder[first.role] - roleOrder[second.role]
		if (roleDiff !== 0) return roleDiff
		return displayName(first).localeCompare(displayName(second))
	}),
)
const overflowOptions = computed(() => [
	{
		id: 'manage',
		shown: !!actions.value.manage,
		action: () => manageGroup(),
	},
	{
		id: 'copy-link',
		action: () => copyGroupLink(),
	},
	{
		id: 'copy-id',
		action: () => copyGroupId(),
	},
	{
		id: 'copy-invite-code',
		shown: !!actions.value.createInvite,
		disabled: actionPending.value === 'invite',
		action: () => copyInviteCode(),
	},
	{
		divider: true,
	},
	{
		id: 'join',
		shown: !viewer.value?.isMember,
		disabled: true,
		tooltip: joinTooltip.value,
	},
	{
		id: 'leave',
		shown: !!viewer.value?.isMember,
		disabled: viewer.value?.isOwner || actionPending.value === 'leave',
		tooltip: leaveTooltip.value,
		color: 'red' as const,
		hoverFilled: true,
		action: () => leaveGroup(),
	},
])

watch(
	groupName,
	(value) => {
		breadcrumbs.setName('Group', value)
	},
	{ immediate: true },
)

watch(
	loadError,
	(error) => {
		if (!error || notifiedError.value === error) return
		notifiedError.value = error
		notifications.addNotification({
			type: 'error',
			title: 'Failed to load group',
			text: getErrorMessage(error),
		})
	},
)

function displayName(member: FriendGroupMember) {
	return (
		member.user?.displayName ||
		member.user?.name ||
		member.user?.username ||
		member.userId
	)
}

function mockMember(
	userId: string,
	role: FriendGroupMember['role'],
	displayNameValue: string,
	username: string,
): FriendGroupMember {
	return {
		_id: `${userId}-membership`,
		friendGroupId: 'mock-group',
		userId,
		role,
		permissionPreset: role,
		createdAt: now - 1000 * 60 * 60 * 24 * (role === 'owner' ? 45 : 20),
		updatedAt: now - 1000 * 60 * 60 * 3,
		user: {
			id: userId,
			userId,
			username,
			displayName: displayNameValue,
			name: displayNameValue,
			avatar_url: null,
			bio: null,
			created: new Date(now - 1000 * 60 * 60 * 24 * 90).toISOString(),
		},
	}
}

function avatarUrl(member: FriendGroupMember) {
	return member.user?.avatar_url || member.user?.image
}

function memberRoute(member: FriendGroupMember) {
	return `/user/${encodeURIComponent(member.user?.username ?? member.userId)}`
}

function roleLabel(role: FriendGroupMember['role']) {
	return role.charAt(0).toUpperCase() + role.slice(1)
}

function manageGroup() {
	void router.push('/core')
}

async function copyGroupLink() {
	await navigator.clipboard.writeText(`${window.location.origin}${groupUrlPath.value}`)
	notifications.addNotification({
		type: 'success',
		title: 'Copied group link',
	})
}

async function copyGroupId() {
	if (!group.value) return
	await navigator.clipboard.writeText(group.value.id)
	notifications.addNotification({
		type: 'success',
		title: 'Copied group ID',
	})
}

async function copyInviteCode() {
	if (!group.value || actionPending.value) return
	actionPending.value = 'invite'
	try {
		if (mockProfile.value) {
			await navigator.clipboard.writeText('MOCK1234')
			notifications.addNotification({
				type: 'success',
				title: 'Copied mock invite code',
				text: 'MOCK1234 was copied to your clipboard.',
			})
			return
		}

		const invite = await social.createFriendGroupInvite({ friendGroupId: group.value.id })
		if (invite.code) {
			await navigator.clipboard.writeText(invite.code)
		}
		notifications.addNotification({
			type: 'success',
			title: invite.code ? 'Copied invite code' : 'Created invite',
			text: invite.code ? 'The invite code was copied to your clipboard.' : undefined,
		})
	} catch (error) {
		notifications.addNotification({
			type: 'error',
			title: 'Failed to create invite',
			text: getErrorMessage(error),
		})
	} finally {
		actionPending.value = null
	}
}

async function leaveGroup() {
	if (!group.value || viewer.value?.isOwner || actionPending.value) return
	actionPending.value = 'leave'
	try {
		if (mockProfile.value) {
			notifications.addNotification({
				type: 'success',
				title: 'Mock leave action',
				text: 'This temporary preview did not change any data.',
			})
			return
		}

		await social.leaveGroup(group.value.id)
		await queryClient.invalidateQueries({ queryKey: ['group-profile', routeGroup.value] })
		notifications.addNotification({
			type: 'success',
			title: 'Left group',
		})
	} catch (error) {
		notifications.addNotification({
			type: 'error',
			title: 'Failed to leave group',
			text: getErrorMessage(error),
		})
	} finally {
		actionPending.value = null
	}
}

function getErrorMessage(error: unknown) {
	if (error instanceof Error && error.message) return error.message
	if (typeof error === 'string') return error
	return 'The group could not be loaded.'
}
</script>

<template>
	<Teleport v-if="profile" defer to="#sidebar-teleport-target">
		<div class="flex flex-col gap-4">
			<SidebarCard title="Members">
				<div class="flex flex-col gap-3 font-semibold">
					<RouterLink
						v-for="member in sortedMembers"
						:key="member._id"
						class="group flex w-fit max-w-full items-center gap-2 leading-[1.2] text-primary no-underline"
						:to="memberRoute(member)"
					>
						<Avatar
							:src="avatarUrl(member)"
							:alt="displayName(member)"
							size="32px"
							circle
						/>
						<div class="flex min-w-0 flex-col">
							<span class="flex w-full flex-nowrap items-center gap-1 group-hover:underline">
								<span class="min-w-0 overflow-hidden truncate">{{ displayName(member) }}</span>
								<CrownIcon
									v-if="member.role === 'owner'"
									v-tooltip="'Group owner'"
									class="text-brand-orange"
								/>
							</span>
							<span class="text-sm font-normal text-secondary">
								{{ roleLabel(member.role) }}
							</span>
						</div>
					</RouterLink>
				</div>
			</SidebarCard>

			<Card>
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">Group info</h2>
				</template>
				<div class="flex flex-col gap-3 text-sm">
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Your role</span>
						<span class="font-semibold text-contrast">
							{{ viewer?.role ? roleLabel(viewer.role) : 'Visitor' }}
						</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Public name</span>
						<span class="break-all text-right font-semibold text-contrast">
							{{ group?.subdomain || 'Not set' }}
						</span>
					</div>
					<div class="flex items-start justify-between gap-4">
						<span class="text-secondary">Group ID</span>
						<button
							class="m-0 cursor-pointer border-0 bg-transparent p-0 text-right text-sm font-semibold text-contrast hover:underline"
							@click="copyGroupId"
						>
							{{ group?.id }}
						</button>
					</div>
				</div>
			</Card>
		</div>
	</Teleport>

	<div class="box-border flex min-h-full flex-col gap-4 p-6">
		<div v-if="profile && group" class="flex flex-col gap-4">
			<div
				v-if="group.banner"
				class="h-36 overflow-hidden rounded-xl border border-solid border-surface-4 bg-surface-3 md:h-44"
			>
				<img :src="group.banner" :alt="`${groupName} banner`" class="h-full w-full object-cover" />
			</div>

			<ContentPageHeader>
				<template #icon>
					<Avatar :src="group.banner" :alt="groupName" size="96px" />
				</template>
				<template #title>
					{{ groupName }}
				</template>
				<template #title-suffix>
					<div class="ml-1 flex items-center gap-2 font-semibold text-secondary">
						<UsersIcon class="size-5" aria-hidden="true" />
						Group
					</div>
				</template>
				<template #summary>
					{{ groupDescription }}
				</template>
				<template #stats>
					<div class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold">
						<UsersIcon class="h-6 w-6 text-secondary" aria-hidden="true" />
						{{ formatCompactNumber(members.length) }}
						{{ members.length === 1 ? 'member' : 'members' }}
					</div>
					<div
						v-tooltip="statusTooltip"
						class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold"
					>
						<ServerStackIcon class="h-6 w-6 text-secondary" aria-hidden="true" />
						{{ statusLabel }}
					</div>
					<div v-tooltip="formatDateTime(createdAt)" class="flex items-center gap-2 font-semibold">
						<CalendarIcon class="h-6 w-6 text-secondary" aria-hidden="true" />
						Created {{ formatRelativeTime(createdAt) }}
					</div>
				</template>
				<template #actions>
					<ButtonStyled v-if="actions.manage" size="large">
						<button @click="manageGroup">
							<SettingsIcon aria-hidden="true" />
							Manage
						</button>
					</ButtonStyled>
					<ButtonStyled v-else-if="!viewer?.isMember" size="large" type="outlined">
						<button v-tooltip="joinTooltip" disabled>
							<UserPlusIcon aria-hidden="true" />
							Join
						</button>
					</ButtonStyled>
					<ButtonStyled size="large" circular type="transparent">
						<OverflowMenu
							:options="overflowOptions"
							:tooltip="`More options`"
							aria-label="More options"
						>
							<MoreVerticalIcon aria-hidden="true" />
							<template #manage>
								<SettingsIcon aria-hidden="true" />
								Manage
							</template>
							<template #copy-link>
								<LinkIcon aria-hidden="true" />
								Copy link
							</template>
							<template #copy-id>
								<ClipboardCopyIcon aria-hidden="true" />
								{{ formatMessage(commonMessages.copyIdButton) }}
							</template>
							<template #copy-invite-code>
								<UserPlusIcon aria-hidden="true" />
								{{ actionPending === 'invite' ? 'Creating invite...' : 'Copy invite code' }}
							</template>
							<template #join>
								<UserPlusIcon aria-hidden="true" />
								Join
							</template>
							<template #leave>
								<LogOutIcon aria-hidden="true" />
								Leave group
							</template>
						</OverflowMenu>
					</ButtonStyled>
				</template>
			</ContentPageHeader>

			<div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(18rem,24rem)]">
				<Card>
					<template #header>
						<h2 class="m-0 text-lg font-semibold text-contrast">About</h2>
					</template>
					<p class="m-0 text-primary">{{ groupDescription }}</p>
				</Card>

				<Card>
					<template #header>
						<h2 class="m-0 text-lg font-semibold text-contrast">Owner</h2>
					</template>
					<RouterLink
						v-if="owner"
						:to="memberRoute(owner)"
						class="flex min-w-0 items-center gap-3 text-primary no-underline"
					>
						<Avatar :src="avatarUrl(owner)" :alt="displayName(owner)" size="3rem" circle />
						<div class="min-w-0">
							<p class="m-0 truncate font-semibold text-contrast">{{ displayName(owner) }}</p>
							<p class="m-0 text-sm text-secondary">{{ roleLabel(owner.role) }}</p>
						</div>
					</RouterLink>
					<div v-else class="flex items-center gap-2 text-secondary">
						<ShieldIcon class="size-5" aria-hidden="true" />
						No owner loaded
					</div>
				</Card>

				<Card>
					<template #header>
						<h2 class="m-0 text-lg font-semibold text-contrast">Membership</h2>
					</template>
					<div class="flex flex-col gap-2 text-sm">
						<div class="flex items-start justify-between gap-4">
							<span class="text-secondary">Visible members</span>
							<span class="font-semibold text-contrast">{{ formatNumber(members.length) }}</span>
						</div>
						<div class="flex items-start justify-between gap-4">
							<span class="text-secondary">Your access</span>
							<span class="font-semibold text-contrast">
								{{ viewer?.role ? roleLabel(viewer.role) : viewer?.banned ? 'Banned' : 'Invite required' }}
							</span>
						</div>
					</div>
				</Card>

				<Card>
					<template #header>
						<h2 class="m-0 text-lg font-semibold text-contrast">Linked Core</h2>
					</template>
					<div class="flex flex-col gap-2 text-sm">
						<div class="flex items-start justify-between gap-4">
							<span class="text-secondary">Status</span>
							<span class="font-semibold text-contrast">{{ statusLabel }}</span>
						</div>
						<div class="flex items-start justify-between gap-4">
							<span class="text-secondary">Core ID</span>
							<span class="break-all text-right font-semibold text-contrast">
								{{ core?.coreId || group.coreId || 'Not linked' }}
							</span>
						</div>
						<div class="flex items-start justify-between gap-4">
							<span class="text-secondary">Public name</span>
							<span class="break-all text-right font-semibold text-contrast">
								{{ group.subdomain || core?.subdomain || 'Not set' }}
							</span>
						</div>
					</div>
				</Card>
			</div>
		</div>

		<div v-else-if="isLoading" class="flex min-h-[24rem] items-center justify-center">
			<LoadingIndicator />
		</div>

		<EmptyState
			v-else
			type="error"
			heading="Group not found"
			:description="getErrorMessage(loadError)"
		/>
	</div>
</template>
