<script setup lang="ts">
import { ServerStackIcon } from '@modrinth/assets'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
</script>

<template>
	<div class="flex flex-col gap-4">
		<div v-if="ctx.mode === 'setup' && ctx.alreadyPaired.value" class="flex flex-col gap-3">
			<div class="flex items-center gap-2">
				<ServerStackIcon class="w-5 h-5 text-brand" />
				<span class="font-bold text-green">This Core is paired</span>
			</div>
			<p class="text-secondary text-sm m-0">
				Your Core is already linked. Continue to set up your friend group.
			</p>
		</div>
		<div v-else class="flex flex-col gap-3">
			<label class="flex flex-col gap-1">
				<span class="font-semibold">
					{{ ctx.mode === 'setup' ? 'Pairing code' : 'Invite code' }}
				</span>
				<input
					v-model="ctx.code.value"
					class="rounded-lg bg-bg-input px-3 py-2 font-mono"
					:placeholder="ctx.mode === 'setup' ? '123456' : 'AMB-ABC123'"
					:maxlength="ctx.mode === 'setup' ? 6 : 20"
					:disabled="ctx.step1Loading.value"
				/>
				<span class="text-secondary text-xs">
					{{
						ctx.mode === 'setup'
							? 'Shown in the Core server console on first start. 6 digits.'
							: 'Ask the group owner for an invite code.'
					}}
				</span>
			</label>
			<p v-if="ctx.step1Error.value" class="text-red text-sm m-0">{{ ctx.step1Error.value }}</p>
		</div>
	</div>
</template>
