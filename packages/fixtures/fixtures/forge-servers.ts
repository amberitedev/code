// A Core running Forge 1.7.10, NeoForge 1.20.1 and NeoForge 1.21.10 servers: the oldest Forge
// era, the NeoForge builds published under Forge's name, and NeoForge's one-step patching. Each
// installs from its server profile in meta/.

import { core } from '@amberite/contracts'
import { call, server, stack } from '@amberite/fixtures'

const {
	cores: [machine],
} = await stack({ apps: ['alice'], cores: [{ owner: 'alice' }] })

await call(machine, core.AcceptEula)

// One at a time: installs that run together can race on the libraries they share.
for (const [name, loader, gameVersion, loaderVersion] of [
	['Forge 1.7.10', 'Forge', '1.7.10', '10.13.4.1614'],
	['NeoForge 1.20.1', 'NeoForge', '1.20.1', '47.1.106'],
	['NeoForge 1.21.10', 'NeoForge', '1.21.10', '21.10.64'],
] as const) {
	await server(machine, { name, loader, gameVersion, loaderVersion, running: true })
}
