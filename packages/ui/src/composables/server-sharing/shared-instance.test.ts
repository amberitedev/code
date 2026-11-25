import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { expect, test, vi } from 'vitest'
import { createApp, effectScope, ref } from 'vue'

import { useSharedInstanceInstalledOnce } from './shared-instance'

test('one shared_instance_users event reads the share once for every panel showing it', async () => {
	const listeners: ((notification: unknown) => void)[] = []
	const get = vi.fn(async () => ({ installed_once: true }))
	const queryClient = new QueryClient()
	const app = createApp({})
	app.use(VueQueryPlugin, { queryClient })
	// `provide*` needs a component; the App provides the same keys.
	app.provide(Symbol.for('modrinth:modrinthClient'), {
		sharedinstances: { instances_v1: { get } },
		localEvents: {
			on: (type: string, listener: (notification: unknown) => void) => {
				if (type === 'notification') listeners.push(listener)
				return () => {}
			},
		},
	})
	app.provide(Symbol.for('modrinth:auth'), { user: ref({ id: 'me' }) })
	app.runWithContext(() => {
		const instanceId = ref<string | null>('share')
		effectScope().run(() => {
			for (let panel = 0; panel < 3; panel++) useSharedInstanceInstalledOnce(instanceId)
		})
	})
	const queryKey = ['shared-instances', 'share']
	await queryClient.refetchQueries({ queryKey }, { cancelRefetch: false })
	get.mockClear()

	for (const listener of listeners)
		listener({ body: { type: 'shared_instance_users', shared_instance_id: 'share' } })
	await queryClient.refetchQueries({ queryKey }, { cancelRefetch: false })

	expect(listeners).toHaveLength(3)
	expect(get).toHaveBeenCalledTimes(1)
})
