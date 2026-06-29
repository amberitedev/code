<template>
	<div class="flex min-h-screen items-center justify-center bg-white p-8 dark:bg-black">
		<div class="flex w-full max-w-md flex-col items-center gap-16">
			<button
				type="button"
				class="flex h-10 w-[120px] shrink-0 items-center justify-center rounded-full bg-black px-4 text-base font-medium text-zinc-50 shadow-xs transition-colors hover:bg-zinc-800 dark:bg-zinc-50 dark:text-black dark:hover:bg-zinc-200"
				@click="buttonText = buttonText === 'Continue' ? 'Confirm' : 'Continue'"
			>
				<TextMorph :children="buttonText" />
			</button>

			<div class="flex w-full flex-col items-center justify-end space-y-12">
				<TextMorph
					class-name="text-5xl font-medium text-black dark:text-white"
					:children="inputText"
				/>
				<input
					v-model="inputText"
					type="text"
					class="h-9 w-full rounded-lg border border-zinc-950/10 bg-transparent p-2 text-zinc-900 placeholder-zinc-500 focus:outline-hidden dark:border-zinc-50/10 dark:text-white dark:placeholder-zinc-400"
					placeholder="Type your text here"
				/>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { TextMorph } from '@modrinth/ui'
import { ref } from 'vue'

definePageMeta({
	layout: false,
})

const buttonText = ref('Continue')
const inputText = ref('Craft')
</script>
