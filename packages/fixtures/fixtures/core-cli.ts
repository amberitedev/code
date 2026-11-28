// A paired Core with a running server, a stopped one, and a backup: what the `copal` commands
// have to show. Its data folder is `.data/cores/1`; run the commands on it with `CORE_DATA_DIR`.

import { core } from '@amberite/contracts/core'
import { call, server, stack } from '@amberite/fixtures'

const {
	cores: [machine],
} = await stack({ apps: ['alice'], cores: [{ owner: 'alice' }] })

await call(machine, core.AcceptEula)

// One at a time: installs that run together can race on the libraries they share.
await server(machine, { name: 'Survival', loader: 'Fabric', gameVersion: '1.21.1', running: true })
const creative = await server(machine, { name: 'Creative', gameVersion: '1.20.4' })

// Of the stopped server: a world that is still being made can change under a backup.
await call(machine, core.CreateBackup, { id: creative.id }, { name: 'Fresh start' })
