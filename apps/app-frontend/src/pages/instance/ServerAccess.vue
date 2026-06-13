<template>
	<div class="flex w-full flex-col gap-4">
		<div class="flex flex-col gap-1">
			<h2 class="m-0 text-2xl font-semibold text-contrast">Access</h2>
			<p class="m-0 text-secondary">Manage access and review activity for this server.</p>
		</div>
		<CoreAccessPage :instance-id="ctx.instanceId.value" />
	</div>
</template>

<script setup lang="ts">
import { inject } from 'vue'

import CoreAccessPage from '@/components/core/CoreAccessPage.vue'

import { coreServerContextKey } from './server/core-server-instance'

const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')
</script>
