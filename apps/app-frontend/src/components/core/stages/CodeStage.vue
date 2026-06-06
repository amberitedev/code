<script setup lang="ts">
import { StyledInput } from '@modrinth/ui'

import { injectCoreOnboardingContext } from '../core-onboarding-context'

const ctx = injectCoreOnboardingContext()
</script>

<template>
	<div class="flex min-h-[26rem] flex-col gap-3">
		<p class="m-0 text-secondary">Enter the pairing or invite code from the Core you want to join.</p>
		<StyledInput v-model="ctx.connectCode.value" placeholder="ABC-123" />
		<p v-if="ctx.error.value" class="m-0 text-red">{{ ctx.error.value }}</p>
	</div>
</template>
