import type { Meta, StoryObj } from '@storybook/vue3-vite'

import ServerListing from '../../components/servers/ServerListing.vue'

const baseServer = {
	server_id: '8759b459-6f9e-49e0-aa70-dfab5b2abb2f',
	name: 'Survival SMP',
	status: 'available',
	game: 'Minecraft',
	mc_version: '1.21.5',
	loader: 'Fabric',
	loader_version: '0.16.14',
	net: {
		ip: '203.0.113.42',
		port: 25565,
		outside: null,
		game_port: 25565,
	},
} as const

const meta = {
	title: 'Servers/ServerListing',
	component: ServerListing,
	parameters: {
		layout: 'padded',
	},
	decorators: [
		(story) => ({
			components: { story },
			template: '<div style="max-width: 920px;"><story /></div>',
		}),
	],
} satisfies Meta<typeof ServerListing>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
	args: {
		...baseServer,
	},
}

export const SharedWithOwner: Story = {
	args: {
		...baseServer,
		name: 'Cobbletown',
		owner: {
			username: 'Prospector',
		},
	},
}

export const ConfiguringNewServer: Story = {
	args: {
		...baseServer,
		name: 'Fresh Vanilla World',
		status: 'installing',
		flows: { intro: true },
		loader: 'Vanilla',
		loader_version: null,
	},
}

export const Provisioning: Story = {
	args: {
		...baseServer,
		name: 'My New Server',
		isProvisioning: true,
	},
}
