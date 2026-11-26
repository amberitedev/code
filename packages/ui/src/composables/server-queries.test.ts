import type { Server } from '@amberite/contracts/core'
import type { AbstractModrinthClient } from '@modrinth/api-client'
import { expect, test } from 'vitest'

import { serverListQueryOptions, useServerNames } from './server-queries'

const listing = (names: Record<string, string>, offline_cores: string[] = []) =>
	serverListQueryOptions({
		cores: {
			servers: async () => ({
				servers: Object.entries(names).map(([server_id, name]) => ({ server_id, name }) as Server),
				offline_cores,
			}),
		},
	} as unknown as AbstractModrinthClient).queryFn()

test('a server keeps its last name while its Core is off, and loses it once every Core answers without it', async () => {
	await listing({ home: 'Survival', away: 'Creative' })

	await listing({ home: 'Survival 2' }, ['away-core'])
	expect(useServerNames().value).toEqual({ home: 'Survival 2', away: 'Creative' })

	await listing({ home: 'Survival 2' })
	expect(useServerNames().value).toEqual({ home: 'Survival 2' })
})
