<script setup lang="ts">
import { Toggle } from '@modrinth/ui'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
</script>

<template>
	<div class="flex flex-col gap-4">
		<label class="flex flex-col gap-1">
			<span class="font-semibold">Group name</span>
			<input
				v-model="ctx.groupName.value"
				class="rounded-lg bg-bg-input px-3 py-2"
				placeholder="My Core Friend Group"
			/>
		</label>

		<label class="flex flex-col gap-1">
			<span class="font-semibold">Description</span>
			<textarea
				v-model="ctx.description.value"
				rows="3"
				class="rounded-lg bg-bg-input px-3 py-2"
				placeholder="What this group is for..."
			/>
		</label>

		<label class="flex flex-col gap-1">
			<span class="font-semibold">Banner image URL</span>
			<input
				v-model="ctx.bannerUrl.value"
				class="rounded-lg bg-bg-input px-3 py-2"
				placeholder="https://..."
			/>
			<div
				v-if="ctx.bannerUrl.value"
				class="h-24 rounded-xl bg-cover bg-center border border-surface-5 mt-1"
				:style="{ backgroundImage: `url(${ctx.bannerUrl.value})` }"
			/>
		</label>

		<label class="flex flex-col gap-1">
			<span class="font-semibold">Subdomain</span>
			<input
				v-model="ctx.subdomain.value"
				class="rounded-lg bg-bg-input px-3 py-2"
				placeholder="my-core"
			/>
			<span class="text-secondary text-xs">Custom subdomain for your Core (optional).</span>
		</label>

		<template v-if="ctx.mode === 'setup'">
			<div class="border-t border-surface-5 pt-4 flex flex-col gap-3">
				<span class="font-semibold">Startup behavior</span>
				<div class="flex items-center justify-between">
					<div class="flex flex-col">
						<span class="font-medium">Start on system startup</span>
						<span class="text-secondary text-xs">Launch Core automatically when your PC boots</span>
					</div>
					<Toggle v-model="ctx.runOnStartup.value" small />
				</div>
				<div class="flex items-center justify-between">
					<div class="flex flex-col">
						<span class="font-medium">Run in background</span>
						<span class="text-secondary text-xs"
							>Keep Core running even when the app is closed</span
						>
					</div>
					<Toggle v-model="ctx.runInBackground.value" small />
				</div>
			</div>
		</template>
	</div>
</template>
