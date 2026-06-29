<template>
	<component :is="props.as" :class="props.className" :aria-label="resolvedChildren" :style="props.style">
		<AnimatePresence mode="popLayout" :initial="false">
			<Motion
				v-for="character in characters"
				:key="character.id"
				as="span"
				:layout-id="character.id"
				class="inline-block"
				aria-hidden="true"
				initial="initial"
				animate="animate"
				exit="exit"
				:variants="props.variants || defaultVariants"
				:transition="props.transition || defaultTransition"
			>
				{{ character.label }}
			</Motion>
		</AnimatePresence>
	</component>
</template>

<script setup lang="ts">
import { AnimatePresence, Motion, type Options } from 'motion-v'
import { computed, useId, useSlots, type Component, type CSSProperties, type VNodeChild } from 'vue'

export type TextMorphVariants = NonNullable<Options['variants']>
export type TextMorphTransition = NonNullable<Options['transition']>

const props = withDefaults(
	defineProps<{
		children?: string
		as?: string | Component
		className?: string
		style?: CSSProperties
		variants?: TextMorphVariants
		transition?: TextMorphTransition
	}>(),
	{
		as: 'p',
	},
)

const slots = useSlots()
const uniqueId = useId()

const defaultVariants: TextMorphVariants = {
	initial: { opacity: 0 },
	animate: { opacity: 1 },
	exit: { opacity: 0 },
}

const defaultTransition: TextMorphTransition = {
	type: 'spring',
	stiffness: 280,
	damping: 18,
	mass: 0.3,
}

function resolveText(children: VNodeChild): string {
	if (children === null || children === undefined || typeof children === 'boolean') return ''
	if (typeof children === 'string' || typeof children === 'number') return String(children)
	if (Array.isArray(children)) return children.map((child) => resolveText(child)).join('')
	if (typeof children === 'object' && 'children' in children) {
		return resolveText(children.children as VNodeChild)
	}

	return ''
}

const slotChildren = computed(() => resolveText(slots.default?.() ?? []))
const resolvedChildren = computed(() => props.children ?? slotChildren.value)

const characters = computed(() => {
	const charCounts: Record<string, number> = {}

	return resolvedChildren.value.split('').map((char) => {
		const lowerChar = char.toLowerCase()
		charCounts[lowerChar] = (charCounts[lowerChar] || 0) + 1

		return {
			id: `${uniqueId}-${lowerChar}${charCounts[lowerChar]}`,
			label: char === ' ' ? '\u00A0' : char,
		}
	})
})

</script>
