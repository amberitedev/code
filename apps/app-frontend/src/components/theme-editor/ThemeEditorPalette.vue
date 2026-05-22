<script setup lang="ts">
import { overrides, setVar } from '@/composables/useThemeEditorComms'

const scales = ['red', 'orange', 'green', 'blue', 'purple', 'gray'] as const
const shades = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950] as const

function getVal(scale: string, shade: number): string {
	const name = `--color-${scale}-${shade}`
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

function onColorChange(scale: string, shade: number, value: string) {
	setVar(`--color-${scale}-${shade}`, value)
}
</script>

<template>
	<div class="p-3 flex flex-col gap-2">
		<div class="flex items-center gap-1 mb-1">
			<span class="w-16 shrink-0" />
			<div class="flex gap-1">
				<span
					v-for="shade in shades"
					:key="shade"
					class="w-6 text-center"
					style="font-size: 9px; color: var(--color-secondary)"
				>
					{{ shade >= 100 ? shade / 100 : shade }}
				</span>
			</div>
		</div>
		<div v-for="scale in scales" :key="scale" class="flex items-center gap-1">
			<span
				class="w-16 text-xs font-mono shrink-0 capitalize"
				style="color: var(--color-secondary)"
			>
				{{ scale }}
			</span>
			<div class="flex gap-1">
				<label
					v-for="shade in shades"
					:key="shade"
					class="relative w-6 h-6 rounded overflow-hidden cursor-pointer shrink-0"
					:class="{ 'ring-1': overrides[`--color-${scale}-${shade}`] }"
					:style="{ outlineColor: 'var(--color-brand)' }"
					:title="`--color-${scale}-${shade}: ${getVal(scale, shade)}`"
				>
					<div class="w-full h-full" :style="{ background: `var(--color-${scale}-${shade})` }" />
					<input
						type="color"
						:value="toHex6(getVal(scale, shade))"
						class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
						@change="onColorChange(scale, shade, ($event.target as HTMLInputElement).value)"
					/>
				</label>
			</div>
		</div>
	</div>
</template>
