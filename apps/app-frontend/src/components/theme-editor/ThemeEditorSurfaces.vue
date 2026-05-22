<script setup lang="ts">
import { overrides, resetVar, setVar } from '@/composables/useThemeEditorComms'

const groups = [
	{
		label: 'Surfaces',
		vars: [
			'--surface-1',
			'--surface-1-5',
			'--surface-2',
			'--surface-2-5',
			'--surface-3',
			'--surface-4',
			'--surface-5',
		],
	},
	{
		label: 'Text Colors',
		vars: ['--color-text-primary', '--color-text-default', '--color-text-tertiary'],
	},
]

function getValue(name: string): string {
	return (
		overrides[name] || getComputedStyle(document.documentElement).getPropertyValue(name)
	).trim()
}

function toHex6(value: string): string {
	const v = value.trim()
	if (/^#[0-9a-fA-F]{6}$/.test(v)) return v
	if (/^#[0-9a-fA-F]{8}$/.test(v)) return v.slice(0, 7)
	if (/^#[0-9a-fA-F]{3}$/.test(v)) {
		const [r, g, b] = v.slice(1).split('')
		return `#${r}${r}${g}${g}${b}${b}`
	}
	return '#000000'
}
</script>

<template>
	<div class="p-3 flex flex-col gap-5">
		<section v-for="group in groups" :key="group.label">
			<h3
				class="text-xs font-semibold uppercase tracking-wide mb-2"
				style="color: var(--color-secondary)"
			>
				{{ group.label }}
			</h3>
			<div class="flex flex-col gap-1">
				<div v-for="name in group.vars" :key="name" class="flex items-center gap-2">
					<div
						class="w-4 h-4 rounded shrink-0 border"
						:style="{ background: `var(${name})`, borderColor: 'var(--surface-5)' }"
					/>
					<span class="flex-1 text-xs font-mono truncate" style="color: var(--color-secondary)">
						{{ name }}
					</span>
					<label class="relative w-6 h-5 rounded overflow-hidden cursor-pointer shrink-0">
						<div class="w-full h-full" :style="{ background: toHex6(getValue(name)) }" />
						<input
							type="color"
							:value="toHex6(getValue(name))"
							class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
							@change="setVar(name, ($event.target as HTMLInputElement).value)"
						/>
					</label>
					<span
						v-if="overrides[name]"
						class="text-xs cursor-pointer shrink-0"
						style="color: var(--color-secondary)"
						title="Reset"
						@click="resetVar(name)"
					>
						&#x2715;
					</span>
					<span v-else class="w-3 shrink-0" />
				</div>
			</div>
		</section>
	</div>
</template>
