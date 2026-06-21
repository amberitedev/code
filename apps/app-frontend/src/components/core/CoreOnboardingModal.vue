<script setup lang="ts">
import { CheckIcon, RightArrowIcon } from '@modrinth/assets'
import type { StageConfigInput } from '@modrinth/ui'
import { MultiStageModal } from '@modrinth/ui'
import { ref } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import {
	type CoreOnboardingContext,
	type CoreOnboardingFlow,
	provideCoreOnboardingContext,
} from './core-onboarding-context'
import AdvancedStage from './stages/AdvancedStage.vue'
import CodeStage from './stages/CodeStage.vue'
import GeneralStage from './stages/GeneralStage.vue'
import MembersStage from './stages/MembersStage.vue'
import { useCoreOnboardingState } from './use-core-onboarding-state'

const modal = ref<ComponentExposed<typeof MultiStageModal> | null>(null)
const onboarding = useCoreOnboardingState(modal)

provideCoreOnboardingContext(onboarding.ctx)

const stages: StageConfigInput<CoreOnboardingContext>[] = [
	{
		id: 'connect',
		title: 'Connect',
		stageContent: CodeStage,
		skip: (ctx) => ctx.flow.value !== 'connect',
		cannotNavigateForward: () => !onboarding.connectValidated.value,
		leftButtonConfig: null,
		rightButtonConfig: (ctx) => ({
			label: 'Connect',
			icon: RightArrowIcon,
			iconPosition: 'after',
			color: 'brand',
			disabled: ctx.connectCode.value.replace(/[^A-Z0-9]/gi, '').length !== 8,
			loading: ctx.working.value,
			onClick: ctx.working.value ? undefined : onboarding.validateConnectAndContinue,
		}),
		maxWidth: '800px',
	},
	{
		id: 'general',
		title: 'General',
		stageContent: GeneralStage,
		leftButtonConfig: (ctx) =>
			ctx.flow.value === 'create' ? null : { label: 'Back', onClick: () => modal.value?.prevStage() },
		rightButtonConfig: {
			label: 'Next',
			icon: RightArrowIcon,
			iconPosition: 'after',
			color: 'brand',
			onClick: () => modal.value?.nextStage(),
		},
		maxWidth: '800px',
	},
	{
		id: 'members',
		title: 'Members',
		stageContent: MembersStage,
		leftButtonConfig: { label: 'Back', onClick: () => modal.value?.prevStage() },
		rightButtonConfig: {
			label: 'Next',
			icon: RightArrowIcon,
			iconPosition: 'after',
			color: 'brand',
			onClick: () => modal.value?.nextStage(),
		},
		maxWidth: '800px',
	},
	{
		id: 'advanced',
		title: 'Advanced',
		stageContent: AdvancedStage,
		leftButtonConfig: { label: 'Back', onClick: () => modal.value?.prevStage() },
		rightButtonConfig: (ctx) => ({
			label: 'Finish',
			icon: CheckIcon,
			iconPosition: 'before',
			color: 'brand',
			loading: ctx.working.value,
			onClick: ctx.working.value ? undefined : onboarding.finish,
		}),
		maxWidth: '800px',
	},
]

function show(flow: CoreOnboardingFlow) {
	onboarding.show(flow)
}

defineExpose({ show })
</script>

<template>
	<MultiStageModal ref="modal" :stages="stages" :context="onboarding.ctx" breadcrumbs />
</template>
