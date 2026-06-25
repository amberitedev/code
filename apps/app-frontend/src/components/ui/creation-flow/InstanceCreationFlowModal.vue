<template>
	<MultiStageModal
		ref="modal"
		:stages="ctx.stageConfigs"
		:context="ctx"
		:fade="fade"
		disable-progress
		@hide="handleHide"
	/>
</template>

<script setup lang="ts">
import { LeftArrowIcon, RightArrowIcon } from '@modrinth/assets'
import {
	commonMessages,
	defineMessages,
	MultiStageModal,
	type CreationFlowContextValue,
	type StageConfigInput,
} from '@modrinth/ui'
import {
	createCreationFlowContext,
	type FlowType,
	type LoaderManifestResolver,
	type ModpackSearchResult,
	provideCreationFlowContext,
} from '@modrinth/ui/src/components/flows/creation-flow-modal/creation-flow-context'
import { computed, markRaw, ref, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import InstanceTypeStage from './InstanceTypeStage.vue'
import type { InstanceCreationFlowContextValue, InstanceType } from './types'

const props = withDefaults(
	defineProps<{
		type?: FlowType
		availableLoaders?: string[]
		showSnapshotToggle?: boolean
		disableClose?: boolean
		isInitialSetup?: boolean
		initialLoader?: string
		initialGameVersion?: string
		fetchExistingInstanceNames?: () => Promise<string[]>
		onBack?: (() => void) | null
		fade?: 'standard' | 'warning' | 'danger'
		searchModpacks?: (query: string, limit?: number) => Promise<ModpackSearchResult>
		getProjectVersions?: (projectId: string) => Promise<{ id: string }[]>
		getLoaderManifest?: LoaderManifestResolver
		finishDisabled?: boolean
		finishDisabledTooltip?: string
	}>(),
	{
		type: 'instance',
		availableLoaders: () => ['fabric', 'neoforge', 'forge', 'quilt'],
		showSnapshotToggle: false,
		disableClose: false,
		isInitialSetup: false,
		initialLoader: undefined,
		initialGameVersion: undefined,
		fetchExistingInstanceNames: undefined,
		onBack: null,
	},
)

const emit = defineEmits<{
	(e: 'hide' | 'browse-modpacks'): void
	(e: 'create', config: InstanceCreationFlowContextValue): void
}>()

const modal = useTemplateRef<ComponentExposed<typeof MultiStageModal>>('modal')
const instanceType = ref<InstanceType>('client')

const messages = defineMessages({
	title: {
		id: 'app.creation-flow.instance-type.title',
		defaultMessage: 'Create instance',
	},
})

const ctx = createCreationFlowContext(
	modal,
	props.type,
	{
		browseModpacks: () => emit('browse-modpacks'),
		create: (config) => emit('create', config as InstanceCreationFlowContextValue),
	},
	{
		availableLoaders: props.availableLoaders,
		showSnapshotToggle: props.showSnapshotToggle,
		disableClose: props.disableClose,
		isInitialSetup: props.isInitialSetup,
		initialLoader: props.initialLoader,
		initialGameVersion: props.initialGameVersion,
		fetchExistingInstanceNames: props.fetchExistingInstanceNames,
		onBack: props.onBack ?? undefined,
		searchModpacks: props.searchModpacks,
		getProjectVersions: props.getProjectVersions,
		getLoaderManifest: props.getLoaderManifest,
		finishDisabled: computed(() => props.finishDisabled ?? false),
		finishDisabledTooltip: computed(() => props.finishDisabledTooltip),
	},
) as InstanceCreationFlowContextValue

ctx.instanceType = instanceType
ctx.stageConfigs = [...ctx.stageConfigs]

const instanceTypeStage: StageConfigInput<CreationFlowContextValue> = {
	id: 'app-instance-type',
	title: (context) => context.formatMessage(messages.title),
	stageContent: markRaw(InstanceTypeStage),
	skip: (context) =>
		context.flowType !== 'instance' ||
		context.setupType.value !== 'custom' ||
		context.isImportMode.value,
	disableClose: props.disableClose,
	leftButtonConfig: (context) => ({
		label: context.formatMessage(commonMessages.backButton),
		icon: LeftArrowIcon,
		onClick: () => context.modal.value?.setStage('setup-type'),
	}),
	rightButtonConfig: (context) => ({
		label: context.formatMessage(commonMessages.continueButton),
		icon: RightArrowIcon,
		iconPosition: 'after',
		onClick: () => context.modal.value?.setStage('custom-setup'),
	}),
	maxWidth: '520px',
}

const customSetupStageIndex = ctx.stageConfigs.findIndex((stage) => stage.id === 'custom-setup')
if (customSetupStageIndex !== -1) {
	ctx.stageConfigs.splice(customSetupStageIndex, 0, instanceTypeStage)
}

const setSetupType = ctx.setSetupType
ctx.setSetupType = (type) => {
	setSetupType(type)
	if (ctx.flowType === 'instance' && type === 'custom') {
		ctx.modal.value?.setStage('app-instance-type')
	}
}

provideCreationFlowContext(ctx)

async function show() {
	instanceType.value = 'client'
	await ctx.reset()
	void ctx.prefetchLoaderMetadata()
	modal.value?.setStage(0)
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

function handleHide() {
	ctx.cancelBackup.value?.()
	emit('hide')
}

defineExpose({ show, hide, ctx })
</script>
