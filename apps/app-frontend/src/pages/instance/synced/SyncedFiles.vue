<template>
	<TabView v-model="side" :tabs="tabs">
		<template #server>
			<ServerFiles />
		</template>
		<template #client>
			<Files
				:instance="instance"
				:options="null"
				:offline="offline"
				:playing="playing"
				:installed="false"
				:is-server-instance="false"
			/>
		</template>
	</TabView>
</template>

<script setup lang="ts">
import TabView from '@/components/ui/TabView.vue'
import type { GameInstance } from '@/helpers/types'

import Files from '../Files.vue'
import ServerFiles from '../ServerFiles.vue'
import { useSyncedSideTabs } from './use-synced-permissions'

defineProps<{
	instance: GameInstance
	offline: boolean
	playing: boolean
}>()

const { side, tabs } = useSyncedSideTabs()
</script>
