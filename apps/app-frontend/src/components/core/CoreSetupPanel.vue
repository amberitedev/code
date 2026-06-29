<script setup lang="ts">
import { LogInIcon, ServerStackIcon } from '@modrinth/assets'
import { BigOptionButton } from '@modrinth/ui'

defineEmits<{
	create: []
	connect: []
}>()
</script>

<template>
	<div class="flex min-h-full flex-1 items-center justify-center p-6">
		<div class="flex w-full max-w-[760px] flex-col gap-6">
			<div class="flex max-w-[500px] flex-col gap-2">
				<h1 class="m-0 text-3xl font-semibold text-contrast">Set up Copal</h1>
				<p class="m-0 text-base text-secondary">
					Set up a local Core or connect to an existing Core.
				</p>
			</div>
			<div class="grid gap-4 md:grid-cols-2">
				<BigOptionButton
					:icon="ServerStackIcon"
					title="Create a new Core"
					description="Set up this Core and manage its members."
					@click="$emit('create')"
				/>
				<BigOptionButton
					:icon="LogInIcon"
					title="Connect to an existing Core"
					description="Use a pairing or invite code from another Core."
					@click="$emit('connect')"
				/>
			</div>
		</div>
	</div>
</template>
