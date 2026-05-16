<template>
	<div class="flex flex-col gap-4">
		<div class="flex items-center justify-between gap-3">
			<h2 class="text-2xl font-semibold text-contrast">Files</h2>
			<button
				class="rounded-full bg-surface-4 px-4 py-2 font-semibold text-contrast"
				@click="loadDirectory"
			>
				Refresh
			</button>
		</div>
		<div v-if="error" class="rounded-xl bg-red-highlight p-4 text-red">{{ error }}</div>
		<div class="overflow-hidden rounded-2xl bg-surface-2">
			<button
				v-if="path !== '/'"
				class="w-full border-b border-surface-5 px-4 py-3 text-left text-primary hover:bg-surface-3"
				@click="openParent"
			>
				../
			</button>
			<button
				v-for="entry in entries"
				:key="entry.path"
				class="flex w-full items-center justify-between border-b border-surface-5 px-4 py-3 text-left text-primary last:border-b-0 hover:bg-surface-3"
				@click="entry.type === 'directory' ? openDirectory(entry.path) : undefined"
			>
				<span>{{ entry.type === 'directory' ? '[dir]' : '[file]' }} {{ entry.name }}</span>
				<span class="text-sm text-secondary">{{ entry.type }}</span>
			</button>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreFsEntry } from '@amberite/api-lib'
import { onMounted, ref, watch } from 'vue'

import { injectCoreClient, injectModrinthServerContext } from '#ui/providers'

const coreClient = injectCoreClient()
const { serverId } = injectModrinthServerContext()
const path = ref('/')
const entries = ref<CoreFsEntry[]>([])
const error = ref<string | null>(null)

async function loadDirectory() {
	error.value = null
	try {
		const listing = await coreClient.listDirectory(serverId, path.value)
		entries.value = listing.items
	} catch (loadError) {
		console.error('[core/server-manage] Failed to load files:', loadError)
		error.value = 'Could not load this directory.'
	}
}

function openDirectory(nextPath: string) {
	path.value = nextPath
}

function openParent() {
	const parts = path.value.split('/').filter(Boolean)
	parts.pop()
	path.value = parts.length ? `/${parts.join('/')}` : '/'
}

watch(path, () => void loadDirectory())
onMounted(() => void loadDirectory())
</script>
