<template>
	<TabView v-model="side" :tabs="tabs">
		<template #server>
			<ServerBackups />
		</template>
		<template #client>
			<Worlds
				:instance="instance"
				:options="null"
				:offline="offline"
				:playing="playing"
				:installed="false"
			/>
		</template>
	</TabView>
</template>

<script setup lang="ts">
import TabView from '@/components/ui/TabView.vue'
import type { GameInstance } from '@/helpers/types'

import ServerBackups from '../ServerBackups.vue'
import Worlds from '../Worlds.vue'
import { useSyncedSideTabs } from './use-synced-permissions'

defineProps<{
	instance: GameInstance
	offline: boolean
	playing: boolean
}>()

const { side, tabs } = useSyncedSideTabs()
</script>
