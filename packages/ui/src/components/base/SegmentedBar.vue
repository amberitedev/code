<script lang="ts">
import { computed } from 'vue'

// A bar split into colored parts by size, like the content storage bar in Resources. Each part
// names itself on hover; `legend` also lists them under the bar with their values. Without `max`
// the parts fill the bar; with it, what they leave of `max` stays empty.
export type BarSegment = {
	id: string
	/** Shown on hover and in the legend. */
	label: string
	value: number
	/** A background class, such as `bg-blue`. */
	color: string
	/** The value as the legend shows it, such as `1.2 GiB`. */
	display?: string
}
</script>

<script setup lang="ts">
const props = defineProps<{
	segments: BarSegment[]
	legend?: boolean
	max?: number
}>()

const rest = computed(() =>
	props.max === undefined
		? 0
		: props.max - props.segments.reduce((sum, segment) => sum + Math.max(segment.value, 0), 0),
)
</script>

<template>
	<div class="flex flex-col gap-3">
		<div class="flex h-3.5 gap-0.5 overflow-hidden rounded-full bg-surface-4" aria-hidden="true">
			<template v-for="segment in segments" :key="segment.id">
				<div
					v-if="segment.value > 0"
					v-tooltip="segment.display ? `${segment.label} · ${segment.display}` : segment.label"
					class="h-full min-w-0 bg-gradient-to-b from-white/10 to-black/10 motion-safe:transition-[flex-grow] motion-safe:duration-300"
					:class="segment.color"
					:style="{ flexGrow: segment.value, flexBasis: '0%' }"
				/>
			</template>
			<div v-if="rest > 0" class="min-w-0" :style="{ flexGrow: rest, flexBasis: '0%' }" />
		</div>
		<dl v-if="legend" class="m-0 flex flex-wrap gap-x-6 gap-y-2">
			<div
				v-for="segment in segments"
				:key="segment.id"
				class="flex items-center gap-2 whitespace-nowrap"
			>
				<dt class="flex items-center gap-2">
					<span class="size-3 shrink-0 rounded-full" :class="segment.color" aria-hidden="true" />
					{{ segment.label }}
				</dt>
				<dd class="m-0 font-semibold tabular-nums text-contrast">
					{{ segment.display ?? segment.value }}
				</dd>
			</div>
		</dl>
	</div>
</template>
