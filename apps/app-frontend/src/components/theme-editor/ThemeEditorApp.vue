<script setup lang="ts">
import { ref } from 'vue'

import { overrides, resetAll } from '@/composables/useThemeEditorComms'

import ThemeEditorPalette from './ThemeEditorPalette.vue'
import ThemeEditorSurfaces from './ThemeEditorSurfaces.vue'
import ThemeEditorTokens from './ThemeEditorTokens.vue'

type Tab = 'surfaces' | 'palette' | 'tokens'

const activeTab = ref<Tab>('surfaces')
const isDark = ref(true)

const tabs: Tab[] = ['surfaces', 'palette', 'tokens']

function toggleMode() {
	const html = document.documentElement
	isDark.value = !isDark.value
	if (isDark.value) {
		html.classList.remove('light-mode')
		html.classList.add('dark-mode')
	} else {
		html.classList.remove('dark-mode')
		html.classList.add('light-mode')
	}
}

function copyCSS() {
	const entries = Object.entries(overrides)
	if (!entries.length) return
	const css = `:root {\n${entries.map(([k, v]) => `\t${k}: ${v};`).join('\n')}\n}`
	navigator.clipboard.writeText(css).catch(console.warn)
}
</script>

<template>
	<div
		class="flex flex-col h-screen overflow-hidden"
		style="background: var(--color-bg); color: var(--color-base)"
	>
		<header
			class="flex items-center justify-between px-3 py-2 shrink-0 border-b"
			style="background: var(--color-raised-bg); border-color: var(--surface-5)"
		>
			<span class="font-semibold text-sm" style="color: var(--color-contrast)">Theme Editor</span>
			<div class="flex gap-1.5">
				<button
					class="px-2 py-1 rounded text-xs cursor-pointer border-0"
					style="background: var(--color-button-bg); color: var(--color-base)"
					@click="toggleMode"
				>
					{{ isDark ? 'Light mode' : 'Dark mode' }}
				</button>
				<button
					class="px-2 py-1 rounded text-xs cursor-pointer border-0"
					style="background: var(--color-button-bg); color: var(--color-base)"
					@click="copyCSS"
				>
					Copy CSS
				</button>
				<button
					class="px-2 py-1 rounded text-xs cursor-pointer border-0"
					style="background: var(--color-red-bg); color: var(--color-red)"
					@click="resetAll()"
				>
					Reset All
				</button>
			</div>
		</header>
		<nav class="flex shrink-0 border-b" style="border-color: var(--surface-5)">
			<button
				v-for="tab in tabs"
				:key="tab"
				class="px-4 py-2 capitalize text-xs font-medium cursor-pointer border-0 border-b-2 transition-colors"
				:style="
					activeTab === tab
						? 'border-color: var(--color-brand); color: var(--color-contrast); background: transparent'
						: 'border-color: transparent; color: var(--color-secondary); background: transparent'
				"
				@click="activeTab = tab"
			>
				{{ tab }}
			</button>
		</nav>
		<main class="flex-1 overflow-y-auto">
			<ThemeEditorSurfaces v-if="activeTab === 'surfaces'" />
			<ThemeEditorPalette v-if="activeTab === 'palette'" />
			<ThemeEditorTokens v-if="activeTab === 'tokens'" />
		</main>
	</div>
</template>
