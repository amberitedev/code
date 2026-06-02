<script setup lang="ts">
import { CheckIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
</script>

<template>
	<div class="flex flex-col items-center gap-4 text-center py-6">
		<div class="w-16 h-16 rounded-full bg-green/20 flex items-center justify-center">
			<CheckIcon class="w-8 h-8 text-green" />
		</div>
		<div class="flex flex-col gap-1">
			<span class="text-xl font-bold text-contrast">
				{{ ctx.mode === 'setup' ? 'Core ready!' : "You're in!" }}
			</span>
			<span class="text-secondary">
				{{
					ctx.mode === 'setup'
						? 'Your Core is set up and your friend group is live.'
						: "You've joined the friend group. Start syncing servers!"
				}}
			</span>
		</div>
		<ButtonStyled color="brand">
			<button @click="ctx.finish">Finish</button>
		</ButtonStyled>
	</div>
</template>
