<script setup lang="ts">
import type { TimeFrameLastUnit, TimeFrameMode, TimeFramePreset } from '@modrinth/ui'
import { AuditLogTable, DropdownFilterBar } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useCoreActivityLog } from '@/components/core/use-core-activity-log'

const { auditEntries } = useCoreActivityLog()
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
			{ value: 'user_permission_modified', label: 'Changed permissions' },
			{ value: 'user_removed', label: 'Removed user' },
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
			:has-more="false"
			:loading="false"
			:loading-more="false"
			:show-world-column="false"
			:suppress-row-transitions="false"
			no-activity-message="Perform an action in your Core and you will see it here!"
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
