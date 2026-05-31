<template>
	<div class="flex w-full flex-col gap-4">
		<ContentPageHeader :class="headerClass">
			<template #icon>
				<ServerIcon :image="serverImage ?? undefined" class="size-16 !rounded-xl" />
			</template>
			<template #title>
				{{ server?.name || 'Server' }}
			</template>
			<template #stats>
				<div
					v-if="server?.flows?.intro"
					class="flex items-center gap-2 font-semibold text-secondary"
				>
					<SettingsIcon />
					Configuring server...
				</div>
				<div v-else class="flex flex-wrap items-center gap-2">
					<div v-if="server?.loader" class="flex items-center gap-2 font-medium capitalize">
						<LoaderIcon :loader="server.loader" class="flex shrink-0 [&&]:size-5" />
						{{ server.loader }} {{ server.mc_version }}
					</div>

					<div v-if="server?.loader && address" class="h-1.5 w-1.5 rounded-full bg-surface-5" />

					<div
						v-if="address"
						v-tooltip="'Copy server address'"
						class="flex cursor-pointer items-center gap-2 text-nowrap font-medium hover:underline"
						@click="copyServerAddress"
					>
						<LinkIcon class="flex size-5 shrink-0" />
						{{ address }}
					</div>

					<div v-if="showUptime" class="h-1.5 w-1.5 rounded-full bg-surface-5" />

					<div v-if="showUptime" class="flex items-center gap-2 font-medium">
						<TimerIcon class="flex size-5 shrink-0" />
						{{ formattedUptime }}
					</div>
				</div>
			</template>
			<template #actions>
				<slot name="actions" />
			</template>
		</ContentPageHeader>
	</div>
</template>

<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { LinkIcon, LoaderIcon, SettingsIcon, TimerIcon } from '@modrinth/assets'
import { ContentPageHeader, injectNotificationManager, ServerIcon } from '@modrinth/ui'
import { computed } from 'vue'

const props = withDefaults(
	defineProps<{
		server: Archon.Servers.v0.Server | null | undefined
		serverImage?: string | null
		uptimeSeconds?: number
		showUptime?: boolean
		headerClass?: string
	}>(),
	{
		serverImage: null,
		uptimeSeconds: 0,
		showUptime: true,
		headerClass: '',
	},
)

const { addNotification } = injectNotificationManager()

const address = computed(() => props.server?.net?.domain ?? '')

const showUptime = computed(() => props.showUptime && (props.uptimeSeconds ?? 0) > 0)

const formattedUptime = computed(() => {
	const uptime = props.uptimeSeconds ?? 0
	const days = Math.floor(uptime / (24 * 3600))
	const hours = Math.floor((uptime % (24 * 3600)) / 3600)
	const minutes = Math.floor((uptime % 3600) / 60)
	const seconds = uptime % 60

	let formatted = ''
	if (days > 0) formatted += `${days}d `
	if (hours > 0 || days > 0) formatted += `${hours}h `
	formatted += `${minutes}m ${seconds}s`
	return formatted.trim()
})

function copyServerAddress() {
	if (!address.value) return
	navigator.clipboard.writeText(address.value)
	addNotification({
		title: 'Server address copied',
		text: "Your server's address has been copied to your clipboard.",
		type: 'success',
	})
}
</script>
