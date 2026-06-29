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
			<div class="flex shrink-0 flex-wrap items-center gap-2 md:flex-nowrap">
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

		<div v-if="accessError" class="rounded-lg bg-red-highlight p-4 font-semibold text-contrast">
			Failed to load access: {{ accessError.message }}
		</div>
		<div
			v-else-if="accessLoading"
			class="rounded-2xl border border-solid border-surface-4 bg-surface-2 px-4 py-8 text-center text-secondary"
		>
			Loading access...
		</div>
		<AccessTable
			v-else
			:members="filteredMembers"
			:roles="roleOptions"
			:can-manage-users="canManageUsers"
			:user-profile-link="props.userProfileLink"
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
				:show-world-column="showAuditLogInstances"
				@load-more="loadMoreActivity"
			/>
		</div>

		<GrantAccessModal
			ref="grantAccessModal"
			:members="members"
			:suggestions="suggestions"
			:search-users="searchUsers"
			:can-grant="canManageUsers"
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
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { type Component, computed, ref } from 'vue'

import AccessTable from '#ui/components/servers/access/AccessTable.vue'
import AuditLogTable from '#ui/components/servers/access/AuditLogTable.vue'
import GrantAccessModal from '#ui/components/servers/access/GrantAccessModal.vue'
import RemoveAccessModal from '#ui/components/servers/access/RemoveAccessModal.vue'
import Combobox, { type ComboboxOption } from '#ui/components/base/Combobox.vue'
import type {
	TimeFrameLastUnit,
	TimeFrameMode,
	TimeFramePreset,
} from '#ui/components/base/TimeFramePicker.vue'
import type {
	GrantServerAccessPayload,
	ServerAccessInviteSuggestion,
	ServerAccessMember,
	ServerAccessUserProfileLink,
	ServerAuditLogEntry,
} from '#ui/components/servers/access/types'
import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import { injectHostingBackend } from '#ui/providers'

import CoreActivityEvent from '../../CoreActivityEvent.vue'
import { injectCoreServerContext } from '../../context'

const props = withDefaults(
	defineProps<{
		showAuditLogInstances?: boolean
		userProfileLink?: (username: string) => ServerAccessUserProfileLink
	}>(),
	{
		showAuditLogInstances: false,
		userProfileLink: undefined,
	},
)

type RoleFilter = AmberiteAccessUiRole | 'all'

const backend = injectHostingBackend()
const core = backend.core
const queryClient = useQueryClient()
const ctx = injectCoreServerContext()
const search = ref('')
const grantAccessModal = ref<InstanceType<typeof GrantAccessModal>>()
const removeAccessModal = ref<InstanceType<typeof RemoveAccessModal>>()
const pendingRemoval = ref<ServerAccessMember>()
const activityCursor = ref<string | null>(null)
const activityEntries = ref<CoreActivityLogEntry[]>([])
const activityLoadingMore = ref(false)
const auditLogSortDirection = ref<'asc' | 'desc'>('desc')
const auditLogTimeframeMode = ref<TimeFrameMode>('preset')
const auditLogTimeframePreset = ref<TimeFramePreset>('all_time')
const auditLogTimeframeLastAmount = ref(7)
const auditLogTimeframeLastUnit = ref<TimeFrameLastUnit>('days')
const auditLogTimeframeCustomStartDate = ref('')
const auditLogTimeframeCustomEndDate = ref('')
const roleOptions = amberiteAccessRoleOptions

const accessQuery = useQuery({
	queryKey: computed(() => ['core-access', ctx.instanceId.value]),
	queryFn: () => core.listInstanceAccess(ctx.instanceId.value),
	staleTime: 15_000,
	placeholderData: (previous) => previous,
})

const activityQuery = useQuery({
	queryKey: computed(() => ['core-activity', ctx.instanceId.value]),
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
const accessError = computed(() => (accessQuery.error.value as Error | null) ?? null)
const accessLoading = computed(
	() => accessQuery.isLoading.value && accessQuery.data.value === undefined && !accessError.value,
)
const canManageUsers = computed(() => accessQuery.data.value?.viewer.can_manage_users ?? false)
const activityLoading = computed(() => activityQuery.isLoading.value)
const roleFilter = ref<RoleFilter>('all')
const roleFilterOptions = computed<ComboboxOption<RoleFilter>[]>(() => [
	{ value: 'all', label: 'All roles' },
	...roleOptions.map((role) => ({ value: role.value, label: role.label })),
])
const selectedRoleFilterLabel = computed(
	() => roleFilterOptions.value.find((option) => option.value === roleFilter.value)?.label ?? 'All roles',
)
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

async function refresh() {
	activityCursor.value = null
	await Promise.all([
		queryClient.invalidateQueries({ queryKey: ['core-access', ctx.instanceId.value] }),
		queryClient.invalidateQueries({ queryKey: ['core-activity', ctx.instanceId.value] }),
	])
}

async function searchUsers(query: string) {
	return backend.searchUsers(query)
}

async function grantAccess(payload: GrantServerAccessPayload) {
	await core.grantInstanceAccess(ctx.instanceId.value, {
		user_id: payload.user.id,
		display_name: payload.user.username,
		role: uiAccessRoleToCore(payload.role as AmberiteAccessUiRole),
		permission_preset: uiAccessRoleToPreset(payload.role as AmberiteAccessUiRole),
	})
	await refresh()
}

async function updateRole(member: ServerAccessMember, role: AmberiteAccessUiRole) {
	if (member.isOwner || role === 'owner') return
	await core.updateInstanceAccess(ctx.instanceId.value, member.user.id, {
		role: uiAccessRoleToCore(role),
		permission_preset: uiAccessRoleToPreset(role),
	})
	await refresh()
}

function requestRemove(member: ServerAccessMember) {
	pendingRemoval.value = member
	removeAccessModal.value?.show()
}

async function confirmRemove() {
	if (!pendingRemoval.value) return
	await core.removeInstanceAccess(ctx.instanceId.value, pendingRemoval.value.user.id)
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
	return core.listInstanceActivity(ctx.instanceId.value, query)
}
</script>
