import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { onMounted, ref } from 'vue'

import GhostBlock from '../../components/base/GhostBlock.vue'
import GhostText from '../../components/base/GhostText.vue'
import LoadingBar from '../../components/base/LoadingBar.vue'
import ReadyTransition from '../../components/base/ReadyTransition.vue'
import { createLoadingStateCore } from '../../composables/use-loading-state-core'
import { provideLoadingState } from '../../providers/loading-state'

const meta = {
	title: 'Base/ReadyTransition',
	component: ReadyTransition,
} satisfies Meta<typeof ReadyTransition>

export default meta
type Story = StoryObj<typeof meta>

function provideStoryLoadingState() {
	provideLoadingState(createLoadingStateCore())
}

export const CacheHit: Story = {
	render: () => ({
		components: { ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			return { pending: ref(false) }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending">
					<div class="rounded bg-bg-raised p-4 text-contrast">Cached content visible immediately.</div>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const GhostDelay: Story = {
	render: () => ({
		components: { GhostBlock, GhostText, ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			const pending = ref(true)
			onMounted(() => {
				setTimeout(() => (pending.value = false), 900)
			})
			return { pending }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending" :ghost-delay-ms="300">
					<div class="rounded bg-bg-raised p-4 text-contrast">Content after delayed ghost.</div>
					<template #ghost="{ state }">
						<div class="grid gap-3 rounded bg-bg-raised p-4">
							<GhostText kind="title" width="55%" />
							<GhostBlock class="h-20 w-full" />
							<div class="text-secondary text-sm">{{ state }}</div>
						</div>
					</template>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const MinimumGhostDuration: Story = {
	render: () => ({
		components: { GhostText, ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			const pending = ref(true)
			onMounted(() => {
				setTimeout(() => (pending.value = false), 120)
			})
			return { pending }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending" :ghost-delay-ms="0" :minimum-ghost-ms="1000">
					<div class="rounded bg-bg-raised p-4 text-contrast">Fast result waits for minimum ghost duration.</div>
					<template #ghost>
						<div class="grid gap-3 rounded bg-bg-raised p-4">
							<GhostText kind="title" width="45%" />
							<GhostText :lines="3" />
						</div>
					</template>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const Timeout: Story = {
	render: () => ({
		components: { ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			return { pending: ref(true) }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending" :timeout-ms="900">
					<div class="rounded bg-bg-raised p-4 text-contrast">Resolved content.</div>
					<template #timeout="{ timedOut, state }">
						<div class="rounded bg-bg-raised p-4 text-secondary">
							Timeout slot. timedOut={{ timedOut }} state={{ state }}
						</div>
					</template>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const ErrorState: Story = {
	render: () => ({
		components: { ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			const pending = ref(true)
			const error = ref<Error | null>(null)
			onMounted(() => {
				setTimeout(() => (error.value = new Error('Unable to load the panel')), 500)
			})
			function formatError(value: unknown) {
				return value instanceof Error ? value.message : String(value ?? '')
			}
			return { error, formatError, pending }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending" :error="error">
					<div class="rounded bg-bg-raised p-4 text-contrast">Resolved content.</div>
					<template #error="{ error, state }">
						<div class="rounded bg-bg-raised p-4 text-secondary">
							{{ state }}: {{ formatError(error) }}
						</div>
					</template>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const Silent: Story = {
	render: () => ({
		components: { ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			const pending = ref(true)
			onMounted(() => {
				setTimeout(() => (pending.value = false), 1200)
			})
			return { pending }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending" silent>
					<div class="rounded bg-bg-raised p-4 text-contrast">Silent content reveal without loading-bar token.</div>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const ContentKeyChanges: Story = {
	render: () => ({
		components: { GhostText, ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			const pending = ref(false)
			const contentKey = ref('alpha')
			const label = ref('Alpha content')
			function loadNext() {
				contentKey.value = contentKey.value === 'alpha' ? 'beta' : 'alpha'
				pending.value = true
				setTimeout(() => {
					label.value = contentKey.value === 'alpha' ? 'Alpha content' : 'Beta content'
					pending.value = false
				}, 700)
			}
			return { contentKey, label, loadNext, pending }
		},
		template: `
			<div class="relative grid gap-3">
				<LoadingBar />
				<button class="w-fit rounded bg-button-bg px-3 py-2 font-semibold text-button-text" @click="loadNext">
					Change content key
				</button>
				<ReadyTransition :pending="pending" :content-key="contentKey">
					<div class="rounded bg-bg-raised p-4 text-contrast">{{ label }}</div>
					<template #ghost>
						<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
					</template>
				</ReadyTransition>
			</div>
		`,
	}),
}

export const KeepPrevious: Story = {
	render: () => ({
		components: { GhostText, ReadyTransition, LoadingBar },
		setup() {
			provideStoryLoadingState()
			const pending = ref(false)
			const contentKey = ref('one')
			const label = ref('Previous keyed content')
			onMounted(() => {
				setTimeout(() => {
					contentKey.value = 'two'
					pending.value = true
				}, 600)
				setTimeout(() => {
					label.value = 'Resolved keyed content'
					pending.value = false
				}, 1800)
			})
			return { contentKey, label, pending }
		},
		template: `
			<div class="relative">
				<LoadingBar />
				<ReadyTransition :pending="pending" :content-key="contentKey" keep-previous>
					<div class="rounded bg-bg-raised p-4 text-contrast">{{ label }}</div>
					<template #ghost>
						<div class="rounded bg-bg-raised p-4"><GhostText :lines="2" /></div>
					</template>
				</ReadyTransition>
			</div>
		`,
	}),
}
