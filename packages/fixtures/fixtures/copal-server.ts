// A Core with one Fabric server that is not running.

import { core } from '@amberite/contracts/core'
import { call, server, stack } from '@amberite/fixtures'

const {
	cores: [machine],
} = await stack({ apps: ['alice'], cores: [{ owner: 'alice' }] })

await call(machine, core.AcceptEula)
await server(machine, { name: 'Survival', loader: 'Fabric', gameVersion: '1.21.1' })
