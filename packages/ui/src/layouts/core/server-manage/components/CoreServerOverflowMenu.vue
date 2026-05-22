<template>
	<div class="contents">
		<ButtonStyled circular type="transparent" size="large">
			<TeleportOverflowMenu :options="menuOptions">
				<MoreVerticalIcon aria-hidden="true" />
				<template #all-instances>
					<ServerIcon class="h-5 w-5" />
					<span>All instances</span>
				</template>
				<template #copy-id>
					<ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
					<span>Copy ID</span>
				</template>
			</TeleportOverflowMenu>
		</ButtonStyled>
	</div>
</template>

<script setup lang="ts">
import { ClipboardCopyIcon, MoreVerticalIcon, ServerIcon } from '@modrinth/assets'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { ButtonStyled } from '#ui/components'
import TeleportOverflowMenu from '#ui/components/base/TeleportOverflowMenu.vue'
import { injectModrinthServerContext } from '#ui/providers'

const props = withDefaults(defineProps<{ showCopyIdAction?: boolean }>(), {
	showCopyIdAction: false,
})

const router = useRouter()
const { serverId } = injectModrinthServerContext()

const menuOptions = computed(() => [
	{
		id: 'all-instances',
		label: 'All instances',
		icon: ServerIcon,
		action: () => router.push('/library/servers'),
	},
	{
		id: 'copy-id',
		label: 'Copy ID',
		icon: ClipboardCopyIcon,
		action: () => navigator.clipboard.writeText(serverId),
		shown: props.showCopyIdAction,
	},
])
</script>
