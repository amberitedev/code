<script setup lang="ts">
import { CheckIcon, CopyIcon, RefreshCwIcon, UndoIcon } from '@modrinth/assets'
import { Button } from '@modrinth/ui'
import { ref } from 'vue'

const props = defineProps<{
	message: string
	/** Everything needed to report the crash: version, route, source, and stack. */
	report: string
	retry: () => void
}>()

const copied = ref(false)

async function copy() {
	await navigator.clipboard.writeText(props.report)
	copied.value = true
	setTimeout(() => (copied.value = false), 3000)
}

function reload() {
	window.location.reload()
}
</script>

<template>
	<div
		data-crash-screen
		class="fixed inset-0 z-[9999] flex flex-col justify-center gap-4 overflow-auto bg-black p-10 text-white"
	>
		<h1 class="m-0 text-2xl font-semibold">Something broke</h1>
		<p class="m-0 max-w-2xl break-words">{{ message }}</p>
		<div class="flex flex-wrap gap-2">
			<Button type="colored" color="brand" @click="retry"><UndoIcon /> Try again</Button>
			<Button @click="reload"><RefreshCwIcon /> Reload</Button>
			<Button @click="copy">
				<CheckIcon v-if="copied" />
				<CopyIcon v-else />
				{{ copied ? 'Copied' : 'Copy error' }}
			</Button>
		</div>
		<details class="max-w-4xl">
			<summary class="cursor-pointer select-none">Details</summary>
			<pre class="mt-2 max-h-80 overflow-auto whitespace-pre-wrap text-sm">{{ report }}</pre>
		</details>
	</div>
</template>
