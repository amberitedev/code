<template>
	<div v-if="instance" :class="['flex flex-col', isFixedRender && 'h-full']">
		<div :class="['p-6 pr-2 pb-4', isFixedRender && 'shrink-0']">
			<ContentPageHeader>
				<template #icon>
					<Avatar
						:src="icon ?? undefined"
						:alt="instance.name"
						size="64px"
						:tint-by="instance.path"
					/>
				</template>
				<template #title>{{ instance.name }}</template>
				<template #stats>
					<div class="flex items-center flex-wrap gap-2">
						<span class="capitalize font-medium" :class="statusClass">
							{{ coreInstance?.status ?? 'Unknown' }}
						</span>
						<div v-if="coreInstance?.game_version" class="w-1.5 h-1.5 rounded-full bg-surface-5" />
						<span v-if="coreInstance" class="font-medium capitalize">
							{{ coreInstance.loader }} {{ coreInstance.game_version }}
						</span>
					</div>
				</template>
				<template #actions>
					<div class="flex gap-2">
						<ButtonStyled v-if="coreInstance?.status === 'running'" color="red" size="large">
							<button :disabled="actionLoading" @click="handleStop">
								<StopCircleIcon /> Stop
							</button>
						</ButtonStyled>
						<ButtonStyled v-else color="brand" size="large">
							<button :disabled="actionLoading || !coreInstance" @click="handleStart">
								<PlayIcon /> Start
							</button>
						</ButtonStyled>
						<ButtonStyled size="large">
							<button
								:disabled="actionLoading || coreInstance?.status !== 'running'"
								@click="handleRestart"
							>
								<UpdatedIcon /> Restart
							</button>
						</ButtonStyled>
					</div>
				</template>
			</ContentPageHeader>
		</div>

		<div :class="['px-6', isFixedRender && 'shrink-0']">
			<NavTabs :links="tabs" />
		</div>

		<div :class="['p-6 pt-4', isFixedRender && 'min-h-0 flex-1 overflow-y-auto']">
			<RouterView
				v-if="coreInstance"
				:instance="instance"
				:core-instance="coreInstance"
			/>
			<p v-else-if="loadError" class="text-red-400">{{ loadError }}</p>
			<p v-else class="text-secondary">Connecting to Core...</p>
		</div>
	</div>
	<div v-else class="p-6 text-secondary">Loading server...</div>
</template>

<script setup lang="ts">
import { BoxesIcon, PlayIcon, ServerIcon, StopCircleIcon, TerminalSquareIcon, UpdatedIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled, ContentPageHeader, injectNotificationManager, NavTabs } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import { core_get_instance, core_restart, core_start, core_stop } from '@/helpers/core'
import type { CoreInstanceDetail } from '@/helpers/core'
import { get } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/state'

const { handleError } = injectNotificationManager()
const route = useRoute()
const breadcrumbs = useBreadcrumbs()

const instance = ref<GameInstance>()
const coreInstance = ref<CoreInstanceDetail>()
const actionLoading = ref(false)
const loadError = ref('')

const isFixedRender = computed(() => route.meta.renderMode === 'fixed')
const icon = computed(() =>
	instance.value?.icon_path ? convertFileSrc(instance.value.icon_path) : null,
)
const statusClass = computed(() => ({
	'text-green-400': coreInstance.value?.status === 'running',
	'text-red-400': coreInstance.value?.status === 'stopped',
	'text-yellow-400': ['starting', 'stopping'].includes(coreInstance.value?.status ?? ''),
}))

const encodedId = computed(() => encodeURIComponent(route.params.id as string))
const basePath = computed(() => `/server/${encodedId.value}`)
const tabs = computed(() => [
	{ label: 'Overview', href: basePath.value, icon: ServerIcon },
	{ label: 'Console', href: `${basePath.value}/console`, icon: TerminalSquareIcon },
	{ label: 'Content', href: `${basePath.value}/content`, icon: BoxesIcon },
])

async function fetchInstance() {
	instance.value = await get(route.params.id as string).catch(handleError)
	if (instance.value) {
		breadcrumbs.setName('Server', instance.value.name)
		breadcrumbs.setContext({ name: instance.value.name, link: route.path, query: route.query })
	}
}

async function fetchCoreInstance() {
	const coreId = instance.value?.core_instance_id
	if (!coreId) { loadError.value = 'No Core instance linked to this profile.'; return }
	try {
		coreInstance.value = await core_get_instance(coreId)
		loadError.value = ''
	} catch (e) {
		loadError.value = `Core unreachable: ${e}`
	}
}

await fetchInstance()
await fetchCoreInstance()

// Poll Core status every 5 seconds
const pollInterval = setInterval(fetchCoreInstance, 5000)
onUnmounted(() => clearInterval(pollInterval))

async function handleStart() {
	if (!instance.value?.core_instance_id) return
	actionLoading.value = true
	try { await core_start(instance.value.core_instance_id) } catch (e) { handleError(e as Error) }
	actionLoading.value = false
	await fetchCoreInstance()
}

async function handleStop() {
	if (!instance.value?.core_instance_id) return
	actionLoading.value = true
	try { await core_stop(instance.value.core_instance_id) } catch (e) { handleError(e as Error) }
	actionLoading.value = false
	await fetchCoreInstance()
}

async function handleRestart() {
	if (!instance.value?.core_instance_id) return
	actionLoading.value = true
	try { await core_restart(instance.value.core_instance_id) } catch (e) { handleError(e as Error) }
	actionLoading.value = false
	await fetchCoreInstance()
}
</script>
