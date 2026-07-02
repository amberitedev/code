<template>
	<div
		ref="outerRef"
		data-tauri-drag-region
		class="min-w-0 overflow-hidden pl-3"
		:class="{ 'breadcrumb-fade-mask': isOverflowing }"
		:style="outerStyle"
		@mouseenter="onMouseEnter"
		@mouseleave="onMouseLeave"
	>
		<div
			ref="innerRef"
			data-tauri-drag-region
			class="breadcrumbs-layer-stack"
			:class="{ 'breadcrumbs-scroll': isAnimating }"
			@animationiteration="onAnimationIteration"
		>
			<div class="breadcrumbs-layer" :style="routeBreadcrumbsStyle">
				<template v-for="breadcrumb in routeBreadcrumbs" :key="breadcrumb.name">
					<router-link
						v-if="breadcrumb.link"
						data-tauri-drag-region-exclude
						:to="{
							path: breadcrumb.link.replace('{id}', encodeURIComponent($route.params.id as string)),
							query: breadcrumb.query,
						}"
						class="shrink-0 whitespace-nowrap text-primary"
					>
						{{ resolveLabel(breadcrumb.name) }}
					</router-link>
					<span
						v-else
						data-tauri-drag-region
						class="shrink-0 whitespace-nowrap text-contrast font-semibold cursor-default select-none"
					>
						{{ resolveLabel(breadcrumb.name) }}
					</span>
					<ChevronRightIcon
						v-if="breadcrumb.link"
						data-tauri-drag-region
						class="w-5 h-5 shrink-0"
					/>
				</template>
			</div>
			<div
				v-if="visualPreviewBreadcrumbs"
				class="breadcrumbs-layer breadcrumbs-layer-preview"
				:style="previewBreadcrumbsStyle"
			>
				<template v-for="breadcrumb in visualPreviewBreadcrumbs" :key="breadcrumb.name">
					<router-link
						v-if="breadcrumb.link"
						data-tauri-drag-region-exclude
						:to="{
							path: breadcrumb.link.replace('{id}', encodeURIComponent($route.params.id as string)),
							query: breadcrumb.query,
						}"
						class="shrink-0 whitespace-nowrap text-primary"
					>
						{{ resolveLabel(breadcrumb.name) }}
					</router-link>
					<span
						v-else
						data-tauri-drag-region
						class="shrink-0 whitespace-nowrap text-contrast font-semibold cursor-default select-none"
					>
						{{ resolveLabel(breadcrumb.name) }}
					</span>
					<ChevronRightIcon
						v-if="breadcrumb.link"
						data-tauri-drag-region
						class="w-5 h-5 shrink-0"
					/>
				</template>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { ChevronRightIcon } from '@modrinth/assets'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { useBreadcrumbs } from '@/store/breadcrumbs'

interface Breadcrumb {
	name: string
	link?: string
	query?: Record<string, string>
}

const route = useRoute()
const breadcrumbData = useBreadcrumbs()
const props = defineProps<{
	previewBreadcrumbs?: Breadcrumb[]
	previewProgress?: number
}>()

const routeBreadcrumbs = computed<Breadcrumb[]>(() => {
	const additionalContext =
		route.meta.useContext === true
			? breadcrumbData.context
			: route.meta.useRootContext === true
				? breadcrumbData.rootContext
				: null
	const crumbs = (route.meta.breadcrumb ?? []) as Breadcrumb[]
	return additionalContext ? [additionalContext as Breadcrumb, ...crumbs] : crumbs
})
const visualPreviewBreadcrumbs = computed(() => props.previewBreadcrumbs)
const pullProgress = computed(() => {
	const progress = props.previewProgress ?? 0
	return Math.min(1, Math.max(0, (progress - 0.04) / 0.96))
})
const routeProgress = computed(() => Math.min(1, Math.max(0, (pullProgress.value - 0.1) / 0.34)))
const previewProgressValue = computed(() =>
	Math.min(1, Math.max(0, (pullProgress.value - 0.5) / 0.34)),
)
const outerStyle = computed(() =>
	isOverflowing.value ? { '--scroll-distance': `-${overflowAmount.value}px` } : undefined,
)
const routeBreadcrumbsStyle = computed(() => ({
	opacity: `${1 - routeProgress.value}`,
	pointerEvents: pullProgress.value > 0.01 ? 'none' : undefined,
}))
const previewBreadcrumbsStyle = computed(() => ({
	opacity: `${previewProgressValue.value}`,
}))

function resolveLabel(name: string): string {
	return name.charAt(0) === '?' ? breadcrumbData.getName(name.slice(1)) : name
}

// Overflow detection
const outerRef = ref<HTMLDivElement | null>(null)
const innerRef = ref<HTMLDivElement | null>(null)
const isOverflowing = ref(false)
const isAnimating = ref(false)
const overflowAmount = ref(0)

let hovered = false
let stopping = false

function checkOverflow() {
	if (!outerRef.value || !innerRef.value) return
	const overflow = innerRef.value.scrollWidth - outerRef.value.clientWidth
	isOverflowing.value = overflow > 0
	overflowAmount.value = overflow + 12
}

function onMouseEnter() {
	hovered = true
	stopping = false
	if (isOverflowing.value) {
		isAnimating.value = true
	}
}

function onMouseLeave() {
	hovered = false
	if (isAnimating.value) {
		stopping = true
	}
}

function onAnimationIteration() {
	if (stopping && !hovered) {
		isAnimating.value = false
		stopping = false
	}
}

let resizeObserver: ResizeObserver | null = null

onMounted(() => {
	checkOverflow()
	resizeObserver = new ResizeObserver(checkOverflow)
	if (outerRef.value) resizeObserver.observe(outerRef.value)
	if (innerRef.value) resizeObserver.observe(innerRef.value)
})

onBeforeUnmount(() => {
	resizeObserver?.disconnect()
})

watch(
	routeBreadcrumbs,
	(breadcrumbs) => {
		breadcrumbData.resetToNames(breadcrumbs)
		requestAnimationFrame(checkOverflow)
	},
	{ immediate: true },
)

watch(
	() => props.previewBreadcrumbs,
	() => {
		requestAnimationFrame(checkOverflow)
	},
)

watch(pullProgress, () => {
	requestAnimationFrame(checkOverflow)
})
</script>

<style scoped>
.breadcrumb-fade-mask {
	mask-image: linear-gradient(
		to right,
		transparent,
		black 12px,
		black calc(100% - 12px),
		transparent
	);
}

.breadcrumbs-scroll {
	animation: breadcrumb-scroll 10s ease-in-out infinite;
}

.breadcrumbs-layer-stack {
	display: grid;
	width: fit-content;
	align-items: center;
	min-height: 1.5rem;
}

.breadcrumbs-layer {
	grid-area: 1 / 1;
	display: flex;
	width: max-content;
	align-items: center;
	gap: 0.25rem;
	will-change: opacity;
}

.breadcrumbs-layer-preview {
	pointer-events: none;
}

[data-tauri-drag-region-exclude] {
	-webkit-app-region: no-drag;
}

@keyframes breadcrumb-scroll {
	0% {
		transform: translateX(0);
	}
	35%,
	65% {
		transform: translateX(var(--scroll-distance));
	}
	100% {
		transform: translateX(0);
	}
}
</style>
