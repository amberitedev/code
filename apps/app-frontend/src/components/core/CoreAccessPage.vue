<template>
	<div class="flex w-full flex-col gap-4">
		<div class="flex flex-col gap-2 md:flex-row">
			<StyledInput
				v-model="search"
				:icon="SearchIcon"
				:placeholder="`Search ${members.length} members...`"
				wrapper-class="min-w-0 flex-1"
				input-class="!h-10"
				clearable
			/>
			<div class="flex shrink-0 items-center gap-2 flex-wrap md:flex-nowrap">
				<Combobox
					v-model="roleFilter"
					:options="roleFilterOptions"
					:display-value="selectedRoleFilterLabel"
					trigger-class="min-w-[225px] !h-10 !min-h-10 !py-0"
				>
					<template #prefix>
						<FilterIcon class="size-5 text-secondary" aria-hidden="true" />
					</template>
				</Combobox>
				<ButtonStyled color="brand">
					<button
						class="!h-10 w-full md:w-fit"
						:disabled="!canManageUsers"
						@click="grantAccessModal?.show($event)"
					>
						<UserPlusIcon aria-hidden="true" />
						Add user
					</button>
				</ButtonStyled>
			</div>
		</div>

		<AccessTable
			:members="filteredMembers"
			:roles="roleOptions"
			:can-manage-users="canManageUsers"
			permission-denied-message="You do not have permission to manage access."
			@update-role="updateRole"
			@cancel-invite="requestRemove"
			@remove-member="requestRemove"
		/>

		<div class="flex flex-col gap-4">
			<span class="m-0 text-2xl font-semibold text-contrast">Activity log</span>
			<AuditLogTable
				v-model:sort-direction="auditLogSortDirection"
				v-model:timeframe-mode="auditLogTimeframeMode"
				v-model:timeframe-preset="auditLogTimeframePreset"
				v-model:timeframe-last-amount="auditLogTimeframeLastAmount"
				v-model:timeframe-last-unit="auditLogTimeframeLastUnit"
				v-model:timeframe-custom-start-date="auditLogTimeframeCustomStartDate"
				v-model:timeframe-custom-end-date="auditLogTimeframeCustomEndDate"
				:entries="auditEntries"
				:has-more="hasMoreActivity"
				:loading="activityLoading"
				:loading-more="activityLoadingMore"
				:show-world-column="!instanceId"
				@load-more="loadMoreActivity"
			/>
		</div>

		<GrantAccessModal
			ref="grantAccessModal"
			:members="members"
			:suggestions="suggestions"
			:friend-ids="friendIds"
			:friend-request-unavailable-ids="pendingFriendRequestIds"
			:search-users="searchUsers"
			:can-grant="canManageUsers"
			target-label="Username"
			target-placeholder="Search Amberite or Minecraft username"
			target-help="Use their unique Amberite username or their Minecraft username."
			@grant="grantAccess"
		/>
		<RemoveAccessModal
			ref="removeAccessModal"
			:username="pendingRemoval?.user.username ?? ''"
			:avatar-url="pendingRemoval?.user.avatarUrl"
			:role="pendingRemoval?.role"
			:joined-at="pendingRemoval?.joinedAt"
			:pending="pendingRemoval?.pending"
			@remove="confirmRemove"
		/>
	</div>
</template>

<script setup lang="ts">
import type {
	AmberiteAccessUiRole,
	CoreActivityLogEntry,
	CoreActivityLogQuery,
} from '@amberite/amberite-api'
import {
	amberiteAccessRoleOptions,
	formatActivityAction,
	toAmberiteAccessUiMember,
	uiAccessRoleToCore,
	uiAccessRoleToPreset,
} from '@amberite/amberite-api'
import { FilterIcon, SearchIcon, UserPlusIcon } from '@modrinth/assets'
import {
	AccessTable,
	AuditLogTable,
	ButtonStyled,
	Combobox,
	GrantAccessModal,
	injectNotificationManager,
	type GrantServerAccessPayload,
	RemoveAccessModal,
	type ServerAccessInviteSuggestion,
	type ServerAccessMember,
	type ServerAuditLogEntry,
	StyledInput,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { type Component, computed, ref, watch } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'

import CoreActivityEvent from './CoreActivityEvent.vue'

const props = defineProps<{
	instanceId?: string
}>()

const core = useCoreClient()
const social = useSocialClient()
const socialState = useSocial()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const search = ref('')
const roleFilter = ref<AmberiteAccessUiRole | 'all'>('all')
const grantAccessModal = ref<InstanceType<typeof GrantAccessModal>>()
const removeAccessModal = ref<InstanceType<typeof RemoveAccessModal>>()
const pendingRemoval = ref<ServerAccessMember>()
const activityCursor = ref<string | null>(null)
const activityEntries = ref<CoreActivityLogEntry[]>([])
const activityLoadingMore = ref(false)
const auditLogSortDirection = ref<'asc' | 'desc'>('desc')
const auditLogTimeframeMode = ref<'preset' | 'last' | 'custom_range' | 'custom_datetime_range'>(
	'preset',
)
const auditLogTimeframePreset = ref<
	| 'today'
	| 'yesterday'
	| 'last_7_days'
	| 'last_14_days'
	| 'last_30_days'
	| 'last_90_days'
	| 'last_180_days'
	| 'year_to_date'
	| 'all_time'
>('all_time')
const auditLogTimeframeLastAmount = ref(7)
const auditLogTimeframeLastUnit = ref<'hours' | 'days' | 'weeks' | 'months'>('days')
const auditLogTimeframeCustomStartDate = ref('')
const auditLogTimeframeCustomEndDate = ref('')

const roleOptions = amberiteAccessRoleOptions
const roleFilterOptions = [
	{ value: 'all', label: 'All roles' },
	...roleOptions.map((role) => ({ value: role.value, label: role.label })),
]
const selectedRoleFilterLabel = computed(
	() => roleFilterOptions.find((option) => option.value === roleFilter.value)?.label ?? 'All roles',
)

const accessQuery = useQuery({
	queryKey: computed(() => ['core-access', props.instanceId ?? 'core']),
	queryFn: () =>
		props.instanceId ? core.listInstanceAccess(props.instanceId) : core.listCoreAccess(),
	staleTime: 15_000,
	placeholderData: (previous) => previous,
})

const activityQuery = useQuery({
	queryKey: computed(() => ['core-activity', props.instanceId ?? 'core']),
	queryFn: async () => {
		const page = await fetchActivityPage(null)
		activityEntries.value = page.entries
		activityCursor.value = page.next_cursor ?? null
		return page
	},
	staleTime: 15_000,
	placeholderData: (previous) => previous,
})

const members = computed<ServerAccessMember[]>(() =>
	(accessQuery.data.value?.members ?? []).map(toAmberiteAccessUiMember),
)
const canManageUsers = computed(() => accessQuery.data.value?.viewer.can_manage_users ?? false)
const activityLoading = computed(() => activityQuery.isLoading.value)
const filteredMembers = computed(() => {
	const normalized = search.value.trim().toLowerCase()
	return members.value.filter((member) => {
		if (roleFilter.value !== 'all' && member.role !== roleFilter.value) return false
		return !normalized || member.user.username.toLowerCase().includes(normalized)
	})
})
const suggestions = computed<ServerAccessInviteSuggestion[]>(() =>
	members.value.map((member) => ({
		id: member.user.id,
		username: member.user.username,
		avatarUrl: member.user.avatarUrl,
	})),
)
const friendIds = computed(() =>
	(socialState.friends.value?.friends ?? [])
		.map((friend) => friend.user?.userId)
		.filter((id): id is string => !!id),
)
const pendingFriendRequestIds = computed(() => [
	...(socialState.friends.value?.incoming ?? []).map((request) => request.request.fromUserId),
	...(socialState.friends.value?.outgoing ?? []).map((request) => request.request.toUserId),
])
const auditEntries = computed<ServerAuditLogEntry[]>(() =>
	activityEntries.value.map((entry) => ({
		id: entry.id,
		actor: { id: entry.actor_user_id, username: entry.actor_user_id },
		world: entry.instance_id ? { id: entry.instance_id, name: entry.instance_id } : null,
		event: {
			key: entry.action,
			component: CoreActivityEvent as Component,
			props: {
				action: entry.action,
				timestamp: entry.created_at,
				actor: { id: entry.actor_user_id, username: entry.actor_user_id },
				world: entry.instance_id ? { id: entry.instance_id, name: entry.instance_id } : null,
				label: formatActivityAction(entry.action),
				details: entry.target_user_id ? `Target: ${entry.target_user_id}` : undefined,
			},
			searchText: `${entry.action} ${entry.actor_user_id} ${entry.target_user_id ?? ''}`,
		},
		timestamp: entry.created_at,
	})),
)
const hasMoreActivity = computed(() => !!activityCursor.value)

watch(
	() => accessQuery.error.value,
	(error, previous) => {
		if (!error || error === previous) return
		addNotification({
			type: 'error',
			title: 'Failed to load access',
			text: error instanceof Error ? error.message : String(error),
		})
	},
)

async function refresh() {
	activityCursor.value = null
	await Promise.all([
		queryClient.invalidateQueries({ queryKey: ['core-access', props.instanceId ?? 'core'] }),
		queryClient.invalidateQueries({ queryKey: ['core-activity', props.instanceId ?? 'core'] }),
	])
}

async function searchUsers(query: string) {
	const users = await social.searchUsers(query).catch(() => [])
	return users.map((user) => ({
		id: user.userId,
		username: user.displayName ?? user.username ?? user.userId,
		avatarUrl: user.image,
	}))
}

async function grantAccess(payload: GrantServerAccessPayload) {
	const body = {
		user_id: payload.user.id,
		display_name: payload.user.username,
		role: uiAccessRoleToCore(payload.role),
		permission_preset: uiAccessRoleToPreset(payload.role),
	}
	if (props.instanceId) await core.grantInstanceAccess(props.instanceId, body)
	else await core.grantCoreAccess(body)
	await refresh()
}

async function updateRole(member: ServerAccessMember, role: AmberiteAccessUiRole) {
	if (member.isOwner || role === 'owner') return
	const body = {
		role: uiAccessRoleToCore(role),
		permission_preset: uiAccessRoleToPreset(role),
	}
	if (props.instanceId) await core.updateInstanceAccess(props.instanceId, member.user.id, body)
	else await core.updateCoreAccess(member.user.id, body)
	await refresh()
}

function requestRemove(member: ServerAccessMember) {
	pendingRemoval.value = member
	removeAccessModal.value?.show()
}

async function confirmRemove() {
	if (!pendingRemoval.value) return
	if (props.instanceId)
		await core.removeInstanceAccess(props.instanceId, pendingRemoval.value.user.id)
	else await core.removeCoreAccess(pendingRemoval.value.user.id)
	pendingRemoval.value = undefined
	await refresh()
}

async function loadMoreActivity() {
	if (!activityCursor.value || activityLoadingMore.value) return
	activityLoadingMore.value = true
	try {
		const page = await fetchActivityPage(activityCursor.value)
		activityEntries.value = [...activityEntries.value, ...page.entries]
		activityCursor.value = page.next_cursor ?? null
	} finally {
		activityLoadingMore.value = false
	}
}

function fetchActivityPage(cursor: string | null) {
	const query: CoreActivityLogQuery = { limit: 50, cursor }
	return props.instanceId
		? core.listInstanceActivity(props.instanceId, query)
		: core.listActivity(query)
}
</script>
