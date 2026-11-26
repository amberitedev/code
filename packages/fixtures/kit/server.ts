import * as NodeFS from 'node:fs'
import { sharing } from '@amberite/contracts/backend'
import { core as coreApi, type Loader } from '@amberite/contracts/core'
import { installs } from '@amberite/contracts/engine'

import { call } from '../src/call.ts'
import { type App, type Core, FixtureError, step, until } from '../src/context.ts'
import { finished } from './instance.ts'

export interface Server {
	readonly kind: 'server'
	readonly id: string
	readonly name: string
	readonly core: Core
	/** The share its players read, or `null` when it has none. */
	readonly share: string | null
}

/**
 * A server on `core`, made and installed by the Core as its owner. `players` are invited from
 * the Play tab and have joined: each has the server's instance installed. With `running`, it is
 * started and has finished starting. With `pack`, the `.mrpack` at that path is then uploaded over
 * it, as setup and Installation settings upload one, and installed.
 */
export async function server(
	core: Core,
	spec: {
		readonly name: string
		readonly gameVersion: string
		/** As the Core names it, such as `Vanilla`, `Fabric` or `Paper`. */
		readonly loader?: Loader
		/** The newest when left out. */
		readonly loaderVersion?: string
		readonly players?: ReadonlyArray<App>
		readonly running?: boolean
		readonly pack?: string | URL
	},
): Promise<Server> {
	const { id } = await step(`the Core installs "${spec.name}"`, async () => {
		const made = await call(core, coreApi.CreateServer, null, { name: spec.name })
		await call(
			core,
			coreApi.InstallLoader,
			{ id: made.id },
			{
				loader: spec.loader ?? 'Vanilla',
				loader_version: spec.loaderVersion ?? null,
				game_version: spec.gameVersion,
				// A long op: the Core answers at once, and the wait below sees it end.
				op: crypto.randomUUID(),
			},
		)
		await until(
			`"${spec.name}" installs`,
			async () => {
				const now = await call(core, coreApi.GetServer, { id: made.id })
				return now.status !== 'installing' && now.loader !== null
			},
			// A first install downloads Minecraft, the loader and Java.
			{ timeoutMs: 15 * 60_000, everyMs: 500 },
		)
		return made
	})
	const players = spec.players ?? []
	const share = players.length > 0 ? await invite(core, id, spec.name, players) : null
	if (spec.pack) {
		const pack = new Blob([NodeFS.readFileSync(spec.pack)])
		await step(`a pack is uploaded over "${spec.name}"`, async () => {
			await call(core, coreApi.InstallModpackFile, { id }, pack)
			await until(
				`the pack installs on "${spec.name}"`,
				async () => (await call(core, coreApi.GetServer, { id })).status !== 'installing',
				{ timeoutMs: 15 * 60_000, everyMs: 500 },
			)
			// The server is let go however the install ended; its job says whether it worked.
			const [job] = (
				await call(core, installs.ListInstallJobs, null, undefined, { include_finished: true })
			)
				.filter(
					(each) => each.instance_id === id && each.kind === 'install_pack_to_existing_instance',
				)
				.sort((a, b) => b.created.localeCompare(a.created))
			if (job?.status !== 'succeeded')
				throw new FixtureError(
					`The pack did not install on "${spec.name}": ${JSON.stringify(job?.error ?? job?.status ?? 'no install job')}`,
				)
		})
	}
	if (spec.running) {
		await step(`"${spec.name}" starts`, async () => {
			await call(core, coreApi.ChangePower, { id }, { action: 'Start', op: crypto.randomUUID() })
			await until(`"${spec.name}" is running`, async () => {
				const { power_state } = await call(core, coreApi.GetServer, { id })
				return power_state === 'running'
			})
		})
	}
	return { kind: 'server', id, name: spec.name, core, share }
}

/** Shares the server and has each of `players` join it from an invite. Returns the share. */
async function invite(core: Core, id: string, name: string, players: ReadonlyArray<App>) {
	if (!core.owner) throw new FixtureError('Players need a Core with an owner to invite them.')
	const owner = core.owner
	const { shared_instance_id: share } = await call(
		core,
		coreApi.ShareServer,
		{ id },
		{ file_paths: [], removed_paths: [], restart: false },
	)
	const { net } = await call(core, coreApi.GetServer, { id })
	const invited = await call(
		owner,
		sharing.InvitePlayers,
		{ core_id: core.id, server_id: id },
		{
			user_ids: players.map((player) => player.account.id),
			instance_id: share,
			name,
			port: net.game_port,
			outside: null,
		},
	)
	if (invited.failed.length > 0)
		throw new FixtureError(
			`${owner.name} could not invite players to "${name}": ${invited.failed.join(', ')}. An invite needs them to be friends.`,
		)
	for (const player of players) {
		await call(player, installs.AcceptPendingSharedInstanceInvite, { id: share })
		await step(`${player.name} joins "${name}"`, async () =>
			finished(
				player,
				await call(
					player,
					installs.InstallSharedInstance,
					{ id: share },
					{
						name,
						manager_id: null,
						server_manager_name: name,
						server_manager_icon_url: null,
						instance_icon_url: null,
						instance_icon_config: null,
					},
				),
			),
		)
	}
	return share
}
