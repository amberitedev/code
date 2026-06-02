<script setup lang="ts">
import { ButtonStyled, NavTabs } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import { useCorePreview } from '@/components/core/use-core-preview'
import AppPageSkeleton from '@/components/ui/AppPageSkeleton.vue'
import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CorePage' })

const route = useRoute()
const { group, currentUser, loading } = useSocial()
const { previewState, isPreviewConnected, setPreviewState } = useCorePreview()

const initialPending = computed(() => loading.value && currentUser.value === null)
const hasGroup = computed(() => group.value !== null || isPreviewConnected.value)
</script>

<template>
	<div class="p-6 flex flex-col gap-4 max-w-6xl mx-auto w-full min-h-0">
		<AppPageSkeleton v-if="initialPending" variant="list" class="!p-0" />
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
		<div class="fixed bottom-5 right-5 z-50 flex gap-2 rounded-2xl bg-surface-3 p-2 shadow-lg">
			<ButtonStyled :color="previewState === 'setup' ? 'brand' : undefined" size="small">
				<button @click="setPreviewState('setup')">Setup</button>
			</ButtonStyled>
			<ButtonStyled :color="previewState !== 'setup' ? 'brand' : undefined" size="small">
				<button @click="setPreviewState('local')">Dashboard</button>
			</ButtonStyled>
		</div>
	</div>
</template>
