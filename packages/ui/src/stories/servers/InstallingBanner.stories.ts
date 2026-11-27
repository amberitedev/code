import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { computed, ref } from 'vue'

import InstallingBanner from '../../components/servers/InstallingBanner.vue'
import type { ServerInstallationState } from '../../composables/server-installation-tracker'
import { provideModrinthServerContext } from '../../providers'
import { storyServerContext } from './story-server-context'

function renderInstallation(state: ServerInstallationState) {
	return () => ({
		components: { InstallingBanner },
		setup() {
			const installation = ref<ServerInstallationState | null>(state)
			provideModrinthServerContext(
				storyServerContext({
					server: { status: 'installing', power_state: 'stopped' },
					powerState: ref('stopped'),
					isServerRunning: computed(() => false),
					installation: computed(() => installation.value),
					dismissInstallation: () => {
						installation.value = null
					},
				}),
			)
		},
		template: '<InstallingBanner />',
	})
}

const meta = {
	title: 'Servers/InstallingBanner',
	component: InstallingBanner,
} satisfies Meta<typeof InstallingBanner>

export default meta
type Story = StoryObj<typeof meta>

export const PlatformPending: Story = {
	render: renderInstallation({
		id: 'platform:fabric:0.16.14:1.21.1',
		key: {
			type: 'platform',
			platform: 'fabric',
			platform_version: '0.16.14',
			game_version: '1.21.1',
		},
		status: 'pending',
		progress: null,
		error: null,
		source: 'optimistic',
	}),
}

export const PlatformProgress: Story = {
	render: renderInstallation({
		id: 'platform:fabric:0.16.14:1.21.1',
		key: {
			type: 'platform',
			platform: 'fabric',
			platform_version: '0.16.14',
			game_version: '1.21.1',
		},
		status: 'installing',
		progress: 45,
		error: null,
		source: 'job',
	}),
}

export const Vanilla: Story = {
	render: renderInstallation({
		id: 'platform:vanilla::1.21.1',
		key: {
			type: 'platform',
			platform: 'vanilla',
			platform_version: '',
			game_version: '1.21.1',
		},
		status: 'installing',
		progress: 20,
		error: null,
		source: 'job',
	}),
}

export const Modpack: Story = {
	render: renderInstallation({
		id: 'modrinth-modpack:project:version',
		key: {
			type: 'modrinth_modpack',
			project_id: 'project',
			version_id: 'version',
		},
		status: 'installing',
		progress: 72,
		error: null,
		source: 'job',
	}),
}

export const PlatformFailed: Story = {
	render: renderInstallation({
		id: 'platform:quilt:0.30.1-beta.2:1.14.4',
		key: {
			type: 'platform',
			platform: 'quilt',
			platform_version: '0.30.1-beta.2',
			game_version: '1.14.4',
		},
		status: 'failed',
		progress: null,
		error: 'Platform installer failed',
		source: 'job',
	}),
}

export const ModpackFailed: Story = {
	render: renderInstallation({
		id: 'modrinth-modpack:project:version',
		key: {
			type: 'modrinth_modpack',
			project_id: 'project',
			version_id: 'version',
		},
		status: 'failed',
		progress: null,
		error: 'The modpack does not include a downloadable file',
		source: 'job',
	}),
}

export const LocalModpackFailed: Story = {
	render: renderInstallation({
		id: 'local-modpack:example.mrpack',
		key: {
			type: 'local_modpack',
			filename: 'example.mrpack',
		},
		status: 'failed',
		progress: null,
		error: 'The modpack could not be read',
		source: 'job',
	}),
}
