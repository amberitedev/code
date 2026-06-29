import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import ButtonStyled from '../../components/base/ButtonStyled.vue'
import TextMorph from '../../components/base/TextMorph.vue'

const labels = ['Launch server', 'Server launched', 'Sync complete', 'Backup ready']

const meta = {
	title: 'Base/TextMorph',
	component: TextMorph,
} satisfies Meta<typeof TextMorph>

export default meta
type Story = StoryObj<typeof meta>

export const Cycling: Story = {
	render: () => ({
		components: { ButtonStyled, TextMorph },
		setup() {
			const index = ref(0)
			const label = computed(() => labels[index.value])
			let interval: ReturnType<typeof setInterval> | undefined

			function next() {
				index.value = (index.value + 1) % labels.length
			}

			function previous() {
				index.value = (index.value + labels.length - 1) % labels.length
			}

			onMounted(() => {
				interval = setInterval(next, 1600)
			})

			onBeforeUnmount(() => {
				if (interval) clearInterval(interval)
			})

			return { label, next, previous }
		},
		template: /* html */ `
			<div class="grid gap-4">
				<TextMorph as="p" class-name="m-0 text-2xl font-bold text-contrast" :children="label" />
				<div class="flex gap-2">
					<ButtonStyled>
						<button type="button" @click="previous">Previous</button>
					</ButtonStyled>
					<ButtonStyled color="brand">
						<button type="button" @click="next">Next</button>
					</ButtonStyled>
				</div>
			</div>
		`,
	}),
}
