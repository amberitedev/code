<script setup lang="ts" generic="T extends AvatarGroupItem">
import { useResizeObserver } from '@vueuse/core'
import { computed, ref } from 'vue'

import Avatar from './Avatar.vue'

export interface AvatarGroupItem {
	key: string
	name: string
	src?: string | null
	/** Faded into `--avatar-group-bg`, for something that is gone. */
	muted?: boolean
	/** Given a soft blue glow, for something that changed. */
	highlighted?: boolean
}

// Avatars fanned out like a hand of cards, as many as fit the space the group is given, then the
// count of the rest. Around the pointer the fan straightens, spreads and lifts, so the group
// reacts anywhere over it, not just over an avatar. `--avatar-group-bg` should match the
// background behind the group.

const props = withDefaults(defineProps<{ items: T[]; size?: number }>(), { size: 40 })

const OVERLAP = 14
/** How far the pointer's pull reaches, in pixels. */
const REACH = 90
/** Room kept for the count of the rest. */
const COUNT_WIDTH = 44

const root = ref<HTMLElement>()
const width = ref(0)
useResizeObserver(root, ([entry]) => (width.value = entry.contentRect.width))

const tileWidth = computed(() => props.size + 6)
const visible = computed(() => {
	if (!width.value) return props.items.slice(0, 8)
	const step = tileWidth.value - OVERLAP
	const fits = (room: number) => Math.max(1, Math.floor((room - tileWidth.value) / step) + 1)
	if (fits(width.value) >= props.items.length) return props.items
	return props.items.slice(0, fits(width.value - COUNT_WIDTH))
})
const rest = computed(() => props.items.length - visible.value.length)

const tiles: HTMLElement[] = []
/** Where each avatar sits when nothing pulls on it, measured as the pointer comes in. */
let centers: number[] = []
let frame = 0
const pull = ref<number[]>([])

function enter() {
	centers = tiles.slice(0, visible.value.length).map((tile) => {
		const rect = tile.getBoundingClientRect()
		return rect.left + rect.width / 2
	})
}
function move(event: PointerEvent) {
	cancelAnimationFrame(frame)
	frame = requestAnimationFrame(() => {
		pull.value = centers.map((center) =>
			Math.exp(-((event.clientX - center) ** 2) / (2 * REACH ** 2)),
		)
	})
}
function leave() {
	cancelAnimationFrame(frame)
	pull.value = []
}
</script>

<template>
	<div
		ref="root"
		class="flex min-w-0 items-center"
		:style="{ '--size': `${size}px` }"
		@pointerenter="enter"
		@pointermove="move"
		@pointerleave="leave"
	>
		<span
			v-for="(item, index) in visible"
			:key="item.key"
			:ref="(el) => (tiles[index] = el as HTMLElement)"
			v-tooltip="item.name"
			class="avatar-group-tile"
			:class="{ muted: item.muted, highlighted: item.highlighted }"
			:style="{ '--f': pull[index] ?? 0, zIndex: Math.round((pull[index] ?? 0) * 10) }"
		>
			<Avatar
				:src="item.src"
				:tint-by="item.name"
				:alt="item.name"
				:size="`${size}px`"
				pad-transparent-corners
				no-shadow
			/>
		</span>
		<span v-if="rest" class="ml-3 shrink-0 text-sm font-semibold tabular-nums text-secondary">
			+{{ rest }}
		</span>
	</div>
</template>

<style scoped>
.avatar-group-tile {
	--f: 0;
	--tilt: -5deg;
	position: relative;
	/* Not inline, which would leave line space under the avatar. */
	display: flex;
	flex-shrink: 0;
	padding: 3px;
	border-radius: calc(16 / 96 * var(--size) + 3px);
	background: var(--surface-4);
	box-shadow:
		0 2px 6px rgb(0 0 0 / 50%),
		inset 0 1px 0 rgb(255 255 255 / 7%);
	transform-origin: 50% 100%;
	transform: rotate(calc(var(--tilt) * (1 - var(--f)))) translateY(calc(-8px * var(--f)))
		scale(calc(1 + 0.22 * var(--f)));
	transition:
		transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1),
		margin 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.avatar-group-tile:nth-child(even) {
	--tilt: 4deg;
}
.avatar-group-tile + .avatar-group-tile {
	margin-left: calc(-14px + 16px * var(--f));
}
.avatar-group-tile.muted {
	background: var(--avatar-group-bg, var(--surface-3));
	box-shadow: inset 0 0 0 1.5px var(--surface-5);
}
.avatar-group-tile.highlighted {
	box-shadow:
		0 0 12px 1px color-mix(in srgb, var(--color-blue) 60%, transparent),
		inset 0 1px 0 rgb(255 255 255 / 7%);
}
/* Doubled to outrank Avatar's own outline rule. */
.avatar-group-tile :deep(.avatar.avatar) {
	outline: none;
}
.avatar-group-tile.muted :deep(.avatar) {
	filter: brightness(0.5) grayscale(1);
}

@media (prefers-reduced-motion: reduce) {
	.avatar-group-tile {
		transition: none;
	}
}
</style>
