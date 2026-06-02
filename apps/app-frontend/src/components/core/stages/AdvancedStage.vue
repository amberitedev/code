<script setup lang="ts">
import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
</script>

<template>
	<div class="flex flex-col gap-4">
		<label class="flex flex-col gap-1">
			<span class="font-semibold">Run mode</span>
			<select v-model="ctx.runMode.value" class="rounded-lg bg-bg-input px-3 py-2">
				<option value="manual">Manual — only when you start it</option>
				<option value="app_open">When the app opens</option>
				<option value="startup">On system startup</option>
			</select>
		</label>

		<div class="border-t border-surface-5 pt-4 flex flex-col gap-3">
			<span class="font-semibold">Resource limits</span>
			<label class="flex flex-col gap-1">
				<span>Max players</span>
				<input type="number" class="rounded-lg bg-bg-input px-3 py-2" placeholder="20" />
			</label>
		</div>
	</div>
</template>
