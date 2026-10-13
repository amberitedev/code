<script setup lang="ts">
import { LogInIcon, ThinkingRinthbot } from '@modrinth/assets'
import { Button, commonMessages, EmptyState, injectAuth, useVIntl } from '@modrinth/ui'

// The signed-out screen for anything that needs the account's Cores. Same look as the account tabs in
// settings. Show it when `injectAuth().user` is empty.
defineProps<{
	description: string
}>()

const auth = injectAuth()
const { formatMessage } = useVIntl()
</script>

<template>
	<EmptyState
		type="empty"
		class="[&>div:last-child]:!mt-6"
		heading="Modrinth account required"
		:description="description"
	>
		<template #illustration>
			<div class="relative mb-4 h-[200px]">
				<img :src="ThinkingRinthbot" alt="" class="h-full w-auto object-contain" />
				<div
					class="pointer-events-none absolute inset-x-0 bottom-0 h-14 bg-gradient-to-t from-bg-raised to-transparent"
				/>
			</div>
		</template>
		<template #actions>
			<Button
				type="colored"
				color="brand"
				size="xl"
				native-type="button"
				@click="auth.requestSignIn('')"
			>
				<LogInIcon aria-hidden="true" />
				{{ formatMessage(commonMessages.signInButton) }}
			</Button>
		</template>
	</EmptyState>
</template>
