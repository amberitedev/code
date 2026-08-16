<template>
	<div class="contents">
		<ButtonStyled circular type="transparent" size="large">
			<TeleportOverflowMenu :options="menuOptions">
				<MoreVerticalIcon aria-hidden="true" />
				<template #allServers>
					<ServerIcon class="h-5 w-5" />
					<span>All servers</span>
				</template>
				<template #copy-id>
					<ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
					<span>{{ copyIdLabel }}</span>
				</template>
			</TeleportOverflowMenu>
		</ButtonStyled>
	</div>
</template>

<script setup lang="ts">
import { ClipboardCopyIcon, MoreVerticalIcon, ServerIcon } from '@modrinth/assets'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { ButtonStyled, TeleportOverflowMenu } from '#ui/components'
import { injectModrinthServerContext } from '#ui/providers'

const props = withDefaults(
	defineProps<{
		disabled?: boolean
		showCopyIdAction?: boolean
		copyIdLabel?: string
		showDebugInfo?: boolean
		uptimeSeconds?: number
	}>(),
	{
		disabled: false,
		showCopyIdAction: false,
		copyIdLabel: 'Copy ID',
		showDebugInfo: false,
		uptimeSeconds: 0,
	},
)

const router = useRouter()
const { serverId } = injectModrinthServerContext()

const menuOptions = computed(() => [
	{
		id: 'allServers',
		label: 'All servers',
		icon: ServerIcon,
		action: () => router.push('/hosting/manage'),
	},
	{
		id: 'copy-id',
		label: props.copyIdLabel,
		icon: ClipboardCopyIcon,
		action: () => copyId(),
		shown: props.showCopyIdAction,
	},
])

async function copyId() {
	await navigator.clipboard.writeText(serverId)
}
</script>
