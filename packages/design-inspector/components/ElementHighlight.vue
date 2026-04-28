<script setup lang="ts">
interface Props {
	top: number
	left: number
	width: number
	height: number
	name?: string
}

defineProps<Props>()
</script>

<template>
	<div
		style="
			position: fixed;
			border: 2px solid #22c55e;
			border-radius: 2px;
			pointer-events: none;
			z-index: 9998;
			box-sizing: border-box;
			background: rgba(34, 197, 94, 0.08);
			transition: all 60ms ease;
		"
		:style="{ top: `${top}px`, left: `${left}px`, width: `${width}px`, height: `${height}px` }"
	>
		<span
			v-if="name"
			style="
				position: absolute;
				bottom: 100%;
				left: -2px;
				background: #22c55e;
				color: white;
				font-size: 11px;
				font-family: ui-monospace, monospace;
				padding: 1px 5px;
				border-radius: 2px 2px 0 0;
				white-space: nowrap;
				pointer-events: none;
				line-height: 1.6;
			"
		>
			{{ name }}
		</span>
	</div>
</template>
