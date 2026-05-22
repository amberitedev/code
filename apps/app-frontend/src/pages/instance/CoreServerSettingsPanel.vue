<template>
	<div class="flex flex-col gap-6">
		<section class="flex max-w-xl flex-col gap-3">
			<label class="flex flex-col gap-2">
				<span class="text-lg font-semibold text-contrast">Server name</span>
				<StyledInput v-model="serverName" :maxlength="48" wrapper-class="w-full" />
			</label>
			<ButtonStyled color="brand">
				<button :disabled="!canSaveServerName" @click="saveServerName">Save name</button>
			</ButtonStyled>
		</section>

		<section class="flex max-w-xl flex-col gap-3">
			<div class="flex flex-col gap-1">
				<span class="text-lg font-semibold text-contrast">Java version</span>
				<span class="text-secondary"
					>Leave empty to let Core select the required Java version.</span
				>
			</div>
			<StyledInput v-model="javaVersion" inputmode="numeric" wrapper-class="w-full" />
			<ButtonStyled color="brand">
				<button :disabled="savingJavaVersion" @click="saveJavaVersion">Save Java version</button>
			</ButtonStyled>
		</section>

		<section class="flex max-w-xl flex-col gap-3 rounded-2xl bg-surface-2 p-4">
			<div class="flex items-center justify-between gap-4">
				<span class="text-secondary">Core instance ID</span>
				<CopyCode :text="coreInstanceId" />
			</div>
			<div class="flex items-center justify-between gap-4">
				<span class="text-secondary">Port</span>
				<span class="font-semibold text-contrast">{{ coreInstance?.port ?? 'Unknown' }}</span>
			</div>
			<div class="flex items-center justify-between gap-4">
				<span class="text-secondary">Memory</span>
				<span class="font-semibold text-contrast">
					{{ coreInstance?.memory.min_mb ?? 0 }}-{{ coreInstance?.memory.max_mb ?? 0 }} MB
				</span>
			</div>
		</section>
	</div>
</template>

<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/amberite-api'
import {
	ButtonStyled,
	CopyCode,
	injectCoreClient,
	injectCoreInstanceState,
	injectNotificationManager,
	StyledInput,
} from '@modrinth/ui'
import { computed, onUnmounted, ref, watch } from 'vue'

import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
	instance: GameInstance
}>()

const coreClient = injectCoreClient()
const coreInstances = injectCoreInstanceState()
const { handleError } = injectNotificationManager()

const coreSnapshot = ref(coreInstances.snapshot)
const unlistenCoreInstances = coreInstances.subscribe((snapshot) => {
	coreSnapshot.value = snapshot
})
const coreInstanceId = computed(() => props.instance.core_instance_id ?? '')
const coreInstance = computed<CoreInstanceSummary | null>(
	() => coreSnapshot.value.instances.find((item) => item.id === coreInstanceId.value) ?? null,
)
const serverName = ref(props.instance.name)
const javaVersion = ref('')
const savingName = ref(false)
const savingJavaVersion = ref(false)
const canSaveServerName = computed(
	() =>
		serverName.value.trim().length > 0 &&
		serverName.value.trim() !== coreInstance.value?.name &&
		!savingName.value,
)

watch(
	() => coreInstance.value?.name ?? props.instance.name,
	(name) => {
		serverName.value = name
	},
	{ immediate: true },
)

onUnmounted(() => unlistenCoreInstances())

async function saveServerName() {
	if (!coreInstanceId.value || !canSaveServerName.value) return
	savingName.value = true
	try {
		await coreClient.renameInstance(coreInstanceId.value, serverName.value.trim())
		await coreInstances.refresh()
	} catch (error) {
		handleError(error as Error)
	} finally {
		savingName.value = false
	}
}

async function saveJavaVersion() {
	if (!coreInstanceId.value) return
	savingJavaVersion.value = true
	const parsed = Number(javaVersion.value.trim())
	try {
		await coreClient.updateJavaVersion(
			coreInstanceId.value,
			javaVersion.value.trim() && Number.isFinite(parsed) ? parsed : null,
		)
		await coreInstances.refresh()
	} catch (error) {
		handleError(error as Error)
	} finally {
		savingJavaVersion.value = false
	}
}
</script>
