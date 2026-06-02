<script setup lang="ts">
import { CheckIcon, LoaderIcon, ServerStackIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
const complete = ref(false)
const title = computed(() => (ctx.mode === 'setup' ? 'Preparing local Core' : 'Verifying connection'))

onMounted(async () => {
	complete.value = false
	await ctx.runFinish()
	complete.value = true
})
</script>

<template>
	<div class="flex flex-col gap-5">
		<div class="rounded-2xl bg-surface-3 p-6 text-center">
			<div class="mx-auto mb-4 grid h-20 w-20 place-items-center rounded-3xl bg-brand-highlight">
				<CheckIcon v-if="complete" class="h-10 w-10 text-brand" />
				<LoaderIcon v-else class="h-10 w-10 animate-spin text-brand" />
			</div>
			<h2 class="m-0 text-2xl font-bold text-contrast">{{ complete ? 'Core ready' : title }}</h2>
			<p class="mx-auto mb-0 mt-2 max-w-md text-sm text-secondary">
				{{
					complete
						? 'Your Core workspace is linked. You can manage servers, members, and settings from the Core page.'
						: ctx.progressLabel.value
				}}
			</p>
			<div class="mt-5 overflow-hidden rounded-full bg-surface-4">
				<div class="h-2 rounded-full bg-brand transition-all" :style="{ width: `${ctx.progress.value}%` }" />
			</div>
			<div class="mt-2 text-sm font-bold text-secondary">{{ ctx.progress.value }}%</div>
		</div>

		<div class="grid gap-3 md:grid-cols-3">
			<div class="rounded-2xl bg-surface-3 p-4">
				<ServerStackIcon class="mb-2 text-brand" />
				<div class="font-bold text-contrast">Core identity</div>
				<p class="m-0 text-sm text-secondary">{{ ctx.coreId.value ?? 'Pending local link' }}</p>
			</div>
			<div class="rounded-2xl bg-surface-3 p-4">
				<div class="font-bold text-contrast">Mode</div>
				<p class="m-0 text-sm text-secondary">{{ ctx.mode === 'setup' ? 'Local install' : 'External Core' }}</p>
			</div>
			<div class="rounded-2xl bg-surface-3 p-4">
				<ButtonStyled color="brand">
					<button :disabled="!complete" @click="ctx.finish">Open Core dashboard</button>
				</ButtonStyled>
			</div>
		</div>
	</div>
</template>
