<template>
	<TabView v-model="side" :tabs="tabs">
		<template #server>
			<ServerContent />
		</template>
		<template #client>
			<Mods :instance="instance" :is-server-instance="false" :preloaded-content="null" />
		</template>
	</TabView>
</template>

<script setup lang="ts">
import TabView from '@/components/ui/TabView.vue'
import type { GameInstance } from '@/helpers/types'

import Mods from '../Mods.vue'
import ServerContent from '../ServerContent.vue'
import { useSyncedSideTabs } from './use-synced-permissions'

defineProps<{
	instance: GameInstance
	offline?: boolean
	playing?: boolean
}>()

const { side, tabs } = useSyncedSideTabs()
</script>
