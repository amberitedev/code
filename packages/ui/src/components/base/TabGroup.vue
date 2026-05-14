<script setup lang="ts">
import type { Component } from 'vue'
import { computed, nextTick, onMounted, ref, watch } from 'vue'

export interface TabItem {
	id: string
	label: string
	icon?: Component
}

const props = defineProps<{
	tabs: TabItem[]
}>()

const modelValue = defineModel<string>({ required: true })

const tabContainer = ref<HTMLElement | null>(null)
const tabElements = ref<(HTMLElement | null)[]>([])

const sliderLeft = ref(4)
const sliderTop = ref(4)
const sliderRight = ref(4)
const sliderBottom = ref(4)
const sliderReady = ref(false)
const transitionsEnabled = ref(false)
const sliderDelays = ref({ left: '0ms', right: '0ms' })

const leftDelay = computed(() => sliderDelays.value.left)
const rightDelay = computed(() => sliderDelays.value.right)

const sliderStyle = computed(() => ({
	left: `${sliderLeft.value}px`,
	top: `${sliderTop.value}px`,
	right: `${sliderRight.value}px`,
	bottom: `${sliderBottom.value}px`,
	opacity: sliderReady.value ? 1 : 0,
}))

function positionSlider() {
	const activeIdx = props.tabs.findIndex((t) => t.id === modelValue.value)
	const el = tabElements.value[activeIdx]
	const container = tabContainer.value
	if (!el || !container) return

	const newLeft = el.offsetLeft
	const newTop = el.offsetTop
	const newRight = container.offsetWidth - el.offsetLeft - el.offsetWidth
	const newBottom = container.offsetHeight - el.offsetTop - el.offsetHeight

	if (!sliderReady.value) {
		sliderLeft.value = newLeft
		sliderRight.value = newRight
		sliderTop.value = newTop
		sliderBottom.value = newBottom
		sliderReady.value = true
		requestAnimationFrame(() => {
			transitionsEnabled.value = true
		})
		return
	}

	const STAGGER = '150ms'
	const goingLeft = newLeft < sliderLeft.value
	sliderDelays.value = {
		left: goingLeft ? '0ms' : STAGGER,
		right: goingLeft ? STAGGER : '0ms',
	}
	sliderLeft.value = newLeft
	sliderRight.value = newRight
	sliderTop.value = newTop
	sliderBottom.value = newBottom
}

async function updateSlider() {
	await nextTick()
	positionSlider()
}

onMounted(updateSlider)
watch(() => modelValue.value, updateSlider)
</script>

<template>
	<div class="overflow-hidden rounded-xl border border-surface-5">
		<div
			ref="tabContainer"
			class="relative flex border-b border-surface-5 bg-bg-raised p-1 text-sm font-bold drop-shadow-sm"
		>
			<button
				v-for="(tab, i) in tabs"
				:key="tab.id"
				:ref="(el) => (tabElements[i] = el as HTMLElement | null)"
				class="z-[1] flex cursor-pointer items-center gap-2 rounded-full px-4 py-2 focus-visible:outline-none"
				:class="modelValue === tab.id ? 'text-button-textSelected' : 'text-contrast hover:text-primary'"
				@click="modelValue = tab.id"
			>
				<component :is="tab.icon" v-if="tab.icon" class="size-4" />
				{{ tab.label }}
			</button>

			<div
				v-if="sliderReady"
				class="pointer-events-none absolute overflow-hidden rounded-full bg-button-bgSelected"
				:class="{ 'slider-transition': transitionsEnabled }"
				:style="sliderStyle"
				aria-hidden="true"
			/>
		</div>

		<div class="flex flex-col gap-4 bg-surface-2 p-4">
			<slot />
		</div>
	</div>
</template>

<style scoped>
.slider-transition {
	transition:
		left 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(leftDelay),
		right 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(rightDelay),
		top 150ms cubic-bezier(0.4, 0, 0.2, 1) 0ms,
		bottom 150ms cubic-bezier(0.4, 0, 0.2, 1) 0ms,
		opacity 250ms cubic-bezier(0.5, 0, 0.2, 1) 50ms;
}
</style>
