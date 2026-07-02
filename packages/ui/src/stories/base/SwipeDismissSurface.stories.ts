import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { onBeforeUnmount, ref } from 'vue'

import ButtonStyled from '../../components/base/ButtonStyled.vue'
import SwipeDismissSurface from '../../components/base/SwipeDismissSurface.vue'

const meta = {
	title: 'Base/SwipeDismissSurface',
	component: SwipeDismissSurface,
} satisfies Meta<typeof SwipeDismissSurface>

export default meta
type Story = StoryObj<typeof meta>

interface DemoCard {
	id: string
	title: string
	body: string
}

const demoCards: DemoCard[] = [
	{
		id: 'slow',
		title: 'Slow drag and cancel',
		body: 'Drag partway and release before the one-third detent.',
	},
	{
		id: 'fling',
		title: 'Fling commit',
		body: 'A quick horizontal flick commits even before the wall.',
	},
	{
		id: 'mirrored',
		title: 'Left or right dismiss',
		body: 'The same action is mirrored from either edge.',
	},
	{
		id: 'wall',
		title: 'One-third wall clamp',
		body: 'The user drag snaps to a one-third-width wall, then fades over a short extra travel.',
	},
	{
		id: 'diagonal',
		title: 'Diagonal drag',
		body: 'Early vertical movement is ignored until horizontal intent is clear.',
	},
]

function createCards() {
	return demoCards.map((card) => ({ ...card }))
}

export const GestureMatrix: Story = {
	render: () => ({
		components: { ButtonStyled, SwipeDismissSurface },
		setup() {
			const cards = ref(createCards())
			const progress = ref<Record<string, number>>({})

			function dismiss(id: string) {
				cards.value = cards.value.filter((card) => card.id !== id)
			}

			function reset() {
				cards.value = createCards()
				progress.value = {}
			}

			function updateProgress(id: string, value: number) {
				progress.value = { ...progress.value, [id]: value }
			}

			return { cards, dismiss, progress, reset, updateProgress }
		},
		template: /* html */ `
			<div class="grid max-w-xl gap-4">
				<div class="flex justify-end">
					<ButtonStyled>
						<button type="button" @click="reset">Reset</button>
					</ButtonStyled>
				</div>
				<SwipeDismissSurface
					v-for="card in cards"
					:key="card.id"
					class="rounded-lg"
					aria-label="Dismiss card"
					@dismiss="dismiss(card.id)"
					@progress="updateProgress(card.id, $event)"
				>
					<div class="rounded-lg border border-solid border-surface-5 bg-surface-3 p-4 shadow-card">
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0">
								<h3 class="m-0 truncate text-base font-bold text-contrast">{{ card.title }}</h3>
								<p class="m-0 mt-1 text-sm text-secondary">{{ card.body }}</p>
							</div>
							<span class="text-xs font-semibold text-secondary">
								{{ Math.round((progress[card.id] ?? 0) * 100) }}% exit
							</span>
						</div>
					</div>
				</SwipeDismissSurface>
			</div>
		`,
	}),
}

export const Disabled: Story = {
	args: {
		disabled: true,
	},
	render: (args) => ({
		components: { SwipeDismissSurface },
		setup() {
			return { args }
		},
		template: /* html */ `
			<SwipeDismissSurface v-bind="args" class="max-w-xl rounded-lg">
				<div class="rounded-lg border border-solid border-surface-5 bg-surface-3 p-4">
					<h3 class="m-0 text-base font-bold text-contrast">Disabled surface</h3>
					<p class="m-0 mt-1 text-sm text-secondary">Pointer gestures are ignored.</p>
				</div>
			</SwipeDismissSurface>
		`,
	}),
}

export const ReducedMotion: Story = {
	render: () => ({
		components: { SwipeDismissSurface },
		setup() {
			const originalMatchMedia = window.matchMedia
			window.matchMedia = ((query: string) => {
				if (query === '(prefers-reduced-motion: reduce)') {
					return {
						matches: true,
						media: query,
						onchange: null,
						addListener: () => undefined,
						removeListener: () => undefined,
						addEventListener: () => undefined,
						removeEventListener: () => undefined,
						dispatchEvent: () => false,
					} as MediaQueryList
				}

				return originalMatchMedia.call(window, query)
			}) as typeof window.matchMedia

			onBeforeUnmount(() => {
				window.matchMedia = originalMatchMedia
			})
		},
		template: /* html */ `
			<SwipeDismissSurface class="max-w-xl rounded-lg" aria-label="Dismiss card">
				<div class="rounded-lg border border-solid border-surface-5 bg-surface-3 p-4">
					<h3 class="m-0 text-base font-bold text-contrast">Reduced motion</h3>
					<p class="m-0 mt-1 text-sm text-secondary">Commit and cancel snaps complete without animated settling.</p>
				</div>
			</SwipeDismissSurface>
		`,
	}),
}
