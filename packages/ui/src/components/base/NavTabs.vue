<template>
	<nav
		v-if="filteredLinks.length > 1"
		ref="scrollContainer"
		class="relative flex w-fit overflow-x-auto rounded-full bg-bg-raised p-1 text-sm font-bold"
		:class="{ 'drop-shadow-xl border border-solid border-surface-4': mode === 'navigation' }"
	>
		<template v-if="mode === 'navigation'">
			<RouterLink
				v-for="(link, index) in filteredLinks"
				v-show="link.shown ?? true"
				:key="link.href"
				ref="tabLinkElements"
				:replace="replace"
				:to="query ? (link.href ? `?${query}=${link.href}` : '?') : link.href"
				class="button-animation z-[1] flex flex-row items-center gap-2 px-4 py-2 focus:rounded-full"
				:class="getSSRFallbackClasses(index)"
				@pointerup="handleNavigationTabPointerUp(index, $event)"
				@click.capture="handleNavigationTabClick(index, $event)"
				@mouseenter="link.onHover?.()"
				@focus="link.onHover?.()"
			>
				<component :is="link.icon" v-if="link.icon" class="size-5" :class="getIconClasses(index)" />
				<span class="text-nowrap" :class="getLabelClasses(index)">
					{{ link.label }}
				</span>
			</RouterLink>
		</template>

		<template v-else>
			<div
				v-for="(link, index) in filteredLinks"
				v-show="link.shown ?? true"
				:key="link.href"
				ref="tabLinkElements"
				class="button-animation z-[1] flex flex-row items-center gap-2 px-4 py-2 hover:cursor-pointer focus:rounded-full"
				:class="getSSRFallbackClasses(index)"
				@pointerup="handleLocalTabPointerUp(index, link, $event)"
				@click="handleLocalTabClick(index, link)"
				@mouseenter="link.onHover?.()"
				@focus="link.onHover?.()"
			>
				<component :is="link.icon" v-if="link.icon" class="size-5" :class="getIconClasses(index)" />
				<span class="text-nowrap" :class="getLabelClasses(index)">
					{{ link.label }}
				</span>
			</div>
		</template>

		<!-- Animated slider background -->
		<div
			v-if="sliderReady && currentActiveIndex !== -1"
			class="pointer-events-none absolute h-[calc(100%-0.5rem)] overflow-hidden rounded-full p-1"
			:class="[
				subpageSelected ? 'bg-button-bg' : 'bg-button-bgSelected',
				{ 'navtabs-transition': transitionsEnabled },
			]"
			:style="sliderStyle"
			aria-hidden="true"
		/>
	</nav>
</template>

<script setup lang="ts">
import type { Component } from 'vue'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

import {
	UI_MOTION_NAV_TABS_FADE_DELAY_MS,
	UI_MOTION_NAV_TABS_FADE_EASING,
	UI_MOTION_NAV_TABS_FADE_MS,
	UI_MOTION_NAV_TABS_SLIDER_EASING,
	UI_MOTION_NAV_TABS_SLIDER_MS,
	UI_MOTION_NAV_TABS_SLIDER_STAGGER_DELAY_MS,
} from '#ui/composables/ui-motion'

const route = useRoute()

interface Tab {
	label: string
	href: string
	shown?: boolean
	icon?: Component
	subpages?: string[]
	onHover?: () => void
}

interface SliderPosition {
	left: number
	top: number
	right: number
	bottom: number
}

const props = withDefaults(
	defineProps<{
		replace?: boolean
		links: Tab[]
		query?: string
		mode?: 'navigation' | 'local'
		activeIndex?: number
	}>(),
	{
		mode: 'navigation',
		query: undefined,
		activeIndex: undefined,
	},
)

const emit = defineEmits<{
	tabClick: [index: number, tab: Tab]
}>()

// DOM refs
const scrollContainer = ref<HTMLElement | null>(null)
const tabLinkElements = ref<HTMLElement[]>()

// Slider pos state
const sliderLeft = ref(4)
const sliderTop = ref(4)
const sliderRight = ref(4)
const sliderBottom = ref(4)

// active tab state
const currentActiveIndex = ref(-1)
const subpageSelected = ref(false)

// SSR state
const sliderReady = ref(false)
const transitionsEnabled = ref(false)

// Stagger delays for the trailing edges of the slider animation
const sliderDelays = ref({ left: '0ms', top: '0ms', right: '0ms', bottom: '0ms' })

const filteredLinks = computed(() => props.links.filter((link) => link.shown ?? true))
const linkLayoutSignature = computed(() =>
	filteredLinks.value
		.map(
			(link) =>
				`${link.label}:${link.shown === false ? 'hidden' : 'shown'}:${link.icon ? 'icon' : 'text'}`,
		)
		.join('|'),
)

const sliderStyle = computed(() => ({
	left: `${sliderLeft.value}px`,
	top: `${sliderTop.value}px`,
	right: `${sliderRight.value}px`,
	bottom: `${sliderBottom.value}px`,
	opacity: sliderReady.value && currentActiveIndex.value !== -1 ? 1 : 0,
	'--navtabs-fade-delay-ms': `${UI_MOTION_NAV_TABS_FADE_DELAY_MS}ms`,
	'--navtabs-fade-easing': UI_MOTION_NAV_TABS_FADE_EASING,
	'--navtabs-fade-ms': `${UI_MOTION_NAV_TABS_FADE_MS}ms`,
	'--navtabs-left-delay': sliderDelays.value.left,
	'--navtabs-right-delay': sliderDelays.value.right,
	'--navtabs-top-delay': sliderDelays.value.top,
	'--navtabs-bottom-delay': sliderDelays.value.bottom,
	'--navtabs-slider-easing': UI_MOTION_NAV_TABS_SLIDER_EASING,
	'--navtabs-slider-ms': `${UI_MOTION_NAV_TABS_SLIDER_MS}ms`,
}))

const isActiveAndNotSubpage = computed(
	() => (index: number) => currentActiveIndex.value === index && !subpageSelected.value,
)

let layoutResizeObserver: ResizeObserver | null = null
let layoutMutationObserver: MutationObserver | null = null
let remeasureFrame: number | null = null
let suppressNextLocalClickIndex: number | null = null

function cancelRemeasureFrame() {
	if (remeasureFrame === null) return

	cancelAnimationFrame(remeasureFrame)
	remeasureFrame = null
}

function scheduleSliderRemeasure() {
	if (typeof window === 'undefined') return

	cancelRemeasureFrame()
	remeasureFrame = requestAnimationFrame(() => {
		remeasureFrame = null
		void updateActiveTab()
	})
}

function disconnectLayoutObservers() {
	layoutResizeObserver?.disconnect()
	layoutResizeObserver = null
	layoutMutationObserver?.disconnect()
	layoutMutationObserver = null
}

function getTabElements() {
	const container = scrollContainer.value
	if (!container) return []

	return Array.from(container.querySelectorAll<HTMLElement>('.button-animation'))
}

function setupLayoutObservers() {
	if (typeof window === 'undefined') return

	disconnectLayoutObservers()

	const container = scrollContainer.value
	if (!container) return

	if (typeof ResizeObserver !== 'undefined') {
		layoutResizeObserver = new ResizeObserver(scheduleSliderRemeasure)
		layoutResizeObserver.observe(container)
		getTabElements().forEach((element) => layoutResizeObserver?.observe(element))
	}

	if (typeof MutationObserver !== 'undefined') {
		layoutMutationObserver = new MutationObserver(() => {
			setupLayoutObservers()
			scheduleSliderRemeasure()
		})
		layoutMutationObserver.observe(container, {
			childList: true,
			subtree: false,
		})
	}
}

function getSSRFallbackClasses(index: number) {
	if (sliderReady.value) return {}
	if (currentActiveIndex.value !== index) return {}

	return {
		'rounded-full': true,
		'bg-button-bgSelected': !subpageSelected.value,
		'bg-button-bg': subpageSelected.value,
	}
}

function getIconClasses(index: number) {
	return {
		'text-button-textSelected': isActiveAndNotSubpage.value(index),
		'text-secondary': !isActiveAndNotSubpage.value(index),
	}
}

function getLabelClasses(index: number) {
	return {
		'text-button-textSelected': isActiveAndNotSubpage.value(index),
		'text-contrast': !isActiveAndNotSubpage.value(index),
	}
}

function computeActiveIndex(): { index: number; isSubpage: boolean } {
	if (props.mode === 'local' && props.activeIndex !== undefined) {
		return {
			index: Math.min(props.activeIndex, filteredLinks.value.length - 1),
			isSubpage: false,
		}
	}

	for (let i = filteredLinks.value.length - 1; i >= 0; i--) {
		const link = filteredLinks.value[i]
		const decodedPath = decodeURIComponent(route.path)
		const decodedHref = decodeURIComponent(link.href.split('?')[0])

		if (props.query) {
			const queryValue = route.query[props.query]
			if (queryValue === link.href || (!queryValue && !link.href)) {
				return { index: i, isSubpage: false }
			}
			continue
		}

		if (decodedPath === decodedHref) {
			return { index: i, isSubpage: false }
		}

		const isSubpageMatch =
			(decodedPath.startsWith(decodedHref) &&
				(decodedPath.length === decodedHref.length || decodedPath[decodedHref.length] === '/')) ||
			link.subpages?.some((subpage) => decodedPath.includes(subpage))

		if (isSubpageMatch) {
			return { index: i, isSubpage: true }
		}
	}

	return { index: -1, isSubpage: false }
}

function getTabElement(index: number): HTMLElement | null {
	if (index === -1) return null

	const element = getTabElements()[index]

	if (!element) return null

	return element
}

function positionSlider() {
	const el = getTabElement(currentActiveIndex.value)
	if (!el?.offsetParent) return

	const parent = el.offsetParent as HTMLElement
	const newPosition = {
		left: el.offsetLeft,
		top: el.offsetTop,
		right: parent.offsetWidth - el.offsetLeft - el.offsetWidth,
		bottom: parent.offsetHeight - el.offsetTop - el.offsetHeight,
	}

	if (!sliderReady.value) {
		applySliderPosition(newPosition)
		sliderReady.value = true

		requestAnimationFrame(() => {
			transitionsEnabled.value = true
		})
	} else if (isSameSliderPosition(newPosition)) {
		return
	} else {
		animateSliderTo(newPosition)
	}
}

function isSameSliderPosition(position: SliderPosition) {
	return (
		position.left === sliderLeft.value &&
		position.right === sliderRight.value &&
		position.top === sliderTop.value &&
		position.bottom === sliderBottom.value
	)
}

function applySliderPosition(position: SliderPosition) {
	sliderLeft.value = position.left
	sliderRight.value = position.right
	sliderTop.value = position.top
	sliderBottom.value = position.bottom
}

function animateSliderTo(newPosition: SliderPosition) {
	const staggerDelay = `${UI_MOTION_NAV_TABS_SLIDER_STAGGER_DELAY_MS}ms`

	sliderDelays.value = {
		left: newPosition.left < sliderLeft.value ? '0ms' : staggerDelay,
		right: newPosition.left < sliderLeft.value ? staggerDelay : '0ms',
		top: newPosition.top < sliderTop.value ? '0ms' : staggerDelay,
		bottom: newPosition.top < sliderTop.value ? staggerDelay : '0ms',
	}

	applySliderPosition(newPosition)
}

function setLocalActiveTab(index: number) {
	if (index < 0 || index >= filteredLinks.value.length) return
	if (currentActiveIndex.value === index && !subpageSelected.value) return

	currentActiveIndex.value = index
	subpageSelected.value = false

	if (sliderReady.value) {
		positionSlider()
	} else {
		positionSlider()
		if (!sliderReady.value) {
			void nextTick(positionSlider)
		}
	}
}

function isPrimaryTabIntent(event: MouseEvent | PointerEvent) {
	return !(
		event.defaultPrevented ||
		event.button !== 0 ||
		event.metaKey ||
		event.altKey ||
		event.ctrlKey ||
		event.shiftKey
	)
}

function handleNavigationTabPointerUp(index: number, event: PointerEvent) {
	if (!isPrimaryTabIntent(event)) return

	setLocalActiveTab(index)
}

function handleNavigationTabClick(index: number, event: MouseEvent) {
	if (!isPrimaryTabIntent(event)) return

	setLocalActiveTab(index)
}

function handleLocalTabPointerUp(index: number, link: Tab, event: PointerEvent) {
	if (!isPrimaryTabIntent(event)) return

	setLocalActiveTab(index)
	suppressNextLocalClickIndex = index
	emit('tabClick', index, link)

	setTimeout(() => {
		if (suppressNextLocalClickIndex === index) {
			suppressNextLocalClickIndex = null
		}
	}, 0)
}

function handleLocalTabClick(index: number, link: Tab) {
	if (suppressNextLocalClickIndex === index) {
		suppressNextLocalClickIndex = null
		return
	}

	setLocalActiveTab(index)
	emit('tabClick', index, link)
}

function updateActiveTab() {
	const { index, isSubpage } = computeActiveIndex()
	currentActiveIndex.value = index
	subpageSelected.value = isSubpage

	if (index !== -1) {
		positionSlider()
	} else {
		sliderLeft.value = 0
		sliderRight.value = 0
	}
}

const initialActive = computeActiveIndex()
currentActiveIndex.value = initialActive.index
subpageSelected.value = initialActive.isSubpage

onMounted(() => {
	void updateActiveTab()
	void nextTick(() => {
		setupLayoutObservers()
		scheduleSliderRemeasure()
	})

	if (typeof window !== 'undefined') {
		window.addEventListener('resize', scheduleSliderRemeasure)
	}

	if (typeof document !== 'undefined' && document.fonts) {
		void document.fonts.ready.then(scheduleSliderRemeasure).catch(() => undefined)
	}
})

watch(
	() => [route.path, route.query],
	() => {
		if (props.mode === 'navigation') {
			updateActiveTab()
		}
	},
)

watch(
	() => props.activeIndex,
	(activeIndex) => {
		if (props.mode !== 'local') return
		if (activeIndex === undefined || activeIndex === -1) {
			updateActiveTab()
			return
		}

		setLocalActiveTab(Math.min(activeIndex, filteredLinks.value.length - 1))
	},
)

watch(linkLayoutSignature, async () => {
	sliderReady.value = false
	transitionsEnabled.value = false
	await nextTick()
	updateActiveTab()
	setupLayoutObservers()
	scheduleSliderRemeasure()
})

onUnmounted(() => {
	cancelRemeasureFrame()
	disconnectLayoutObservers()

	if (typeof window !== 'undefined') {
		window.removeEventListener('resize', scheduleSliderRemeasure)
	}
})
</script>

<style scoped>
.navtabs-transition {
	transition:
		left var(--navtabs-slider-ms) var(--navtabs-slider-easing) var(--navtabs-left-delay),
		right var(--navtabs-slider-ms) var(--navtabs-slider-easing) var(--navtabs-right-delay),
		top var(--navtabs-slider-ms) var(--navtabs-slider-easing) var(--navtabs-top-delay),
		bottom var(--navtabs-slider-ms) var(--navtabs-slider-easing) var(--navtabs-bottom-delay),
		opacity var(--navtabs-fade-ms) var(--navtabs-fade-easing) var(--navtabs-fade-delay-ms);
}
</style>
