import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { getCurrentInstance } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import GhostBlock from '../../components/base/GhostBlock.vue'
import GhostControl from '../../components/base/GhostControl.vue'
import GhostMedia from '../../components/base/GhostMedia.vue'
import GhostTabGroup from '../../components/base/GhostTabGroup.vue'
import GhostText from '../../components/base/GhostText.vue'

const router = createRouter({
	history: createMemoryHistory(),
	routes: [{ path: '/', component: { template: '<div />' } }],
})

const meta = {
	title: 'Base/GhostPrimitives',
	decorators: [
		(story) => ({
			components: { story },
			setup() {
				const app = getCurrentInstance()?.appContext.app
				if (app && !app.config.globalProperties.$router) {
					app.use(router)
				}
			},
			template: '<story />',
		}),
	],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Shapes: Story = {
	render: () => ({
		components: { GhostBlock },
		setup() {
			return {
				shapes: ['square', 'text', 'control', 'surface', 'pill', 'circle'],
			}
		},
		template: `
			<div class="grid max-w-xl grid-cols-2 gap-4 sm:grid-cols-3">
				<div v-for="shape in shapes" :key="shape" class="grid gap-2">
					<GhostBlock :shape="shape" class="h-12 w-full" />
					<span class="text-sm text-secondary">{{ shape }}</span>
				</div>
			</div>
		`,
	}),
}

export const TextMediaAndControls: Story = {
	render: () => ({
		components: { GhostControl, GhostMedia, GhostText },
		template: `
			<div class="grid max-w-3xl gap-6">
				<div class="grid gap-4 rounded bg-bg-raised p-4">
					<GhostText kind="title" width="48%" />
					<GhostText kind="body" :lines="3" />
					<GhostText kind="metadata" width="30%" />
				</div>
				<div class="flex flex-wrap items-center gap-4">
					<GhostMedia kind="square" class="w-16" />
					<GhostMedia kind="rounded" class="w-16" />
					<GhostMedia kind="circle" class="w-16" />
					<GhostMedia kind="banner" class="max-w-sm" />
				</div>
				<div class="flex flex-wrap items-center gap-3">
					<GhostControl kind="input" class="max-w-sm" />
					<GhostControl kind="select" />
					<GhostControl kind="button" />
					<GhostControl kind="icon-button" />
					<GhostControl kind="chip" size="small" />
					<GhostControl kind="pagination" />
				</div>
			</div>
		`,
	}),
}

export const TabGroup: Story = {
	render: () => ({
		components: { GhostTabGroup },
		template: `
			<div class="grid gap-4">
				<GhostTabGroup :count="4" />
				<GhostTabGroup :labels="['Overview', 'Members', 'Audit log']" :active-index="1" />
			</div>
		`,
	}),
}

export const StaticRendering: Story = {
	render: () => ({
		components: { GhostBlock, GhostControl, GhostTabGroup, GhostText },
		template: `
			<div class="grid max-w-xl gap-4">
				<GhostBlock class="h-20 w-full" :animated="false" />
				<GhostText :lines="3" :animated="false" />
				<div class="flex gap-3">
					<GhostControl kind="button" :animated="false" />
					<GhostControl kind="icon-button" :animated="false" />
				</div>
				<GhostTabGroup :animated="false" />
			</div>
		`,
	}),
}
