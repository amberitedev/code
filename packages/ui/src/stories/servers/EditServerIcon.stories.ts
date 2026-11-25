import type { Meta, StoryObj } from '@storybook/vue3-vite'

import EditServerIcon from '../../components/servers/edit-server-icon/EditServerIcon.vue'
import { provideModrinthServerContext } from '../../providers'
import { storyServerContext } from './story-server-context'

const meta = {
	title: 'Servers/EditServerIcon',
	component: EditServerIcon,
	parameters: {
		layout: 'padded',
	},
	decorators: [
		(story) => ({
			components: { story },
			setup() {
				provideModrinthServerContext(storyServerContext())
			},
			template: '<div style="max-width: 320px;"><story /></div>',
		}),
	],
} satisfies Meta<typeof EditServerIcon>

export default meta

type Story = StoryObj<typeof meta>

export const Default: Story = {}

export const ReadOnly: Story = {
	args: {
		canEdit: false,
		permissionDeniedMessage: 'You cannot edit this server icon.',
	},
}
