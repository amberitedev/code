import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { ref } from 'vue'

import GhostText from '../../components/base/GhostText.vue'
import UiLazyMount from '../../components/base/UiLazyMount.vue'

const meta = {
	title: 'Base/UiLazyMount',
	component: UiLazyMount,
} satisfies Meta<typeof UiLazyMount>

export default meta
type Story = StoryObj<typeof meta>

export const Visible: Story = {
	render: () => ({
		components: { GhostText, UiLazyMount },
		template: `
			<div class="grid max-w-xl gap-4">
				<div class="rounded bg-bg-raised p-4 text-secondary">Scroll down to intersect the lazy mount target.</div>
				<div class="h-[420px] rounded bg-surface-2" />
				<UiLazyMount mode="visible" root-margin="0px">
					<div class="rounded bg-bg-raised p-4 text-contrast">Visible mode mounted.</div>
					<template #fallback>
						<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
					</template>
				</UiLazyMount>
			</div>
		`,
	}),
}

export const Idle: Story = {
	render: () => ({
		components: { GhostText, UiLazyMount },
		template: `
			<UiLazyMount mode="idle" :idle-timeout-ms="1200" class="max-w-xl">
				<div class="rounded bg-bg-raised p-4 text-contrast">Idle mode mounted.</div>
				<template #fallback>
					<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
				</template>
			</UiLazyMount>
		`,
	}),
}

export const Delay: Story = {
	render: () => ({
		components: { GhostText, UiLazyMount },
		template: `
			<UiLazyMount mode="delay" :delay-ms="900" class="max-w-xl">
				<div class="rounded bg-bg-raised p-4 text-contrast">Delay mode mounted after 900ms.</div>
				<template #fallback>
					<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
				</template>
			</UiLazyMount>
		`,
	}),
}

export const Immediate: Story = {
	render: () => ({
		components: { UiLazyMount },
		template: `
			<UiLazyMount mode="immediate" class="max-w-xl">
				<div class="rounded bg-bg-raised p-4 text-contrast">Immediate mode renders content right away.</div>
				<template #fallback>
					<div class="rounded bg-bg-raised p-4 text-secondary">Fallback should not render.</div>
				</template>
			</UiLazyMount>
		`,
	}),
}

export const ContentKeyReset: Story = {
	render: () => ({
		components: { GhostText, UiLazyMount },
		setup() {
			const contentKey = ref(1)
			return { contentKey }
		},
		template: `
			<div class="grid max-w-xl gap-3">
				<button class="w-fit rounded bg-button-bg px-3 py-2 font-semibold text-button-text" @click="contentKey++">
					Change key
				</button>
				<UiLazyMount mode="delay" :delay-ms="700" :content-key="contentKey">
					<div class="rounded bg-bg-raised p-4 text-contrast">Mounted content key {{ contentKey }}.</div>
					<template #fallback>
						<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
					</template>
				</UiLazyMount>
			</div>
		`,
	}),
}

export const CleanupOnUnmount: Story = {
	render: () => ({
		components: { GhostText, UiLazyMount },
		setup() {
			const shown = ref(true)
			return { shown }
		},
		template: `
			<div class="grid max-w-xl gap-3">
				<button class="w-fit rounded bg-button-bg px-3 py-2 font-semibold text-button-text" @click="shown = !shown">
					Toggle mount
				</button>
				<UiLazyMount v-if="shown" mode="delay" :delay-ms="2000">
					<div class="rounded bg-bg-raised p-4 text-contrast">Mounted after delayed timer.</div>
					<template #fallback>
						<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
					</template>
				</UiLazyMount>
			</div>
		`,
	}),
}
