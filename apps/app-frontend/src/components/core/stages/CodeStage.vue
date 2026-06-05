<script setup lang="ts">
import { KeyIcon, LinkIcon, ServerStackIcon } from '@modrinth/assets'
import { StyledInput } from '@modrinth/ui'
import { computed } from 'vue'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
const valid = computed(() => /^\d{6}$/.test(ctx.code.value.trim()))
</script>

<template>
	<div class="flex flex-col gap-5">
		<div class="rounded-2xl bg-surface-3 p-5">
			<div class="mb-4 flex items-center gap-3">
				<div class="grid h-11 w-11 place-items-center rounded-2xl bg-brand-highlight text-brand">
					<LinkIcon />
				</div>
				<div>
					<h2 class="m-0 text-xl font-bold text-contrast">Connect to existing</h2>
					<p class="m-0 text-sm text-secondary">
						Start Core on the host machine and copy the code printed in its terminal.
					</p>
				</div>
			</div>
			<div class="grid gap-3 md:grid-cols-[1fr_auto] md:items-end">
				<label class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Six-digit pairing code</span>
					<StyledInput
						v-model="ctx.code.value"
						:icon="KeyIcon"
						inputmode="numeric"
						maxlength="6"
						placeholder="123456"
						input-class="font-mono !text-xl tracking-[0.35em]"
						:error="ctx.code.value.length > 0 && !valid"
						:disabled="ctx.step1Loading.value"
					/>
				</label>
				<div class="rounded-xl bg-surface-4 px-4 py-3 text-sm text-secondary">
					<div class="mb-1 flex items-center gap-2 font-bold text-contrast">
						<ServerStackIcon class="h-4 w-4" />
						Core terminal
					</div>
					<code>Amberite Core - Pairing Code</code>
				</div>
			</div>
			<p v-if="ctx.step1Error.value" class="m-0 mt-3 text-sm font-semibold text-red">
				{{ ctx.step1Error.value }}
			</p>
		</div>
	</div>
</template>
