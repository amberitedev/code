<script setup lang="ts">
import { formatActivityAction } from '@amberite/amberite-api'
import type { CoreActivityLogEntry, CoreActivityLogQuery } from '@modrinth/api-client'
import type {
	EventEntity,
	ServerAuditLogEntry,
	TimeFrameLastUnit,
	TimeFrameMode,
	TimeFramePreset,
} from '@modrinth/ui'
import { AuditLogTable, DropdownFilterBar, UserAccessEvent } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { type Component, computed, ref, watch } from 'vue'

import { coreActivityCache } from '@/components/core/core-panel-cache'
import CoreActivityEvent from '@/components/core/CoreActivityEvent.vue'
import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { useConnectedCore } from '@/core/connected-core'

type UserLookup = {
	username: string
	avatarUrl?: string
}
type UserAccessKind = 'invited' | 'invite_revoked' | 'permission_modified' | 'removed'

const userAccessActions: Record<string, UserAccessKind> = {
	user_invited: 'invited',
	user_access_granted: 'invited',
	user_permission_modified: 'permission_modified',
	user_access_updated: 'permission_modified',
	user_invite_revoked: 'invite_revoked',
	user_removed: 'removed',
	user_access_removed: 'removed',
}

const core = useCoreClient()
const social = useSocial()
const connectedCore = useConnectedCore()
const activityCacheKey = computed(() => connectedCore.value?.coreId ?? 'core')
const cachedActivity = coreActivityCache.get(activityCacheKey.value)
const activityCursor = ref<string | null>(cachedActivity?.cursor ?? null)
const activityEntries = ref<CoreActivityLogEntry[]>(cachedActivity?.entries ?? [])
const activityLoadingMore = ref(false)
const auditLogSortDirection = ref<'asc' | 'desc'>('desc')
const auditLogTimeframeMode = ref<TimeFrameMode>('preset')
const auditLogTimeframePreset = ref<TimeFramePreset>('all_time')
const auditLogTimeframeLastAmount = ref(30)
const auditLogTimeframeLastUnit = ref<TimeFrameLastUnit>('days')
const auditLogTimeframeCustomStartDate = ref('')
const auditLogTimeframeCustomEndDate = ref('')
const auditLogFilters = ref<Record<string, string[]>>({
	users: [],
	actions: [],
})

const activityQuery = useQuery({
	queryKey: computed(() => ['core-activity', activityCacheKey.value]),
	queryFn: async () => {
		const requestedCacheKey = activityCacheKey.value
		const page = await fetchActivityPage(null)
		if (requestedCacheKey === activityCacheKey.value) {
			applyInitialActivityPage(requestedCacheKey, page.entries, page.next_cursor ?? null)
		}
		return page
	},
	staleTime: 0,
	refetchOnMount: 'always',
	placeholderData: (previous) => previous,
})

watch(activityCacheKey, applyCachedActivity)

const userLookup = computed(() => {
	const users = new Map<string, UserLookup>()
	addUserLookup(users, social.currentUser.value)
	for (const member of social.members.value) addUserLookup(users, member.user)
	for (const friend of social.friends.value?.friends ?? []) addUserLookup(users, friend.user)
	return users
})
const auditEntries = computed<ServerAuditLogEntry[]>(() => activityEntries.value.map(toAuditEntry))
const activityLoading = computed(
	() => activityQuery.isLoading.value && activityEntries.value.length === 0,
)
const hasMoreActivity = computed(() => !!activityCursor.value)
const activeFilterCount = computed(() =>
	Object.values(auditLogFilters.value).reduce((total, values) => total + values.length, 0),
)
const hasActiveAuditLogFilters = computed(() => activeFilterCount.value > 0)
const filteredEntries = computed(() =>
	auditEntries.value.filter((entry) => {
		const userFilters = auditLogFilters.value.users
		const actionFilters = auditLogFilters.value.actions
		if (userFilters.length > 0 && !userFilters.includes(entry.actor.id)) return false
		if (actionFilters.length > 0 && !actionFilters.includes(entry.event.key)) return false
		return true
	}),
)
const auditLogFilterCategories = computed(() => [
	{
		key: 'users',
		label: 'Users',
		searchable: true,
		searchPlaceholder: 'Search users...',
		options: uniqueOptions(
			auditEntries.value.map((entry) => ({
				value: entry.actor.id,
				label: entry.actor.username,
			})),
		),
	},
	{
		key: 'actions',
		label: 'Actions',
		options: [
			{ value: 'user_invited', label: 'Invited user' },
			{ value: 'user_access_granted', label: 'Granted access' },
			{ value: 'user_permission_modified', label: 'Changed permissions' },
			{ value: 'user_access_updated', label: 'Updated access' },
			{ value: 'user_invite_revoked', label: 'Revoked invite' },
			{ value: 'user_removed', label: 'Removed user' },
			{ value: 'user_access_removed', label: 'Revoked access' },
		],
	},
])

function uniqueOptions(options: Array<{ value: string; label: string }>) {
	const seen = new Set<string>()
	return options.filter((option) => {
		if (seen.has(option.value)) return false
		seen.add(option.value)
		return true
	})
}

async function loadMoreActivity() {
	if (!activityCursor.value || activityLoadingMore.value) return
	activityLoadingMore.value = true
	try {
		const requestedCacheKey = activityCacheKey.value
		const page = await fetchActivityPage(activityCursor.value)
		if (requestedCacheKey === activityCacheKey.value) {
			applyActivitySnapshot(
				requestedCacheKey,
				[...activityEntries.value, ...page.entries],
				page.next_cursor ?? null,
			)
		}
	} finally {
		activityLoadingMore.value = false
	}
}

function applyCachedActivity() {
	const cached = coreActivityCache.get(activityCacheKey.value)
	activityEntries.value = cached?.entries ?? []
	activityCursor.value = cached?.cursor ?? null
}

function applyInitialActivityPage(
	cacheKey: string,
	entries: CoreActivityLogEntry[],
	cursor: string | null,
) {
	const cached = coreActivityCache.get(cacheKey)
	if (
		cached &&
		cached.entries.length > entries.length &&
		sameSerialized(cached.entries.slice(0, entries.length), entries)
	) {
		activityEntries.value = cached.entries
		activityCursor.value = cached.cursor
		return
	}

	applyActivitySnapshot(cacheKey, entries, cursor)
}

function applyActivitySnapshot(
	cacheKey: string,
	entries: CoreActivityLogEntry[],
	cursor: string | null,
) {
	const current = coreActivityCache.get(cacheKey)
	if (!current || current.cursor !== cursor || !sameSerialized(current.entries, entries)) {
		coreActivityCache.set(cacheKey, { entries, cursor })
	}
	activityEntries.value = entries
	activityCursor.value = cursor
}

function sameSerialized(left: unknown, right: unknown) {
	return JSON.stringify(left) === JSON.stringify(right)
}

function fetchActivityPage(cursor: string | null) {
	const query: CoreActivityLogQuery = { limit: 50, cursor }
	return core.listActivity(query)
}

function toAuditEntry(entry: CoreActivityLogEntry): ServerAuditLogEntry {
	const metadata = parseMetadata(entry.metadata_json)
	const actor = actorFor(entry.actor_user_id, metadata)
	const world = entry.instance_id ? { id: entry.instance_id, name: entry.instance_id } : null
	const userAccessKind = userAccessActions[entry.action]

	if (userAccessKind) {
		const targetUserId =
			stringMetadata(metadata, 'user_id', 'target_user_id', 'invitee_user_id') ??
			entry.target_user_id ??
			'unknown'
		const targetUser = targetUserEntity(targetUserId, metadata)
		const permissions = permissionsForMetadata(metadata)

		return {
			id: entry.id,
			actor,
			world,
			event: {
				key: entry.action,
				component: UserAccessEvent as Component,
				props: {
					action: entry.action,
					timestamp: entry.created_at,
					actor,
					world,
					kind: userAccessKind,
					targetUser,
					permissions,
				},
				searchText: [
					entry.action,
					entry.action.replaceAll('_', ' '),
					actor.username,
					targetUser.id,
					targetUser.label,
					permissions,
				]
					.filter((value): value is string => typeof value === 'string' && value.length > 0)
					.join(' ')
					.toLowerCase(),
			},
			timestamp: entry.created_at,
		}
	}

	return {
		id: entry.id,
		actor,
		world,
		event: {
			key: entry.action,
			component: CoreActivityEvent as Component,
			props: {
				action: entry.action,
				timestamp: entry.created_at,
				actor,
				world,
				label: formatActivityAction(entry.action),
				details: entry.target_user_id ? `Target: ${entry.target_user_id}` : undefined,
			},
			searchText: `${entry.action} ${entry.actor_user_id} ${entry.target_user_id ?? ''}`,
		},
		timestamp: entry.created_at,
	}
}

function actorFor(userId: string, metadata: Record<string, unknown>) {
	const username =
		stringMetadata(metadata, 'actor_name', 'actor_username', 'actor_display_name') ??
		userLookup.value.get(userId)?.username ??
		userId
	return {
		id: userId,
		username,
		avatarUrl: userLookup.value.get(userId)?.avatarUrl,
	}
}

function targetUserEntity(userId: string, metadata: Record<string, unknown>): EventEntity {
	const username =
		stringMetadata(
			metadata,
			'username',
			'user_name',
			'display_name',
			'invitee_display_name',
			'target_username',
			'target_display_name',
		) ??
		userLookup.value.get(userId)?.username ??
		userId
	return {
		id: userId,
		label: username,
		iconUrl: userLookup.value.get(userId)?.avatarUrl,
		iconShape: 'circle',
		to:
			username === userId ? undefined : `https://modrinth.com/user/${encodeURIComponent(username)}`,
	}
}

function permissionsForMetadata(metadata: Record<string, unknown>) {
	const permissions = metadata.permissions
	if (typeof permissions === 'string' && permissions.trim()) return permissions
	if (typeof permissions === 'number' && Number.isFinite(permissions)) return permissions
	if (Array.isArray(permissions)) {
		const values = permissions
			.filter((value): value is string => typeof value === 'string')
			.map((value) => value.trim())
			.filter(Boolean)
		if (values.length > 0) return values.join(' | ')
	}

	return rolePermissions(stringMetadata(metadata, 'role', 'role_id', 'permission_preset'))
}

function rolePermissions(role: string | null): string | null {
	if (role === 'owner') return 'SERVER_ADMIN'
	if (role === 'admin' || role === 'editor' || role === 'role-admin')
		return 'BASE_READ | POWER_ACTIONS | FILES_WRITE'
	if (role === 'member' || role === 'viewer' || role === 'role-member') return 'BASE_READ'
	return null
}

function parseMetadata(raw: string | null | undefined): Record<string, unknown> {
	if (!raw) return {}
	try {
		const parsed = JSON.parse(raw)
		return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {}
	} catch {
		return {}
	}
}

function stringMetadata(metadata: Record<string, unknown>, ...keys: string[]) {
	for (const key of keys) {
		const value = metadata[key]
		if (typeof value === 'string' && value.trim()) return value
	}
	return null
}

function addUserLookup(
	users: Map<string, UserLookup>,
	user:
		| {
				userId: string
				username?: string
				displayName?: string
				name?: string
				image?: string
				avatar_url?: string | null
		  }
		| null
		| undefined,
) {
	if (!user?.userId) return
	users.set(user.userId, {
		username: user.username || user.displayName || user.name || user.userId,
		avatarUrl: user.image || user.avatar_url || undefined,
	})
}
</script>

<template>
	<div class="flex flex-col gap-4">
		<AuditLogTable
			v-model:sort-direction="auditLogSortDirection"
			v-model:timeframe-mode="auditLogTimeframeMode"
			v-model:timeframe-preset="auditLogTimeframePreset"
			v-model:timeframe-last-amount="auditLogTimeframeLastAmount"
			v-model:timeframe-last-unit="auditLogTimeframeLastUnit"
			v-model:timeframe-custom-start-date="auditLogTimeframeCustomStartDate"
			v-model:timeframe-custom-end-date="auditLogTimeframeCustomEndDate"
			:entries="filteredEntries"
			:has-active-external-filters="hasActiveAuditLogFilters"
			:has-more="hasMoreActivity"
			:loading="activityLoading"
			:loading-more="activityLoadingMore"
			:show-world-column="false"
			:suppress-row-transitions="false"
			@load-more="loadMoreActivity"
		>
			<template #filters>
				<DropdownFilterBar
					v-model="auditLogFilters"
					:categories="auditLogFilterCategories"
					add-label="Add filter"
					clear-label="Clear"
					empty-options-label="No filters available."
					empty-search-label="No filters found."
					apply-immediately
					use-filter-icon
					checkbox-position="right"
				/>
			</template>
		</AuditLogTable>
	</div>
</template>
