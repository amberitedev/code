<script setup lang="ts">
import { overrides, resetVar, setVar } from '@/composables/useThemeEditorComms'

const gapVars = ['--gap-xs', '--gap-sm', '--gap-md', '--gap-lg', '--gap-xl']
const radiusVars = ['--radius-xs', '--radius-sm', '--radius-md', '--radius-lg', '--radius-xl']
const platformVars = [
	'--color-platform-fabric',
	'--color-platform-quilt',
	'--color-platform-forge',
	'--color-platform-neoforge',
	'--color-platform-liteloader',
	'--color-platform-bukkit',
	'--color-platform-bungeecord',
	'--color-platform-folia',
	'--color-platform-paper',
	'--color-platform-purpur',
	'--color-platform-spigot',
	'--color-platform-velocity',
	'--color-platform-waterfall',
	'--color-platform-sponge',
	'--color-platform-ornithe',
	'--color-platform-bta-babric',
	'--color-platform-nilloader',
]

function getRemVal(name: string): number {
	const raw = (
		overrides[name] || getComputedStyle(document.documentElement).getPropertyValue(name)
	).trim()
	return parseFloat(raw) || 0
}

function setRem(name: string, val: string) {
	setVar(name, `${parseFloat(val).toFixed(3)}rem`)
}

function getColorVal(name: string): string {
	const v = (
		overrides[name] || getComputedStyle(document.documentElement).getPropertyValue(name)
	).trim()
	if (/^#[0-9a-fA-F]{6}$/.test(v)) return v
	if (/^#[0-9a-fA-F]{8}$/.test(v)) return v.slice(0, 7)
	return '#000000'
}

function platformLabel(name: string): string {
	return name.replace('--color-platform-', '')
}
</script>

<template>
	<div class="p-3 flex flex-col gap-5">
		<section>
			<h3
				class="text-xs font-semibold uppercase tracking-wide mb-2"
				style="color: var(--color-secondary)"
			>
				Gap
			</h3>
			<div class="flex flex-col gap-2">
				<div v-for="name in gapVars" :key="name" class="flex items-center gap-2">
					<span class="w-20 text-xs font-mono shrink-0" style="color: var(--color-secondary)">
						{{ name.replace('--', '') }}
					</span>
					<input
						type="range"
						min="0"
						max="3"
						step="0.025"
						:value="getRemVal(name)"
						class="flex-1 cursor-pointer"
						@input="setRem(name, ($event.target as HTMLInputElement).value)"
					/>
					<span
						class="w-14 text-xs font-mono text-right shrink-0"
						style="color: var(--color-secondary)"
					>
						{{ getRemVal(name).toFixed(3) }}rem
					</span>
					<span
						v-if="overrides[name]"
						class="text-xs cursor-pointer shrink-0"
						style="color: var(--color-secondary)"
						title="Reset"
						@click="resetVar(name)"
						>&#x2715;</span
					>
					<span v-else class="w-3 shrink-0" />
				</div>
			</div>
		</section>
		<section>
			<h3
				class="text-xs font-semibold uppercase tracking-wide mb-1"
				style="color: var(--color-secondary)"
			>
				Radius
			</h3>
			<div
				class="h-10 w-full mb-2 border"
				style="
					background: var(--color-button-bg);
					border-color: var(--surface-5);
					border-radius: var(--radius-lg);
					transition: border-radius 0.1s;
				"
			/>
			<div class="flex flex-col gap-2">
				<div v-for="name in radiusVars" :key="name" class="flex items-center gap-2">
					<span class="w-20 text-xs font-mono shrink-0" style="color: var(--color-secondary)">
						{{ name.replace('--', '') }}
					</span>
					<input
						type="range"
						min="0"
						max="2"
						step="0.025"
						:value="getRemVal(name)"
						class="flex-1 cursor-pointer"
						@input="setRem(name, ($event.target as HTMLInputElement).value)"
					/>
					<span
						class="w-14 text-xs font-mono text-right shrink-0"
						style="color: var(--color-secondary)"
					>
						{{ getRemVal(name).toFixed(3) }}rem
					</span>
					<span
						v-if="overrides[name]"
						class="text-xs cursor-pointer shrink-0"
						style="color: var(--color-secondary)"
						title="Reset"
						@click="resetVar(name)"
						>&#x2715;</span
					>
					<span v-else class="w-3 shrink-0" />
				</div>
			</div>
		</section>
		<section>
			<h3
				class="text-xs font-semibold uppercase tracking-wide mb-2"
				style="color: var(--color-secondary)"
			>
				Platform Colors
			</h3>
			<div class="grid grid-cols-2 gap-1">
				<div v-for="name in platformVars" :key="name" class="flex items-center gap-1.5">
					<label class="relative w-5 h-5 rounded overflow-hidden cursor-pointer shrink-0">
						<div class="w-full h-full" :style="{ background: `var(${name})` }" />
						<input
							type="color"
							:value="getColorVal(name)"
							class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
							@change="setVar(name, ($event.target as HTMLInputElement).value)"
						/>
					</label>
					<span class="text-xs font-mono truncate" style="color: var(--color-secondary)">
						{{ platformLabel(name) }}
					</span>
					<span
						v-if="overrides[name]"
						class="text-xs cursor-pointer ml-auto shrink-0"
						style="color: var(--color-secondary)"
						title="Reset"
						@click="resetVar(name)"
						>&#x2715;</span
					>
				</div>
			</div>
		</section>
	</div>
</template>
