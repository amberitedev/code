import { provideInstanceServerSource } from '@modrinth/ui'

import { pushInstanceToServer } from '@/features/servers/linked-servers'
import { getInstanceIconUrl, list } from '@/helpers/instance'

/** Lets the hosting screens create and update a server from one of this App's instances. */
export function setupInstanceServerSourceProvider() {
	provideInstanceServerSource({
		async listInstances() {
			return (await list())
				.filter((instance) => instance.install_stage === 'installed' && !instance.quarantined)
				.map((instance) => ({
					id: instance.id,
					name: instance.name,
					iconUrl: getInstanceIconUrl(instance.icon_path) ?? undefined,
				}))
		},
		async pushInstance({ serverId, instanceId, properties, restart }) {
			const instance = (await list()).find((candidate) => candidate.id === instanceId)
			if (!instance) {
				throw new Error('This instance is not on this device, so it cannot be pushed from here.')
			}
			await pushInstanceToServer({ id: instance.id, name: instance.name }, serverId, {
				properties,
				restart,
			})
		},
	})
}
