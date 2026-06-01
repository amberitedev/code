<template>
	<TabView v-model="side" :tabs="tabs">
		<template #server>
			<ServerOverview />
		</template>
		<template #client>
			<div class="flex flex-col gap-4">
				<div class="flex items-center gap-4 rounded-2xl bg-bg-raised p-4">
					<Avatar
						:src="icon ?? undefined"
						:alt="instance.name"
						size="48px"
						:tint-by="instance.path"
					/>
					<div class="flex flex-col">
						<span class="text-lg font-bold text-contrast">{{ instance.name }}</span>
						<span class="text-sm font-medium capitalize text-secondary">
							{{ instance.loader }} {{ instance.game_version }}
						</span>
					</div>
				</div>
				<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
					<div class="flex flex-col gap-1 rounded-2xl bg-bg-raised p-4">
						<span class="text-xs font-bold uppercase text-secondary">Time played</span>
						<span class="text-base font-semibold text-contrast">{{ timePlayedLabel }}</span>
					</div>
					<div class="flex flex-col gap-1 rounded-2xl bg-bg-raised p-4">
						<span class="text-xs font-bold uppercase text-secondary">Last played</span>
						<span class="text-base font-semibold text-contrast">{{ lastPlayedLabel }}</span>
					</div>
				</div>
			</div>
		</template>
	</TabView>
</template>

<script setup lang="ts">
import { Avatar } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import dayjs from 'dayjs'
import duration from 'dayjs/plugin/duration'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed } from 'vue'

import TabView from '@/components/ui/TabView.vue'
import type { GameInstance } from '@/helpers/types'

import ServerOverview from '../ServerOverview.vue'
import { useSyncedSideTabs } from './use-synced-permissions'

dayjs.extend(duration)
dayjs.extend(relativeTime)

const props = defineProps<{
	instance: GameInstance
	offline?: boolean
	playing?: boolean
}>()

const { side, tabs } = useSyncedSideTabs()

const icon = computed(() =>
	props.instance.icon_path ? convertFileSrc(props.instance.icon_path) : null,
)

const timePlayedLabel = computed(() => {
	const seconds =
		(props.instance.recent_time_played ?? 0) + (props.instance.submitted_time_played ?? 0)
	if (seconds <= 0) return 'Never played'
	return dayjs.duration(seconds, 'seconds').humanize()
})

const lastPlayedLabel = computed(() =>
	props.instance.last_played ? dayjs(props.instance.last_played).fromNow() : 'Never',
)
</script>
