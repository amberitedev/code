import type { CoreSetupContext } from '../core_setup.ts'

interface Instance {
	id: string
	name: string
	install_status: 'installing' | 'ready' | 'failed'
}

interface InstanceList {
	instances: Instance[]
}

export const description = 'Creates one installed vanilla instance without starting it.'

export async function run({ request }: CoreSetupContext) {
	const current = await request<InstanceList>('GET', '/instances')
	const existing = current.instances.find((instance) => instance.name === 'Dev Server')
	const instance =
		existing ??
		(await request<Instance>('POST', '/instances', {
			name: 'Dev Server',
			game_version: '1.21.1',
			loader: 'vanilla',
			loader_version: null,
			port: 25565,
			memory: { min_mb: 512, max_mb: 2048 },
		}))

	const deadline = Date.now() + 300_000
	while (Date.now() < deadline) {
		const state = await request<Instance>('GET', `/instances/${instance.id}`)
		if (state.install_status === 'ready') return
		if (state.install_status === 'failed') throw new Error('Dev Server installation failed.')
		await new Promise((resolve) => setTimeout(resolve, 1_000))
	}
	throw new Error('Dev Server installation did not finish within five minutes.')
}
