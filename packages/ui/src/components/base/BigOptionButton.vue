<template>
	<button
		class="group relative flex w-full isolate overflow-hidden bg-surface-4 hover:cursor-pointer items-center gap-3 rounded-[20px] p-3 text-left transition-[filter,transform] duration-150 hover:brightness-110 active:scale-[0.98] border-none"
		:class="{ '!cursor-not-allowed opacity-50 hover:brightness-100 active:scale-100': disabled }"
		:disabled="disabled"
		@click="$emit('click')"
	>
		<span
			class="pointer-events-none absolute inset-0 z-0 bg-brand-highlight transition-opacity duration-200 ease-out"
			:class="selected ? 'opacity-100' : 'opacity-0'"
		/>
		<div
			class="relative z-[1] flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border border-solid transition-colors duration-200"
			:class="selected ? 'border-brand' : 'border-surface-5'"
		>
			<component
				:is="icon"
				class="size-8 text-secondary transition-colors duration-200"
				:class="selected ? '!stroke-brand' : ''"
				stroke-width="1.5"
			/>
		</div>
		<div class="relative z-[1] flex flex-1 flex-col gap-1">
			<span class="text-base font-semibold text-contrast">{{ title }}</span>
			<span class="text-sm font-medium text-primary">{{ description }}</span>
		</div>
		<ChevronRightIcon
			class="relative z-[1] size-5 shrink-0 text-secondary opacity-0 transition-opacity duration-100 group-hover:opacity-100"
		/>
	</button>
</template>

<script setup lang="ts">
import { ChevronRightIcon } from '@modrinth/assets'
import type { Component } from 'vue'

defineProps<{
	icon: Component
	title: string
	description: string
	selected?: boolean
	disabled?: boolean
}>()

defineEmits<{
	(e: 'click'): void
}>()
</script>
