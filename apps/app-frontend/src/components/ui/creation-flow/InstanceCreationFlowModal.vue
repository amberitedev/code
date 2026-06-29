<template>
	<NewModal
		ref="modalRoot"
		:scrollable="true"
		max-content-height="72vh"
		:on-hide="handleHide"
		:closable="true"
		:close-on-click-outside="true"
		:width="resolvedMaxWidth"
		:fade="fade"
		:disable-close="resolveCtxFn(currentStage.disableClose ?? false, ctx)"
	>
		<template #title>
			<span class="text-lg font-bold text-contrast sm:text-xl">{{ resolvedTitle }}</span>
		</template>

		<div class="create-instance-stage">
			<StageContentTransition :content-key="currentStage.id" :active-index="currentStageIndex">
				<div class="create-instance-stage-frame">
					<component :is="currentStage.stageContent" />

					<div v-if="leftButtonConfig || rightButtonConfig" class="create-instance-actions mt-4">
						<div class="create-instance-actions-left">
							<ButtonStyled v-if="leftButtonConfig" type="outlined">
								<button
									v-tooltip="leftButtonConfig.tooltip"
									:class="leftButtonConfig.buttonClass"
									:disabled="leftButtonConfig.disabled"
									@click="leftButtonConfig.onClick"
								>
									<component :is="leftButtonConfig.icon" />
									{{ leftButtonConfig.label }}
								</button>
							</ButtonStyled>
						</div>
						<div class="create-instance-actions-right">
							<ButtonStyled v-if="rightButtonConfig" :color="rightButtonConfig.color">
								<button
									v-tooltip="rightButtonConfig.tooltip"
									class="!shadow-none"
									:class="rightButtonConfig.buttonClass"
									:disabled="rightButtonConfig.disabled || rightButtonConfig.loading"
									@click="rightButtonConfig.onClick"
								>
									<SpinnerIcon
										v-if="rightButtonConfig.loading && rightButtonConfig.iconPosition === 'before'"
										class="animate-spin"
									/>
									<component
										:is="rightButtonConfig.icon"
										v-else-if="rightButtonConfig.iconPosition === 'before'"
										:class="rightButtonConfig.iconClass"
									/>
									{{ rightButtonConfig.label }}
									<SpinnerIcon
										v-if="rightButtonConfig.loading && rightButtonConfig.iconPosition === 'after'"
										class="animate-spin"
									/>
									<component
										:is="rightButtonConfig.icon"
										v-else-if="rightButtonConfig.iconPosition === 'after'"
										:class="rightButtonConfig.iconClass"
									/>
								</button>
							</ButtonStyled>
						</div>
					</div>
				</div>
			</StageContentTransition>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import { LeftArrowIcon, RightArrowIcon, SpinnerIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	commonMessages,
	defineMessages,
	NewModal,
	resolveCtxFn,
	StageContentTransition,
	type CreationFlowContextValue,
	type StageButtonConfig,
	type StageConfigInput,
} from '@modrinth/ui'
import type MultiStageModal from '@modrinth/ui/src/components/base/MultiStageModal.vue'
import {
	createCreationFlowContext,
	type FlowType,
	type LoaderManifestResolver,
	type ModpackSearchResult,
	provideCreationFlowContext,
} from '@modrinth/ui/src/components/flows/creation-flow-modal/creation-flow-context'
import { computed, markRaw, ref, shallowRef, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import InstanceTypeStage from './InstanceTypeStage.vue'
import type {
	InstanceCreationFlowContextValue,
	InstanceType,
	InstanceTypeClickBehavior,
} from './types'

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
		instanceTypeClickBehavior?: InstanceTypeClickBehavior
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
		instanceTypeClickBehavior: 'continue',
	},
)

const emit = defineEmits<{
	(e: 'hide' | 'show' | 'browse-modpacks'): void
	(e: 'create', config: InstanceCreationFlowContextValue): void
}>()

const modalRoot = useTemplateRef<InstanceType<typeof NewModal>>('modalRoot')
const modal = shallowRef<ComponentExposed<typeof MultiStageModal> | null>(null)
const currentStageIndex = ref(0)
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
ctx.instanceTypeClickBehavior = computed(() => props.instanceTypeClickBehavior)
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
	rightButtonConfig: (context) => {
		const appContext = context as InstanceCreationFlowContextValue
		if (appContext.instanceTypeClickBehavior.value === 'continue') {
			return null
		}

		return {
			label: context.formatMessage(commonMessages.continueButton),
			icon: RightArrowIcon,
			iconPosition: 'after' as const,
			onClick: () => context.modal.value?.setStage('custom-setup'),
		}
	},
	maxWidth: '520px',
}

const customSetupStageIndex = ctx.stageConfigs.findIndex((stage) => stage.id === 'custom-setup')
if (customSetupStageIndex !== -1) {
	ctx.stageConfigs.splice(customSetupStageIndex, 0, instanceTypeStage)
}

const localCustomSetupStageIndex = ctx.stageConfigs.findIndex(
	(stage) => stage.id === 'custom-setup',
)
if (localCustomSetupStageIndex !== -1) {
	const customSetupStage = ctx.stageConfigs[localCustomSetupStageIndex]!
	const originalLeftButtonConfig = customSetupStage.leftButtonConfig
	ctx.stageConfigs[localCustomSetupStageIndex] = {
		...customSetupStage,
		leftButtonConfig: (context) => {
			if (
				context.flowType === 'instance' &&
				context.setupType.value === 'custom' &&
				!context.isImportMode.value
			) {
				return {
					label: context.formatMessage(commonMessages.backButton),
					icon: LeftArrowIcon,
					onClick: () => context.modal.value?.setStage('app-instance-type'),
				}
			}

			return resolveCtxFn(originalLeftButtonConfig, context)
		},
	}
}

const setSetupType = ctx.setSetupType
ctx.setSetupType = (type) => {
	if (ctx.flowType === 'instance' && type === 'custom') {
		ctx.isImportMode.value = false
		ctx.setupType.value = type
		ctx.modpackSelection.value = null
		ctx.modpackFile.value = null
		ctx.modpackFilePath.value = null
		ctx.modal.value?.setStage('app-instance-type')
		return
	}

	setSetupType(type)
}

provideCreationFlowContext(ctx)

const currentStage = computed<StageConfigInput<CreationFlowContextValue>>(
	() => ctx.stageConfigs[currentStageIndex.value] ?? ctx.stageConfigs[0]!,
)
const resolvedTitle = computed(() => resolveCtxFn(currentStage.value.title, ctx))
const leftButtonConfig = computed<StageButtonConfig | null>(() =>
	resolveCtxFn(currentStage.value.leftButtonConfig, ctx),
)
const rightButtonConfig = computed<StageButtonConfig | null>(() =>
	resolveCtxFn(currentStage.value.rightButtonConfig, ctx),
)
const resolvedMaxWidth = computed(() =>
	currentStage.value.maxWidth ? resolveCtxFn(currentStage.value.maxWidth, ctx) : '560px',
)

function setStage(indexOrId: number | string) {
	let index = 0
	if (typeof indexOrId === 'number') {
		index = indexOrId
		if (index < 0 || index >= ctx.stageConfigs.length) return
	} else {
		index = ctx.stageConfigs.findIndex((stage) => stage.id === indexOrId)
		if (index === -1) return
	}

	while (index < ctx.stageConfigs.length) {
		const skip = ctx.stageConfigs[index]?.skip
		if (!skip || !resolveCtxFn(skip, ctx)) break
		index++
	}

	if (index < ctx.stageConfigs.length) {
		currentStageIndex.value = index
	}
}

function nextStage() {
	if (currentStageIndex.value >= ctx.stageConfigs.length - 1) return

	let nextIndex = currentStageIndex.value + 1
	while (nextIndex < ctx.stageConfigs.length) {
		const skip = ctx.stageConfigs[nextIndex]?.skip
		if (!skip || !resolveCtxFn(skip, ctx)) break
		nextIndex++
	}

	if (nextIndex < ctx.stageConfigs.length) {
		currentStageIndex.value = nextIndex
	}
}

function prevStage() {
	if (currentStageIndex.value <= 0) return

	let prevIndex = currentStageIndex.value - 1
	while (prevIndex >= 0) {
		const skip = ctx.stageConfigs[prevIndex]?.skip
		if (!skip || !resolveCtxFn(skip, ctx)) break
		prevIndex--
	}

	if (prevIndex >= 0) {
		currentStageIndex.value = prevIndex
	}
}

function showModal() {
	modalRoot.value?.show()
}

function hideModal() {
	modalRoot.value?.hide()
}

modal.value = {
	show: showModal,
	hide: hideModal,
	setStage,
	nextStage,
	prevStage,
	currentStageIndex,
} as ComponentExposed<typeof MultiStageModal>

async function show() {
	instanceType.value = 'client'
	await ctx.reset()
	void ctx.prefetchLoaderMetadata()
	modal.value?.setStage(0)
	modal.value?.show()
	emit('show')
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

<style scoped>
.create-instance-stage {
	display: grid;
	min-width: 0;
	overflow: hidden;
	box-sizing: border-box;
}

.create-instance-stage-frame {
	box-sizing: border-box;
	min-width: 0;
	width: 100%;
	padding-bottom: 1rem;
}

.create-instance-actions {
	display: flex;
	align-items: center;
	justify-content: space-between;
	gap: 0.5rem;
}

.create-instance-actions-left,
.create-instance-actions-right {
	display: flex;
	min-width: 0;
}

.create-instance-actions-right {
	margin-left: auto;
}
</style>
