<script setup lang="ts">
import { ImageIcon, ServerStackIcon, UploadIcon } from '@modrinth/assets'
import { Avatar, StyledInput } from '@modrinth/ui'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
</script>

<template>
	<div class="flex flex-col gap-5">
		<div class="overflow-hidden rounded-2xl bg-surface-3">
			<div
				class="flex min-h-32 items-end bg-cover bg-center p-5"
				:style="ctx.bannerUrl.value ? { backgroundImage: `url(${ctx.bannerUrl.value})` } : {}"
			>
				<div class="flex items-end gap-4">
					<Avatar :src="ctx.iconUrl.value || undefined" size="82px" tint-by="amberite-core" />
					<div class="pb-1">
						<h2 class="m-0 text-2xl font-bold text-contrast">{{ ctx.groupName.value || 'New Core' }}</h2>
					</div>
				</div>
			</div>
			<div class="grid gap-4 p-5 md:grid-cols-2">
				<label class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Core name</span>
					<StyledInput v-model="ctx.groupName.value" :icon="ServerStackIcon" maxlength="48" />
				</label>
				<label class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Subdomain</span>
					<StyledInput v-model="ctx.subdomain.value" placeholder="family-core" maxlength="32" />
				</label>
				<label class="flex flex-col gap-2 md:col-span-2">
					<span class="text-sm font-bold text-contrast">Description</span>
					<StyledInput
						v-model="ctx.description.value"
						multiline
						placeholder="Servers, worlds, and sync access for this group."
					/>
				</label>
				<label class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Icon image URL</span>
					<StyledInput v-model="ctx.iconUrl.value" :icon="UploadIcon" placeholder="https://..." />
				</label>
				<label class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Banner image URL</span>
					<StyledInput v-model="ctx.bannerUrl.value" :icon="ImageIcon" placeholder="https://..." />
				</label>
			</div>
		</div>
	</div>
</template>
