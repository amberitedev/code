<script setup lang="ts">
import { BoxIcon, GameIcon, getLoaderIcon } from '@modrinth/assets'
import { formatLoader, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { SharedInstanceVersionDetails } from '@/helpers/install'

import { changeOf } from './changes'

// What a version runs on and how much it holds. A game version or loader the version moved to is
// bright; `arrows` also shows what it moved from.

const props = defineProps<{
	details: SharedInstanceVersionDetails
	/** False for the oldest kept version, which has nothing before it to change. */
	compare: boolean
	arrows?: boolean
}>()

const { formatMessage } = useVIntl()

const game = computed(() => props.compare && changeOf(props.details.diffs, 'game_version_updated'))
const loader = computed(() => props.compare && changeOf(props.details.diffs, 'loader_updated'))
const loaderLabel = computed(
	() => `${formatLoader(formatMessage, props.details.loader)} ${props.details.loaderVersion}`,
)
</script>

<template>
	<span class="flex items-center gap-5 whitespace-nowrap text-secondary">
		<span
			v-tooltip="'Minecraft'"
			class="flex items-center gap-1.5"
			:class="{ 'font-semibold text-contrast': game }"
		>
			<GameIcon class="size-4" aria-hidden="true" />
			<template v-if="arrows && game && game.from">{{ game.from }} → </template>
			{{ details.gameVersion }}
		</span>
		<span
			v-tooltip="loaderLabel"
			class="flex items-center gap-1.5"
			:class="{ 'font-semibold text-contrast': loader }"
		>
			<component :is="getLoaderIcon(details.loader) ?? BoxIcon" class="size-4" aria-hidden="true" />
			<template v-if="arrows && loader && loader.from"
				>{{ loader.from }} → {{ loader.to }}</template
			>
			<template v-else>{{ formatLoader(formatMessage, details.loader) }}</template>
		</span>
		<span v-tooltip="'Content'" class="flex items-center gap-1.5 tabular-nums">
			<BoxIcon class="size-4" aria-hidden="true" />
			{{ details.contentCount }}
		</span>
	</span>
</template>
