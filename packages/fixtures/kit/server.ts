import { core as coreApi, installs, sharing } from '@amberite/contracts'

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
 * started and has finished starting.
 */
export async function server(
	core: Core,
	spec: {
		readonly name: string
		readonly gameVersion: string
		/** As the Core names it, such as `vanilla`, `fabric` or `paper`. */
		readonly loader?: string
		/** The newest when left out. */
		readonly loaderVersion?: string
		readonly players?: ReadonlyArray<App>
		readonly running?: boolean
	},
): Promise<Server> {
	const { id } = await step(`the Core installs "${spec.name}"`, async () => {
		const made = await call(core, coreApi.CreateServer, null, { name: spec.name })
		await call(
			core,
			coreApi.InstallLoader,
			{ id: made.id },
			{
				loader: spec.loader ?? 'vanilla',
				loader_version: spec.loaderVersion ?? null,
				game_version: spec.gameVersion,
			},
		)
		await until(
			`"${spec.name}" installs`,
			async () => {
				const now = await call(core, coreApi.GetServer, { id: made.id })
				return !now.installing && now.loader !== null
			},
			// A first install downloads Minecraft, the loader and Java.
			{ timeoutMs: 15 * 60_000, everyMs: 500 },
		)
		return made
	})
	if (spec.running) {
		await step(`"${spec.name}" starts`, async () => {
			await call(core, coreApi.ChangePower, { id }, { action: 'start' })
			await until(`"${spec.name}" is running`, async () => {
				const { power } = await call(core, coreApi.GetServer, { id })
				return power === 'running'
			})
		})
	}
	const players = spec.players ?? []
	if (players.length === 0) return { kind: 'server', id, name: spec.name, core, share: null }
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
			name: spec.name,
			port: net.game_port,
			outside: null,
		},
	)
	if (invited.failed.length > 0)
		throw new FixtureError(
			`${owner.name} could not invite players to "${spec.name}": ${invited.failed.join(', ')}. An invite needs them to be friends.`,
		)
	for (const player of players) {
		await call(player, installs.AcceptPendingSharedInstanceInvite, { id: share })
		await step(`${player.name} joins "${spec.name}"`, async () =>
			finished(
				player,
				await call(
					player,
					installs.InstallSharedInstance,
					{ id: share },
					{
						name: spec.name,
						manager_id: null,
						server_manager_name: spec.name,
						server_manager_icon_url: null,
						instance_icon_url: null,
						instance_icon_config: null,
					},
				),
			),
		)
	}
	return { kind: 'server', id, name: spec.name, core, share }
}
