<template>
	<UiMotionTransition
		:content-key="contentKey"
		:config="motionConfig"
		:visible="visible"
		@before-enter="(el) => emit('before-enter', el)"
		@before-leave="(el) => emit('before-leave', el)"
		@after-enter="(el) => emit('after-enter', el)"
		@after-leave="(el) => emit('after-leave', el)"
		@enter-cancelled="(el) => emit('enter-cancelled', el)"
		@leave-cancelled="(el) => emit('leave-cancelled', el)"
	>
		<slot />
	</UiMotionTransition>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import {
	type UiMotionConfig,
	type UiMotionDirection,
	type UiMotionEasingPreset,
	type UiMotionMode,
	type UiMotionType,
	UI_MOTION_PRESETS,
} from '#ui/composables/ui-motion'
import { useNavTabContentTransitionTiming } from '#ui/composables/nav-tab-transition'

import UiMotionTransition from './UiMotionTransition.vue'

const props = withDefaults(
	defineProps<{
		contentKey?: string | number
		direction?: UiMotionDirection
		type?: UiMotionType
		enterMs?: number
		leaveMs?: number
		easing?: UiMotionEasingPreset | string
		enterEasing?: string
		leaveEasing?: string
		distance?: string
		mode?: UiMotionMode
		enabled?: boolean
		lockHeight?: boolean
		freezeLeave?: boolean
		safetyMs?: number
		visible?: boolean
		config?: UiMotionConfig
	}>(),
	{
		contentKey: '',
		direction: undefined,
		type: undefined,
		enterMs: undefined,
		leaveMs: undefined,
		easing: undefined,
		enterEasing: undefined,
		leaveEasing: undefined,
		distance: undefined,
		mode: undefined,
		enabled: undefined,
		lockHeight: undefined,
		freezeLeave: undefined,
		safetyMs: undefined,
		visible: true,
		config: undefined,
	},
)

const emit = defineEmits<{
	(e: 'before-enter', el: Element): void
	(e: 'before-leave', el: Element): void
	(e: 'after-enter', el: Element): void
	(e: 'after-leave', el: Element): void
	(e: 'enter-cancelled', el: Element): void
	(e: 'leave-cancelled', el: Element): void
}>()

const timing = useNavTabContentTransitionTiming()
const motionConfig = computed<UiMotionConfig>(() => ({
	...UI_MOTION_PRESETS.tabSlide,
	...props.config,
	direction: props.direction ?? props.config?.direction ?? UI_MOTION_PRESETS.tabSlide.direction,
	type: props.type ?? props.config?.type ?? UI_MOTION_PRESETS.tabSlide.type,
	enterMs: props.enterMs ?? props.config?.enterMs ?? timing.durationMs.value,
	leaveMs: props.leaveMs ?? props.config?.leaveMs ?? timing.durationMs.value,
	easing: props.easing ?? props.config?.easing ?? UI_MOTION_PRESETS.tabSlide.easing,
	enterEasing: props.enterEasing ?? props.config?.enterEasing ?? timing.enterEasing,
	leaveEasing: props.leaveEasing ?? props.config?.leaveEasing ?? timing.leaveEasing,
	distance: props.distance ?? props.config?.distance ?? UI_MOTION_PRESETS.tabSlide.distance,
	mode: props.mode ?? props.config?.mode ?? UI_MOTION_PRESETS.tabSlide.mode,
	enabled: props.enabled ?? props.config?.enabled ?? true,
	safetyMs: props.safetyMs ?? props.config?.safetyMs ?? timing.safetyMs.value,
	lockHeight: props.lockHeight ?? props.config?.lockHeight ?? true,
	freezeLeave: props.freezeLeave ?? props.config?.freezeLeave ?? true,
}))
</script>
