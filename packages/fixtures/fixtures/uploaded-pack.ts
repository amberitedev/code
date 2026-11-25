// Servers set up from an uploaded `.mrpack` (Fabric 1.21.1, two mods and a config), which a share
// cannot carry: "Uploaded" is not shared yet, "Shared then uploaded" was shared with bob before
// the pack went over it, and "Blank" is left for setup in the UI.

import { core } from '@amberite/contracts/core'
import { call, friends, server, stack } from '@amberite/fixtures'

const {
	alice,
	bob,
	cores: [machine],
} = await stack({ apps: ['alice', 'bob'], cores: [{ owner: 'alice' }] })

await friends(alice, bob)
await call(machine, core.AcceptEula)

const pack = new URL('../packs/drop-check.mrpack', import.meta.url)
const fabric = { loader: 'Fabric', gameVersion: '1.21.1', pack } as const
await server(machine, { name: 'Uploaded', ...fabric })
await server(machine, { name: 'Shared then uploaded', ...fabric, players: [bob] })
await call(machine, core.CreateServer, null, { name: 'Blank' })
