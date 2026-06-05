<script setup lang="ts">
import { SpinnerIcon, TagCategoryWifiOffIcon, TriangleAlertIcon } from '@modrinth/assets'
import { ButtonStyled, NavTabs } from '@modrinth/ui'
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import AppPageSkeleton from '@/components/ui/AppPageSkeleton.vue'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CorePage' })

const route = useRoute()
const { group, currentUser, loading, refresh } = useSocial()
const { status, error: connectionError, check } = useCoreConnection()
const probePending = ref(false)
let probeId = 0

const initialPending = computed(() => loading.value && currentUser.value === null)
const hasGroup = computed(() => group.value !== null)
const offline = computed(
	() => hasGroup.value && !probePending.value && status.value?.state === 'disconnected',
)
const coreUrl = computed(() => status.value?.coreUrl ?? group.value?.core?.connectionUrl ?? null)

function wait(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms))
}

async function probeCore() {
	const currentProbe = ++probeId
	if (!hasGroup.value) {
		probePending.value = false
		return
	}
	probePending.value = true
	await Promise.allSettled([check(), wait(3500)])
	if (currentProbe === probeId) probePending.value = false
}

async function retry() {
	await refresh()
	await probeCore()
}

onMounted(() => {
	if (hasGroup.value) void probeCore()
})

watch(
	() => group.value?.group.id,
	() => void probeCore(),
)
</script>

<template>
	<div class="p-6 flex flex-col gap-4 max-w-[80rem] mx-auto w-full min-h-0">
		<AppPageSkeleton v-if="initialPending" variant="list" class="!p-0" />
		<section v-else-if="probePending" class="flex min-h-80 flex-col gap-4">
			<div class="flex items-center gap-3 rounded-2xl bg-surface-3 p-5 text-secondary">
				<SpinnerIcon class="size-5 animate-spin text-brand" />
				<span>Checking Core connection...</span>
			</div>
			<AppPageSkeleton variant="list" class="!p-0" />
		</section>
		<section v-else-if="offline" class="rounded-2xl bg-surface-3 p-6">
			<div class="flex flex-col gap-5 md:flex-row md:items-start md:justify-between">
				<div class="flex gap-4">
					<div class="flex size-12 shrink-0 items-center justify-center rounded-2xl bg-bg-red text-red">
						<TagCategoryWifiOffIcon />
					</div>
					<div>
						<div class="mb-2 flex items-center gap-2 text-sm font-bold text-red">
							<TriangleAlertIcon /> Core unreachable
						</div>
						<h1 class="m-0 text-3xl font-black text-contrast">This Core is offline</h1>
						<p class="m-0 mt-2 max-w-2xl text-secondary">
							The app found your Core account, but it could not reach the Core server after a
							connection probe. Start Core on its host machine and try again.
						</p>
						<p v-if="coreUrl" class="m-0 mt-3 font-mono text-sm text-secondary">{{ coreUrl }}</p>
						<p v-if="connectionError" class="m-0 mt-2 text-sm text-red">
							{{ connectionError.message }}
						</p>
					</div>
				</div>
				<ButtonStyled color="brand">
					<button @click="retry">
						<SpinnerIcon v-if="probePending" class="animate-spin" />
						<TriangleAlertIcon v-else />
						Retry
					</button>
				</ButtonStyled>
			</div>
		</section>
		<template v-else>
			<NavTabs
				:links="[
					{ label: 'Overview', href: `/core` },
					{ label: 'Members', href: `/core/members`, shown: hasGroup },
					{ label: 'Servers', href: `/core/servers`, shown: hasGroup },
					{ label: 'Settings', href: `/core/settings`, shown: hasGroup },
				]"
			/>
			<RouterView v-if="route.path.startsWith('/core')" class="flex-1 min-h-0" />
		</template>
	</div>
</template>
