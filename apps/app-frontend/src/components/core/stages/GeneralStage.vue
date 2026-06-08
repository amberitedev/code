<script setup lang="ts">
import { IconSelect, StyledInput } from '@modrinth/ui'

import { injectCoreOnboardingContext } from '../core-onboarding-context'

const ctx = injectCoreOnboardingContext()
</script>

<template>
	<div class="grid min-h-[26rem] content-start gap-5 md:grid-cols-[8rem_minmax(0,1fr)]">
		<div class="flex flex-col gap-2 md:row-span-2">
			<span class="font-semibold text-contrast">Icon</span>
			<IconSelect
				v-model="ctx.coreIcon.value"
				:options="['select', 'remove']"
				:show-edit-icon="false"
				@select="ctx.selectIcon"
				@remove="ctx.coreIcon.value = undefined"
			/>
		</div>
		<div class="flex flex-col gap-3">
			<StyledInput v-model="ctx.coreName.value" placeholder="Friend group name" />
			<StyledInput v-model="ctx.coreDescription.value" placeholder="Description" multiline :rows="8" />
		</div>
	</div>
</template>
