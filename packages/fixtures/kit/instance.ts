import * as NodeCrypto from 'node:crypto'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import {
	installs,
	instanceContent,
	instances,
	instanceSharing,
	type InstallJobSnapshot,
	localInstalls,
	localSharing,
	type ModLoader,
} from '@amberite/contracts'

import { call } from '../src/call.ts'
import { type App, FixtureError, state, step, until } from '../src/context.ts'
import { ended } from '../src/ops.ts'
import { pinnedVersion } from './mods.ts'

export interface Instance {
	readonly kind: 'instance'
	readonly id: string
	readonly name: string
	readonly owner: App
	/** Its folder in the owner's App, with `mods/`, `config/` and the rest. */
	readonly dir: string
	/** The share's id on the backend, or `null` when it is not shared. */
	readonly share: string | null
}

/**
 * An instance in `owner`'s App, made by the App: installed, with its mods, and shared when
 * `sharedWith` names anyone. Each member's App accepts the invite and installs its own copy.
 * Returns once all of that has finished.
 *
 * - `mods`: names from the pinned list in `mods.ts`, or Modrinth version ids.
 * - `files`: loose files by their path in the instance, such as `config/example.toml`. Sharing
 *   them needs a paired Core in the stack, because a share's files are kept on one.
 * - `sharedWith`: Apps of the owner's friends.
 */
export async function instance(
	owner: App,
	spec: {
		readonly name: string
		readonly gameVersion: string
		readonly loader?: ModLoader
		readonly loaderVersion?: string
		readonly mods?: ReadonlyArray<string>
		readonly files?: Readonly<Record<string, string>>
		readonly sharedWith?: ReadonlyArray<App>
	},
): Promise<Instance> {
	const loader = spec.loader ?? 'vanilla'
	const members = spec.sharedWith ?? []
	const files = Object.entries(spec.files ?? {})
	const mods = (spec.mods ?? []).map((mod) => ({
		mod,
		version: pinnedVersion(mod, loader, spec.gameVersion),
	}))
	const paired = state.stack?.cores.some((core) => core.owner !== null)
	if (members.length > 0 && files.length > 0 && !paired) {
		throw new FixtureError(
			`"${spec.name}" shares loose files, which are kept on a paired Core, and this stack has none. Add one: cores: [{ owner: '${owner.name}' }]`,
		)
	}

	const created = await step(`${owner.name} installs "${spec.name}"`, async () =>
		finished(
			owner,
			await call(owner, localInstalls.createInstance, null, {
				name: spec.name,
				gameVersion: spec.gameVersion,
				loader,
				loaderVersion: spec.loaderVersion ?? null,
				iconPath: null,
			}),
		),
	)
	const made = (await call(owner, instances.ListInstances)).find(
		(entry) => entry.id === created.instance_id,
	)
	if (!made)
		throw new FixtureError(`${owner.name}'s App installed "${spec.name}" but does not list it.`)
	const ref = { id: made.id }
	const dir = NodePath.join(owner.dir, 'profiles', made.path)

	for (const { mod, version } of mods) {
		const before = (await call(owner, instanceContent.ListContentItems, ref)).length
		try {
			await call(owner, instanceContent.AddProjectFromVersion, ref, {
				version_id: version,
				reason: 'standalone',
			})
		} catch (error) {
			throw new FixtureError(
				`The mod ${mod} (Modrinth version ${version}) did not install. If Modrinth no longer has that version, pin another in packages/fixtures/kit/mods.ts. ${error instanceof Error ? error.message : String(error)}`,
			)
		}
		// The App answers before it has listed the file it downloaded, and a publish reads that list.
		await step(`${owner.name} adds the mod ${mod}`, () =>
			until(`"${spec.name}" lists the mod ${mod}`, async () => {
				await call(owner, instanceContent.SyncContentFiles, ref)
				return (await call(owner, instanceContent.ListContentItems, ref)).length > before
			}),
		)
	}
	for (const [path, content] of files) {
		const file = NodePath.join(dir, path)
		NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
		NodeFS.writeFileSync(file, content)
	}
	if (members.length === 0)
		return { kind: 'instance', id: made.id, name: spec.name, owner, dir, share: null }

	// As in the App, the first invite creates the share and publishes its first version.
	const invited = await step(`${owner.name} shares "${spec.name}"`, () =>
		call(owner, instanceSharing.InviteSharedInstanceUsers, ref, {
			user_ids: members.map((member) => member.account.id),
		}),
	)
	const share = await call(owner, instanceSharing.EnsureSharedInstance, ref)
	// That first version leaves loose files out; they go up in a second one, before anyone joins.
	if (files.length > 0) {
		const preview = await call(owner, instanceSharing.GetSharedInstancePublishPreview, ref)
		const publish = NodeCrypto.randomUUID()
		await step(`${owner.name} shares the files of "${spec.name}"`, () =>
			ended(owner, publish, () =>
				call(owner, localSharing.publishSharedInstance, null, {
					op: publish,
					instance_id: made.id,
					file_paths: (preview?.files ?? [])
						.filter((file) => !file.blocked)
						.map((file) => file.path),
					removed_paths: [],
					server_core_id: null,
				}),
			),
		)
	}
	for (const member of members) {
		if (!invited.users.some((user) => user.id === member.account.id)) {
			throw new FixtureError(
				`${owner.name} could not invite ${member.name} to "${spec.name}". An invite needs them to be friends: await friends(${owner.name}, ${member.name})`,
			)
		}
		await call(member, installs.AcceptPendingSharedInstanceInvite, { id: share.id })
		await step(`${member.name} joins "${spec.name}"`, async () =>
			finished(
				member,
				await call(
					member,
					installs.InstallSharedInstance,
					{ id: share.id },
					{
						name: spec.name,
						manager_id: owner.account.id,
						server_manager_name: null,
						server_manager_icon_url: null,
						instance_icon_url: null,
						instance_icon_config: null,
					},
				),
			),
		)
	}
	return { kind: 'instance', id: made.id, name: spec.name, owner, dir, share: share.id }
}

/** Waits for an install job, and fails with the App's own reason when the job does. */
export async function finished(app: App, job: InstallJobSnapshot): Promise<InstallJobSnapshot> {
	return until(
		`${app.name}'s App finishes installing`,
		async () => {
			const now = await call(app, installs.GetInstallJob, { id: job.job_id })
			if (now.status === 'succeeded') return now
			if (now.status === 'queued' || now.status === 'running') return undefined
			throw new FixtureError(
				`${app.name}'s App could not install: ${JSON.stringify(now.error ?? now.status)}`,
			)
		},
		// A first install downloads Minecraft, the loader and Java.
		{ timeoutMs: 15 * 60_000, everyMs: 200 },
	)
}
