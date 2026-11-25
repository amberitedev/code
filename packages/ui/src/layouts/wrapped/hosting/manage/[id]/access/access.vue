<template>
	<div class="flex flex-col gap-4">
		<span class="m-0 text-2xl font-semibold text-contrast">
			{{ formatMessage(messages.activityLogTitle) }}
		</span>
		<AuditLogTable
			v-model:sort-direction="auditLogSortDirection"
			v-model:timeframe-mode="auditLogTimeframeMode"
			v-model:timeframe-preset="auditLogTimeframePreset"
			v-model:timeframe-last-amount="auditLogTimeframeLastAmount"
			v-model:timeframe-last-unit="auditLogTimeframeLastUnit"
			v-model:timeframe-custom-start-date="auditLogTimeframeCustomStartDate"
			v-model:timeframe-custom-end-date="auditLogTimeframeCustomEndDate"
			:entries="auditEntries"
			:has-active-external-filters="hasActiveAuditLogFilters"
			:has-more="hasMoreActionLogEntries"
			:loading="isActionLogFiltering"
			:loading-more="isLoadingMoreActionLogEntries"
			:show-world-column="showAuditLogInstances"
			:suppress-row-transitions="isActionLogSortTransitioning"
			@load-more="loadMoreActionLogEntries"
		>
			<template #filters>
				<DropdownFilterBar
					v-model="auditLogFilters"
					:categories="auditLogFilterCategories"
					:add-label="formatMessage(messages.addFilter)"
					:clear-label="formatMessage(messages.clearFilters)"
					:empty-options-label="formatMessage(messages.emptyFilterOptions)"
					:empty-search-label="formatMessage(messages.emptyFilterSearch)"
					apply-immediately
					use-filter-icon
					checkbox-position="right"
				/>
			</template>
		</AuditLogTable>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import DropdownFilterBar from '#ui/components/base/DropdownFilterBar.vue'
import { AuditLogTable } from '#ui/components/servers/access'
import { useVIntl } from '#ui/composables/i18n'
import { injectModrinthServerContext, injectNotificationManager } from '#ui/providers'

import { useAccessAuditLog } from './audit-log'
import { accessMessages } from './messages'

const props = withDefaults(
	defineProps<{
		showAuditLogInstances?: boolean
	}>(),
	{
		showAuditLogInstances: false,
	},
)
const showAuditLogInstances = computed(() => props.showAuditLogInstances)

const { formatMessage } = useVIntl()
const { serverId, core } = injectModrinthServerContext()
const { addNotification } = injectNotificationManager()

const messages = accessMessages

const {
	auditEntries,
	auditLogFilterCategories,
	auditLogFilters,
	auditLogSortDirection,
	auditLogTimeframeCustomEndDate,
	auditLogTimeframeCustomStartDate,
	auditLogTimeframeLastAmount,
	auditLogTimeframeLastUnit,
	auditLogTimeframeMode,
	auditLogTimeframePreset,
	hasActiveAuditLogFilters,
	hasMoreActionLogEntries,
	isActionLogFiltering,
	isActionLogSortTransitioning,
	isLoadingMoreActionLogEntries,
	loadMoreActionLogEntries,
} = useAccessAuditLog({
	core,
	serverId,
	showAuditLogInstances,
	addNotification,
})
</script>
