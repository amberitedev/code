<script setup lang="ts">
/**
 * InspectorClone — persistent coloured overlay for Win+Alt sticky picks.
 * No close button (dismissed via Escape / token deletion), no colour picker.
 *
 * Overlay taxonomy:
 *   HoverHighlight    → ElementHighlight in InspectorOverlay.vue   (transient, animated, Alt held)
 *   PickModeHighlight → ElementHighlight in InspectorOverlay.vue   (transient, pickForBubble mode)
 *   CommentHighlight  → ElementHighlight in DesignInspectorRoot    (plain-Alt click, always green)
 *   InspectorClone    → this component, DesignInspectorRoot sticky  (Win+Alt pick, custom colour)
 */
interface Props {
	top: number; left: number; width: number; height: number
	/** Component/element display name shown in the label chip. Empty → no chip rendered. */
	name?: string
	/** Highlight border/fill colour. Defaults to green (same as plain-Alt). */
	color?: string
}

const props = withDefaults(defineProps<Props>(), { color: '#22c55e' })

function hex2rgba(hex: string, alpha: number): string {
	const r = parseInt(hex.slice(1, 3), 16)
	const g = parseInt(hex.slice(3, 5), 16)
	const b = parseInt(hex.slice(5, 7), 16)
	return `rgba(${r},${g},${b},${alpha})`
}
</script>

<template>
	<!-- data-di-overlay="clone" identifies this as an InspectorClone in browser DevTools -->
	<div
		data-di-overlay="clone"
		:style="{
			position: 'fixed',
			border: `2px solid ${props.color}`,
			borderRadius: '2px',
			pointerEvents: 'none',
			zIndex: '9998',
			boxSizing: 'border-box',
			background: hex2rgba(props.color, 0.08),
			transition: 'none',
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
