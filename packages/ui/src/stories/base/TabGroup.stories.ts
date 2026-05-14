import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { ref } from 'vue'

import TabGroup from '../../components/base/TabGroup.vue'

const meta = {
	title: 'Base/TabGroup',
	component: TabGroup,
} satisfies Meta<typeof TabGroup>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
	render: () => ({
		components: { TabGroup },
		setup() {
			const active = ref('client')
			const tabs = [
				{ id: 'client', label: 'Client' },
				{ id: 'server', label: 'Server' },
			]
			return { active, tabs }
		},
		template: /* html */ `
			<TabGroup v-model="active" :tabs="tabs">
				<div style="color: var(--color-primary); padding: 0.5rem 0;">
					Showing content for the <strong>{{ active }}</strong> tab.
				</div>
			</TabGroup>
		`,
	}),
}

export const ThreeTabs: Story = {
	render: () => ({
		components: { TabGroup },
		setup() {
			const active = ref('overview')
			const tabs = [
				{ id: 'overview', label: 'Overview' },
				{ id: 'settings', label: 'Settings' },
				{ id: 'logs', label: 'Logs' },
			]
			return { active, tabs }
		},
		template: /* html */ `
			<TabGroup v-model="active" :tabs="tabs">
				<div style="color: var(--color-primary); padding: 0.5rem 0;">
					Active tab: <strong>{{ active }}</strong>
				</div>
			</TabGroup>
		`,
	}),
}
