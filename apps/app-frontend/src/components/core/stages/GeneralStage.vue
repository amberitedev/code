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
				class="hide-icon-select-edit-icon"
				:options="['select', 'remove']"
				@select="ctx.selectIcon"
				@remove="ctx.coreIcon.value = undefined"
			/>
		</div>
		<div class="flex flex-col gap-3">
			<StyledInput v-model="ctx.coreName.value" placeholder="Friend group name" />
			<StyledInput v-model="ctx.coreDescription.value" placeholder="Description" multiline :rows="8" />
			<StyledInput
				v-if="ctx.flow.value === 'create'"
				v-model="ctx.coreUrl.value"
				placeholder="Core API address, e.g. https://core.example.com"
			/>
		</div>
	</div>
</template>

<style scoped>
.hide-icon-select-edit-icon :deep(.hovering-icon-shadow) {
	display: none;
}
</style>
