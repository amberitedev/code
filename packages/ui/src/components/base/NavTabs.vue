<template>
	<div
		v-if="filteredLinks.length > 1"
		:class="pageNav ? '-mx-6 -mt-2 mb-1 overflow-x-auto px-6 py-2' : 'contents'"
		v-bind="pageNav ? $attrs : {}"
	>
		<nav
			ref="scrollContainer"
			class="relative flex w-fit rounded-full bg-bg-raised p-1 text-xs sm:text-sm font-bold"
			:class="{
				'card-shadow border border-solid border-surface-4': mode === 'navigation',
			}"
			v-bind="pageNav ? {} : $attrs"
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
					@click="onLinkClick($event, index)"
					@mouseenter="link.onHover?.()"
					@focus="link.onHover?.()"
				>
					<component
						:is="link.icon"
						v-if="link.icon"
						class="tab-color hidden sm:block size-5"
						:class="getIconClasses(index)"
					/>
					<span class="tab-color text-nowrap" :class="getLabelClasses(index)">
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
					@click="emit('tabClick', index, link)"
				>
					<component
						:is="link.icon"
						v-if="link.icon"
						class="tab-color size-5"
						:class="getIconClasses(index)"
					/>
					<span class="tab-color text-nowrap" :class="getLabelClasses(index)">
						{{ link.label }}
					</span>
				</div>
			</template>

			<div
				v-if="sliderReady && currentActiveIndex !== -1"
				ref="sliderElement"
				class="pointer-events-none absolute origin-top-left rounded-full"
				:class="subpageSelected ? 'bg-button-bg' : 'bg-button-bgSelected'"
				:style="sliderStyle"
				aria-hidden="true"
			/>
		</nav>
	</div>
</template>

<script setup lang="ts">
import type { Component } from 'vue'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

defineOptions({ inheritAttrs: false })

interface Tab {
	label: string
	href: string
	shown?: boolean
	icon?: Component
	subpages?: string[]
	onHover?: () => void
}

const props = withDefaults(
	defineProps<{
		replace?: boolean
		links: Tab[]
		query?: string
		mode?: 'navigation' | 'local'
		activeIndex?: number
		pageNav?: boolean
	}>(),
	{
		mode: 'navigation',
		query: undefined,
		activeIndex: undefined,
		pageNav: false,
	},
)

const emit = defineEmits<{
	tabClick: [index: number, tab: Tab]
}>()

// DOM refs
const scrollContainer = ref<HTMLElement | null>(null)
const tabLinkElements = ref<HTMLElement[]>()

const sliderElement = ref<HTMLElement | null>(null)

interface SliderRect {
	left: number
	top: number
	width: number
	height: number
}

const sliderRect = ref<SliderRect>({ left: 0, top: 0, width: 0, height: 0 })

const currentActiveIndex = ref(-1)
const subpageSelected = ref(false)

const sliderReady = ref(false)

const filteredLinks = computed(() => props.links.filter((link) => link.shown ?? true))

const sliderStyle = computed(() => ({
	left: `${sliderRect.value.left}px`,
	top: `${sliderRect.value.top}px`,
	width: `${sliderRect.value.width}px`,
	height: `${sliderRect.value.height}px`,
}))

const isActiveAndNotSubpage = computed(
	() => (index: number) => currentActiveIndex.value === index && !subpageSelected.value,
)

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

	const container = scrollContainer.value as HTMLElement | undefined
	if (!container) return null

	const tabs = container.querySelectorAll('.button-animation')
	const element = tabs[index] as HTMLElement | undefined

	if (!element) return null

	return element
}

function getSliderRect(): SliderRect | null {
	const el = getTabElement(currentActiveIndex.value)
	if (!el) return null

	return {
		left: el.offsetLeft,
		top: el.offsetTop,
		width: el.offsetWidth,
		height: el.offsetHeight,
	}
}

let sliderAnimation: Animation | undefined

/**
 * Moves the pill under the active tab. The pill is placed at its final box right away and the
 * move is played back as a transform animation from the previous box, so it runs on the
 * compositor and stays smooth while the page below is rendering.
 */
function positionSlider(animate = true) {
	const next = getSliderRect()
	if (!next) {
		return
	}

	const previous = sliderRect.value
	const slider = sliderElement.value
	sliderRect.value = next
	sliderReady.value = true

	const moved =
		previous.left !== next.left ||
		previous.top !== next.top ||
		previous.width !== next.width ||
		previous.height !== next.height
	if (!moved) return

	sliderAnimation?.cancel()
	sliderAnimation = undefined
	if (!animate || !slider || !previous.width || !next.width || !next.height) return

	sliderAnimation = slider.animate(
		[
			{
				transform: `translate(${previous.left - next.left}px, ${previous.top - next.top}px) scale(${previous.width / next.width}, ${previous.height / next.height})`,
			},
			{ transform: 'none' },
		],
		{ duration: 150, easing: 'cubic-bezier(0.4, 0, 0.2, 1)' },
	)
}

// Runs from the ResizeObserver, which also reports once on mount. Layout is already clean there,
// so the first measurement does not force one while the page is still mounting.
function resetSliderPosition() {
	if (currentActiveIndex.value === -1) {
		return
	}
	positionSlider(false)
}

function setActiveTab(index: number, isSubpage: boolean) {
	const changed = index !== currentActiveIndex.value
	currentActiveIndex.value = index
	subpageSelected.value = isSubpage

	if (index !== -1 && changed) {
		positionSlider()
	}
}

function updateActiveTab() {
	const { index, isSubpage } = computeActiveIndex()
	setActiveTab(index, isSubpage)
}

// Moves the pill on the click itself instead of waiting for the navigation to commit. The route
// watcher and the failure hook below correct it if the navigation lands somewhere else.
function onLinkClick(event: MouseEvent, index: number) {
	if (event.defaultPrevented || event.button !== 0) return
	if (event.metaKey || event.altKey || event.ctrlKey || event.shiftKey) return
	setActiveTab(index, false)
}

const initialActive = computeActiveIndex()
currentActiveIndex.value = initialActive.index
subpageSelected.value = initialActive.isSubpage

let resizeObserver: ResizeObserver | undefined

let removeNavigationHook: (() => void) | undefined

// The nav only renders with more than one tab, so it can appear after mount.
function observeScrollContainer() {
	resizeObserver?.disconnect()
	if (scrollContainer.value) {
		resizeObserver?.observe(scrollContainer.value)
	}
}

watch(scrollContainer, observeScrollContainer, { flush: 'post' })

onMounted(() => {
	resizeObserver = new ResizeObserver(resetSliderPosition)
	observeScrollContainer()
	removeNavigationHook = router?.afterEach((_to, _from, failure) => {
		if (failure && props.mode === 'navigation') {
			updateActiveTab()
		}
	})
})

onUnmounted(() => {
	resizeObserver?.disconnect()
	removeNavigationHook?.()
	sliderAnimation?.cancel()
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
	() => {
		if (props.mode === 'local') {
			updateActiveTab()
		}
	},
)

// Consumers often rebuild the links array on every route change, so this only reacts when the
// rendered tabs differ. Size changes are picked up by the ResizeObserver.
watch(
	() => filteredLinks.value.map((link) => `${link.href} ${link.label}`).join('|'),
	updateActiveTab,
	{ flush: 'post' },
)
</script>

<style scoped>
.tab-color {
	transition: color 100ms cubic-bezier(0.4, 0, 0.2, 1);
}
</style>
