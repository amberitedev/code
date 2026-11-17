// A Core running Forge 1.7.10, NeoForge 1.20.1 and NeoForge 1.21.10 servers: the oldest Forge
// era, the NeoForge builds published under Forge's name, and NeoForge's one-step patching. Each
// installs from its server profile in meta/.

import { core } from '@amberite/contracts'
import { call, stack, until } from '@amberite/fixtures'

const {
	cores: [machine],
} = await stack({ apps: ['alice'], cores: [{ owner: 'alice' }] })

await call(machine, core.AcceptEula)

async function server(name: string, loader: string, gameVersion: string, loaderVersion: string) {
	const id = await call(machine, core.CreateServer, null, { name })
	await call(machine, core.InstallLoader, id, {
		loader,
		loader_version: loaderVersion,
		game_version: gameVersion,
	})
	await until(
		`${name} installs`,
		async () => {
			const now = await call(machine, core.GetServer, id)
			return !now.installing && now.loader === loader
		},
		{ timeoutMs: 15 * 60_000, everyMs: 1000 },
	)
	await call(machine, core.ChangePower, id, { action: 'start' })
	await until(
		`${name} starts`,
		async () => {
			const { power } = await call(machine, core.GetServer, id)
			if (power === 'crashed' || power === 'killed') throw new Error(`${name} ${power}`)
			return power === 'running'
		},
		{ timeoutMs: 10 * 60_000, everyMs: 1000 },
	)
}

// One at a time: installs that run together can race on the libraries they share.
await server('Forge 1.7.10', 'forge', '1.7.10', '10.13.4.1614')
await server('NeoForge 1.20.1', 'neoforge', '1.20.1', '47.1.106')
await server('NeoForge 1.21.10', 'neoforge', '1.21.10', '21.10.64')
