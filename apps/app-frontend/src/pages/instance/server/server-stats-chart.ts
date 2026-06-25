import { CpuIcon, DatabaseIcon, FolderOpenIcon } from '@modrinth/assets'
import type { Component } from 'vue'

/**
 * Pure chart/metric helpers for the forked Core server stats card.
 *
 * Extracted from `@modrinth/ui`'s ServerManageStats so the desktop app can own a
 * Core-native variant (storage links to the local instance files page) without
 * editing the shared component library.
 *
 * Key exports: GRAPH_SIZE, padGraph, buildChartOptions, buildMetrics, ServerMetric.
 */

export const GRAPH_SIZE = 10

export const CPU_DATA_MAX = 104
export const RAM_DATA_MAX = 104

export type ServerMetric = {
	title: string
	value: string
	secondary: string | null
	icon: Component
	showGraph: boolean
	chartOptions: ReturnType<typeof buildChartOptions> | null
	series: { name: string; data: number[] }[] | null
	link: string | null
}

export function padGraph(data: number[]): number[] {
	const capped = data.map((v) => Math.min(v, 100))
	if (capped.length >= GRAPH_SIZE) return capped.slice(-GRAPH_SIZE)
	return [...Array(GRAPH_SIZE - capped.length).fill(0), ...capped]
}

export function buildChartOptions(
	warning: boolean,
	index: number,
	dataMax: number,
	onReady: (index: number) => void,
) {
	return {
		chart: {
			type: 'area' as const,
			animations: { enabled: false },
			sparkline: { enabled: true },
			toolbar: { show: false },
			padding: { left: -10, right: -10, top: 0, bottom: 0 },
			events: {
				mounted: () => onReady(index),
				updated: () => onReady(index),
			},
		},
		stroke: { curve: 'smooth' as const, width: 3 },
		fill: {
			type: 'gradient' as const,
			gradient: { shadeIntensity: 1, opacityFrom: 0.25, opacityTo: 0.05, stops: [0, 100] },
		},
		tooltip: { enabled: false },
		grid: { show: false },
		xaxis: {
			labels: { show: false },
			axisBorder: { show: false },
			type: 'numeric' as const,
			tickAmount: GRAPH_SIZE,
		},
		yaxis: { show: false, min: 0, max: dataMax, forceNiceScale: false },
		colors: [warning ? 'var(--color-warning)' : 'var(--color-brand)'],
		dataLabels: { enabled: false },
	}
}

export type BuildMetricsParams = {
	loading: boolean
	formatBytes: (bytes: number, digits?: number) => string
	storageUsageBytes: number
	cpuPercent: number
	ramUsageBytes: number
	ramTotalBytes: number
	showRamAsBytes: boolean
	cpuChartOptions: ReturnType<typeof buildChartOptions>
	ramChartOptions: ReturnType<typeof buildChartOptions>
	cpuSeries: { name: string; data: number[] }[]
	ramSeries: { name: string; data: number[] }[]
	storageLink: string | null
}

export function buildMetrics(params: BuildMetricsParams): ServerMetric[] {
	const storageMetric: ServerMetric = {
		title: 'Storage',
		value: params.formatBytes(params.loading ? 0 : params.storageUsageBytes, 1),
		secondary: null,
		icon: FolderOpenIcon,
		showGraph: false,
		chartOptions: null,
		series: null,
		link: params.storageLink,
	}

	const cpuMetric: ServerMetric = {
		title: 'CPU',
		value: params.loading ? '0.00%' : `${params.cpuPercent.toFixed(2)}%`,
		secondary: null,
		icon: CpuIcon,
		showGraph: true,
		chartOptions: params.cpuChartOptions,
		series: params.cpuSeries,
		link: null,
	}

	const ramPercent = (params.ramUsageBytes / (params.ramTotalBytes || 1)) * 100
	const ramMetric: ServerMetric = {
		title: 'Memory',
		value: params.loading
			? '0.00%'
			: params.showRamAsBytes
				? params.formatBytes(params.ramUsageBytes, 1)
				: `${ramPercent.toFixed(2)}%`,
		secondary:
			!params.loading && params.showRamAsBytes
				? `/ ${params.formatBytes(params.ramTotalBytes, 1)}`
				: null,
		icon: DatabaseIcon,
		showGraph: true,
		chartOptions: params.ramChartOptions,
		series: params.ramSeries,
		link: null,
	}

	return [cpuMetric, ramMetric, storageMetric]
}
