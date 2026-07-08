<script setup lang="ts">
import type { InstallableSyncedProfile } from '@amberite/amberite-api'
import { DownloadIcon, GameIcon, SpinnerIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled, formatLoader, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

const props = defineProps<{
	instance: InstallableSyncedProfile
	installing?: boolean
}>()

const emit = defineEmits<{
	install: [instance: InstallableSyncedProfile]
}>()

const { formatMessage } = useVIntl()

const canInstall = computed(() => props.instance.availability === 'installable' && !props.installing)
const installTooltip = computed(() => {
	if (props.installing) return 'Installing...'
	if (!canInstall.value)
		return props.instance.unavailableReason ?? 'This shared instance is not installable.'
	return 'Install'
})
const metadata = computed(() => {
	const loader = props.instance.loader
		? formatLoader(formatMessage, props.instance.loader)
		: 'Unknown loader'
	const gameVersion = props.instance.gameVersion ?? 'Unknown version'
	return `Synced - ${loader} ${gameVersion}`
})

function install() {
	if (!canInstall.value) return
	emit('install', props.instance)
}
</script>

<template>
	<div
		class="button-base bg-bg-raised p-4 rounded-xl flex gap-3 group cursor-default faded-instance-card"
		:class="{ 'faded-instance-card-installable': canInstall }"
	>
		<div class="relative flex items-center justify-center">
			<Avatar
				size="48px"
				:tint-by="instance.coreInstanceId"
				alt="Shared synced instance"
				:class="`transition-all ${
					installing
						? 'brightness-[0.25] scale-[0.85]'
						: 'grayscale opacity-70 group-hover:grayscale-0 group-hover:opacity-100 group-focus-within:grayscale-0 group-focus-within:opacity-100'
				}`"
			/>
			<div class="absolute inset-0 flex items-center justify-center">
				<SpinnerIcon
					v-if="installing"
					v-tooltip="'Installing...'"
					class="animate-spin w-8 h-8"
					tabindex="-1"
				/>
				<ButtonStyled v-else size="large" :color="canInstall ? 'brand' : 'standard'" circular>
					<button
						v-tooltip="installTooltip"
						:disabled="!canInstall"
						class="transition-all scale-75 group-hover:scale-100 group-focus-within:scale-100 origin-bottom opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 card-shadow"
						:aria-label="installTooltip"
						@click.stop="install"
					>
						<DownloadIcon />
					</button>
				</ButtonStyled>
			</div>
		</div>
		<div class="flex min-w-0 flex-col gap-1">
			<p class="m-0 text-md font-bold text-contrast leading-tight line-clamp-1">
				{{ instance.name }}
			</p>
			<div class="flex min-w-0 items-center col-span-3 gap-1 text-secondary font-semibold">
				<GameIcon class="size-4 shrink-0" />
				<span class="truncate text-sm">
					{{ metadata }}
				</span>
			</div>
			<p
				v-if="instance.availability !== 'installable' && instance.unavailableReason"
				class="m-0 text-xs font-semibold text-secondary line-clamp-1"
			>
				{{ instance.unavailableReason }}
			</p>
		</div>
	</div>
</template>

<style lang="scss" scoped>
.faded-instance-card {
	opacity: 0.58;
	transition:
		opacity 160ms cubic-bezier(0.2, 0, 0, 1),
		filter 160ms cubic-bezier(0.2, 0, 0, 1);
}

.faded-instance-card:hover,
.faded-instance-card:focus-within {
	opacity: 1;
}
</style>
