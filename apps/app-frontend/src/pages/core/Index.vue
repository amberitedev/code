<script setup lang="ts">
import { NavTabs } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import AppPageSkeleton from '@/components/ui/AppPageSkeleton.vue'
import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CorePage' })

const route = useRoute()
const { group, currentUser, loading } = useSocial()

const initialPending = computed(() => loading.value && currentUser.value === null)
const hasGroup = computed(() => group.value !== null)
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
					{ label: 'Core Settings', href: `/core/settings`, shown: hasGroup },
					{ label: 'Setup', href: `/core/setup` },
				]"
			/>
			<RouterView v-if="route.path.startsWith('/core')" class="flex-1 min-h-0" />
		</template>
	</div>
</template>
