<template>
	<div
		data-pyro-server-stats
		style="font-variant-numeric: tabular-nums"
		class="flex select-none flex-col items-center gap-3 md:flex-row"
		:class="{ 'pointer-events-none': loading }"
		:aria-hidden="loading"
	>
		<component
			:is="metric.link && !loading ? RouterLink : 'div'"
			v-for="(metric, index) in metrics"
			:key="index"
			:to="metric.link && !loading ? metric.link : undefined"
			class="relative isolate min-h-[145px] w-full overflow-hidden rounded-[20px] bg-surface-3 p-5"
			:class="
				metric.link && !loading
					? 'cursor-pointer transition-transform duration-100 hover:brightness-125 active:scale-95'
					: ''
			"
		>
			<div class="relative z-10 flex flex-col gap-2">
				<div class="flex items-center justify-between">
					<span class="stat-drop-shadow flex items-center gap-2 font-medium text-lg text-primary">
						{{ metric.title }}
					</span>
					<span class="relative">
						<component :is="metric.icon" class="stat-drop-shadow relative z-10 size-8" />
					</span>
				</div>
				<span class="stat-drop-shadow text-4xl font-bold text-contrast">
					{{ metric.value
					}}<span
						v-if="metric.secondary"
						class="ml-1 text-sm font-normal stat-drop-shadow text-secondary"
						>{{ metric.secondary }}</span
					>
				</span>
			</div>

			<div v-if="metric.showGraph" class="chart-space absolute bottom-0 left-0 right-0">
				<VueApexCharts
					v-if="isClient && !loading && metric.chartOptions"
					type="area"
					height="142"
					:options="metric.chartOptions"
					:series="metric.series!"
					class="chart"
					:class="chartsReady.has(index) ? 'opacity-100' : 'opacity-0'"
				/>
			</div>
		</component>
	</div>
</template>

<script setup lang="ts">
import { injectModrinthServerContext, injectPageContext, useFormatBytes } from '@modrinth/ui'
import type { Stats } from '@modrinth/utils'
import { useStorage } from '@vueuse/core'
import { computed, defineAsyncComponent, onMounted, ref, shallowRef, watch } from 'vue'
import { RouterLink } from 'vue-router'

import {
	buildChartOptions,
	buildMetrics,
	CPU_DATA_MAX,
	padGraph,
	RAM_DATA_MAX,
} from './server-stats-chart'

const VueApexCharts = defineAsyncComponent(() => import('vue3-apexcharts'))

const isClient = ref(false)
onMounted(() => {
	isClient.value = true
})

const { serverId } = injectModrinthServerContext()
const { featureFlags } = injectPageContext()

const props = withDefaults(
	defineProps<{
		data?: Stats
		loading?: boolean
		showMemoryAsBytes?: boolean
		storageLink?: string | null
	}>(),
	{
		data: undefined,
		loading: false,
		showMemoryAsBytes: false,
		storageLink: null,
	},
)

const formatBytes = useFormatBytes()

const chartsReady = ref(new Set<number>())
const userPreferences = useStorage(`pyro-server-${serverId || 'unknown'}-preferences`, {
	ramAsNumber: false,
})
const isRamAsBytesForcedByFeatureFlag = computed(
	() => featureFlags?.serverRamAsBytesAlwaysOn?.value ?? false,
)

const showRamAsBytes = computed(
	() =>
		props.showMemoryAsBytes ||
		isRamAsBytesForcedByFeatureFlag.value ||
		userPreferences.value.ramAsNumber,
)

const stats = shallowRef(
	props.data?.current || {
		cpu_percent: 0,
		ram_usage_bytes: 0,
		ram_total_bytes: 1,
		storage_usage_bytes: 0,
	},
)

const onChartReady = (index: number) => {
	chartsReady.value.add(index)
}

const cpuData = computed(() => padGraph(props.data?.graph.cpu ?? []))
const ramData = computed(() => padGraph(props.data?.graph.ram ?? []))

const cpuPercent = computed(() => stats.value.cpu_percent ?? 0)
const ramPercent = computed(
	() => ((stats.value.ram_usage_bytes ?? 0) / (stats.value.ram_total_bytes || 1)) * 100,
)

const cpuChartOptions = computed(() =>
	buildChartOptions(cpuPercent.value >= 90, 0, CPU_DATA_MAX, onChartReady),
)
const ramChartOptions = computed(() =>
	buildChartOptions(ramPercent.value >= 90, 1, RAM_DATA_MAX, onChartReady),
)

const cpuSeries = computed(() => [{ name: 'CPU', data: cpuData.value }])
const ramSeries = computed(() => [{ name: 'Memory', data: ramData.value }])

const metrics = computed(() =>
	buildMetrics({
		loading: props.loading,
		formatBytes,
		storageUsageBytes: stats.value.storage_usage_bytes ?? 0,
		cpuPercent: cpuPercent.value,
		ramUsageBytes: stats.value.ram_usage_bytes ?? 0,
		ramTotalBytes: stats.value.ram_total_bytes ?? 0,
		showRamAsBytes: showRamAsBytes.value,
		cpuChartOptions: cpuChartOptions.value,
		ramChartOptions: ramChartOptions.value,
		cpuSeries: cpuSeries.value,
		ramSeries: ramSeries.value,
		storageLink: props.storageLink,
	}),
)

watch(
	() => props.data?.current,
	(newStats) => {
		if (newStats) {
			stats.value = newStats
		}
	},
)
</script>

<style scoped>
.stat-drop-shadow {
	filter: drop-shadow(0 4px 6px var(--surface-3));
}

.chart-space {
	height: 142px;
	width: calc(100% + 40px);
	margin-left: -20px;
	margin-right: -20px;
}

.chart {
	width: 100% !important;
	height: 142px !important;
	transition: opacity 0.3s ease-out;
	box-shadow:
		0 1px 2px 0 rgba(0, 0, 0, 0.3),
		0 1px 3px 0 rgba(0, 0, 0, 0.15);
}

.chart :deep(svg) {
	overflow: visible;
}
</style>
