<script setup lang="ts">
interface Props {
	top: number
	left: number
	width: number
	height: number
	name?: string
	animated?: boolean
	color?: string
}

const props = withDefaults(defineProps<Props>(), { animated: true, color: '#22c55e' })

/** Convert a #rrggbb hex colour to rgba(r,g,b,alpha). */
function hex2rgba(hex: string, alpha: number): string {
	const r = parseInt(hex.slice(1, 3), 16)
	const g = parseInt(hex.slice(3, 5), 16)
	const b = parseInt(hex.slice(5, 7), 16)
	return `rgba(${r},${g},${b},${alpha})`
}
</script>

<template>
	<div
		:style="{
			position: 'fixed',
			border: `2px solid ${props.color}`,
			borderRadius: '2px',
			pointerEvents: 'none',
			zIndex: '9998',
			boxSizing: 'border-box',
			background: hex2rgba(props.color, 0.08),
			transition: props.animated ? 'all 60ms ease' : 'none',
			top: `${top}px`,
			left: `${left}px`,
			width: `${width}px`,
			height: `${height}px`,
		}"
	>
		<span
			v-if="name"
			:style="{
				position: 'absolute', bottom: '100%', left: '-2px',
				display: 'block', background: props.color, color: 'white',
				fontSize: '11px', fontFamily: 'ui-monospace, monospace',
				padding: '1px 5px', borderRadius: '2px 2px 0 0',
				whiteSpace: 'nowrap', lineHeight: '1.6',
			}"
		>{{ name }}</span>
	</div>
</template>
