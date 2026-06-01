<template>
	<div class="tab-view flex h-full min-h-0 w-full flex-col">
		<div
			class="tab-view-bar flex w-full select-none flex-col justify-between gap-4 md:flex-row md:items-center"
			:class="barClass"
		>
			<NavTabs mode="local" :links="navLinks" :active-index="activeIndex" @tab-click="onTabClick" />
			<div v-if="$slots.actions" class="flex items-center gap-2">
				<slot name="actions" :active-tab="modelValue" />
			</div>
		</div>

		<div class="tab-view-panels relative mt-4 min-h-0 w-full flex-1">
			<div
				v-for="tab in tabs"
				v-show="tab.id === modelValue"
				:key="tab.id"
				class="tab-view-panel h-full w-full overflow-y-auto"
				:data-tab-id="tab.id"
				role="tabpanel"
			>
				<slot :name="tab.id" :tab="tab" :active="tab.id === modelValue" />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { NavTabs } from '@modrinth/ui'
import { type Component, computed } from 'vue'

/**
 * A single tab definition. `id` is used as the slot name for its content panel.
 * Optional `color` is a CSS color (or var) used by consumers to colour-code the
 * tab's content (e.g. server vs client name tags) — TabView itself stays neutral
 * for now to match the existing Modrinth pill look.
 */
export interface TabViewTab {
	id: string
	label: string
	icon?: Component
	shown?: boolean
	color?: string
}

const props = withDefaults(
	defineProps<{
		tabs: TabViewTab[]
		/** Extra classes for the tab bar row (e.g. sticky positioning). */
		barClass?: string
	}>(),
	{
		barClass: '',
	},
)

const modelValue = defineModel<string>({ required: true })

const visibleTabs = computed(() => props.tabs.filter((tab) => tab.shown ?? true))

const navLinks = computed(() =>
	visibleTabs.value.map((tab) => ({
		label: tab.label,
		href: tab.id,
		icon: tab.icon,
	})),
)

const activeIndex = computed(() => {
	const index = visibleTabs.value.findIndex((tab) => tab.id === modelValue.value)
	return index === -1 ? 0 : index
})

function onTabClick(index: number) {
	const tab = visibleTabs.value[index]
	if (tab) {
		modelValue.value = tab.id
	}
}
</script>
