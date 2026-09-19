import { createHash } from 'node:crypto'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { join } from 'node:path'

import type { SharedInstances } from '../../api-client/src/modules/shared-instances/types.ts'

import { PERSONAS } from './fixtures.ts'
import {
	empty,
	HttpError,
	json,
	objectBody,
	readBody,
	readJson,
	stringArrayField,
	stringField,
} from './http.ts'
import type { StateStore } from './state.ts'
import type { Persona, SharedInstanceState, SharedVersion } from './types.ts'

type SharedContext = {
	request: IncomingMessage
	response: ServerResponse
	url: URL
	persona: Persona | null
	store: StateStore
}

export async function handleSharedInstances(context: SharedContext): Promise<boolean> {
	const { request, response, url, persona, store } = context
	const method = request.method ?? 'GET'
	const path = url.pathname
	if (!isSharedPath(path)) return false

	const inviteInfoMatch = path.match(/^\/v1\/invites\/([^/]+)$/)
	if (method === 'GET' && inviteInfoMatch) {
		const inviteId = decodeURIComponent(inviteInfoMatch[1]!)
		const match = findInvite(store.read().shared_instances, inviteId)
		if (!match) throw new HttpError(404, `Invite ${inviteId} was not found`)
		const owner = personaByUserId(match.instance.owner_id)
		json(response, 200, {
			instance_id: match.instance.id,
			instance_name: match.instance.name,
			instance_icon: match.instance.icon,
			game_version: latestVersion(match.instance)?.game_version ?? 'unknown',
			loader_version: latestVersion(match.instance)?.loader_version ?? '',
			managers: [
				{
					type: 'user',
					id: match.instance.owner_id,
					name: owner?.user.username ?? match.instance.owner_id,
					avatar: owner?.user.avatar_url ?? null,
				},
			],
			instance_users: match.instance.users.map((user) => {
				const fixture = personaByUserId(user.id)
				return {
					id: user.id,
					name: fixture?.user.username ?? user.id,
					avatar: fixture?.user.avatar_url ?? null,
					joined_at: user.joined_at,
				}
			}),
		} satisfies SharedInstances.Invites.v1.Invite)
		return true
	}

	const fileMatch = path.match(/^\/v1\/files\/([^/]+)\/(\d+)\/(\d+)$/)
	if (method === 'GET' && fileMatch) {
		const authenticated = requirePersona(persona)
		const instance = requireInstanceAccess(
			store.read(),
			decodeURIComponent(fileMatch[1]!),
			authenticated,
		)
		const version = requireVersion(instance, Number(fileMatch[2]))
		const fileIndex = Number(fileMatch[3])
		const file = version.external_files[fileIndex]
		if (!file?.uploaded) throw new HttpError(404, 'Shared fixture file is not available')
		const bytes = await readStoredFile(store, instance.id, version.version, fileIndex)
		response.writeHead(200, {
			'cache-control': 'no-store',
			'content-length': bytes.length,
			'content-type': 'application/octet-stream',
		})
		response.end(bytes)
		return true
	}

	const uploadMatch = path.match(/^\/v1\/uploads\/([^/]+)\/(\d+)\/(\d+)$/)
	if (method === 'PUT' && uploadMatch) {
		const authenticated = requirePersona(persona)
		const instanceId = decodeURIComponent(uploadMatch[1]!)
		const versionNumber = Number(uploadMatch[2])
		const fileIndex = Number(uploadMatch[3])
		const state = store.read()
		const instance = requireOwnedInstance(state, instanceId, authenticated)
		const version = requireVersion(instance, versionNumber)
		if (!version.external_files[fileIndex]) throw new HttpError(404, 'Upload slot was not found')
		const bytes = await readBody(request)
		const expectedHash = request.headers['x-file-sha512']
		if (typeof expectedHash === 'string') {
			const actualHash = createHash('sha512').update(bytes).digest('hex')
			if (actualHash !== expectedHash.toLowerCase()) {
				throw new HttpError(422, 'x-file-sha512 does not match the uploaded bytes')
			}
		}
		await writeStoredFile(store, instanceId, versionNumber, fileIndex, bytes)
		await store.mutate((current) => {
			const mutableInstance = requireOwnedInstance(current, instanceId, authenticated)
			const mutableVersion = requireVersion(mutableInstance, versionNumber)
			const file = mutableVersion.external_files[fileIndex]
			if (!file) throw new HttpError(404, 'Upload slot was not found')
			file.file_size = bytes.length
			file.uploaded = true
			mutableVersion.ready = mutableVersion.external_files.every((candidate) => candidate.uploaded)
		})
		empty(response)
		return true
	}

	const authenticated = requirePersona(persona)

	const blacklistMatch = path.match(/^\/v1\/blacklist\/([^/]+)$/)
	if (method === 'GET' && blacklistMatch) {
		json(response, 200, { blacklisted: false } satisfies SharedInstances.Users.v1.BlacklistStatus)
		return true
	}

	if (path === '/v1/instances' && method === 'GET') {
		const userId = url.searchParams.get('user') ?? authenticated.user.id
		json(
			response,
			200,
			store
				.read()
				.shared_instances.filter((instance) => instance.users.some((user) => user.id === userId))
				.map((instance) => instance.id),
		)
		return true
	}

	if (path === '/v1/instances' && method === 'POST') {
		const name = stringField(objectBody(await readJson(request)), 'name')
		const id = await store.mutate((state) => {
			const id = `proxy-lab-shared-${state.shared_instances.length + 1}`
			state.shared_instances.push({
				id,
				name,
				icon: null,
				quarantine: false,
				owner_id: authenticated.user.id,
				users: [fixtureInstanceUser(authenticated, 'owner')],
				tokens: 0,
				versions: [],
				invites: [],
				pending_user_ids: [],
			})
			return id
		})
		json(response, 201, { id })
		return true
	}

	const instanceMatch = path.match(/^\/v1\/instances\/([^/]+)$/)
	if (instanceMatch) {
		const instanceId = decodeURIComponent(instanceMatch[1]!)
		if (method === 'GET') {
			const instance = requireInstanceAccess(store.read(), instanceId, authenticated)
			json(response, 200, {
				name: instance.name,
				icon: instance.icon,
				quarantine: instance.quarantine,
			} satisfies SharedInstances.Instances.v1.Instance)
			return true
		}
		if (method === 'PATCH') {
			const name = stringField(objectBody(await readJson(request)), 'name')
			await store.mutate((state) => {
				requireOwnedInstance(state, instanceId, authenticated).name = name
			})
			empty(response)
			return true
		}
		if (method === 'DELETE') {
			await store.mutate((state) => {
				requireOwnedInstance(state, instanceId, authenticated)
				state.shared_instances = state.shared_instances.filter((item) => item.id !== instanceId)
			})
			await rm(join(store.dataDir, 'shared-files', instanceId), { recursive: true, force: true })
			empty(response)
			return true
		}
	}

	const iconMatch = path.match(/^\/v1\/instances\/([^/]+)\/icon$/)
	if (iconMatch && (method === 'PUT' || method === 'DELETE')) {
		const instanceId = decodeURIComponent(iconMatch[1]!)
		if (method === 'PUT') await readBody(request)
		await store.mutate((state) => {
			const instance = requireOwnedInstance(state, instanceId, authenticated)
			instance.icon = method === 'PUT' ? `${url.origin}/v1/instances/${instanceId}/icon` : null
		})
		empty(response)
		return true
	}

	if (await handleUsersAndInvites(context, authenticated)) return true
	if (await handleVersions(context, authenticated)) return true

	return false
}

async function handleUsersAndInvites(context: SharedContext, persona: Persona): Promise<boolean> {
	const { request, response, url, store } = context
	const method = request.method ?? 'GET'
	const match = url.pathname.match(/^\/v1\/instances\/([^/]+)\/(users|invites)(.*)$/)
	if (!match) return false
	const instanceId = decodeURIComponent(match[1]!)
	const collection = match[2]!
	const suffix = match[3]!
	const state = store.read()

	if (collection === 'users' && suffix === '' && method === 'GET') {
		const instance = requireInstanceAccess(state, instanceId, persona)
		json(response, 200, { users: instance.users, tokens: instance.tokens })
		return true
	}
	if (collection === 'users' && suffix === '' && (method === 'POST' || method === 'DELETE')) {
		const userIds = stringArrayField(objectBody(await readJson(request)), 'user_ids')
		await store.mutate((current) => {
			const instance = requireOwnedInstance(current, instanceId, persona)
			if (method === 'POST') {
				for (const userId of userIds) {
					if (!instance.users.some((user) => user.id === userId)) {
						const fixture = personaByUserId(userId)
						instance.users.push(
							fixture
								? fixtureInstanceUser(fixture, 'invite')
								: {
										id: userId,
										joined_at: new Date().toISOString(),
										join_type: 'invite',
										last_played: null,
									},
						)
					}
				}
			} else {
				instance.users = instance.users.filter(
					(user) => !userIds.includes(user.id) || user.id === instance.owner_id,
				)
			}
		})
		empty(response)
		return true
	}

	if (collection !== 'invites') return false
	if (suffix === '' && method === 'GET') {
		json(response, 200, requireOwnedInstance(state, instanceId, persona).invites)
		return true
	}
	if (suffix === '' && method === 'POST') {
		const body = objectBody(await readJson(request))
		const maxAge = typeof body.max_age === 'number' ? body.max_age : 604_800
		const maxUses = typeof body.max_uses === 'number' ? body.max_uses : 10
		const invite = await store.mutate((current) => {
			const instance = requireOwnedInstance(current, instanceId, persona)
			const invite = {
				id: `proxy-lab-invite-${current.next_invite++}`,
				expiration: new Date(Date.now() + maxAge * 1000).toISOString(),
				max_uses: maxUses,
				uses: 0,
			}
			instance.invites.push(invite)
			instance.tokens = instance.invites.length
			return invite
		})
		json(response, 201, { id: invite.id })
		return true
	}
	if (suffix === '/pending' && method === 'POST') {
		await store.mutate((current) => {
			const instance = requireInstance(current, instanceId)
			if (!instance.pending_user_ids.includes(persona.user.id)) {
				throw new HttpError(404, 'No pending fixture invite was found')
			}
			instance.pending_user_ids = instance.pending_user_ids.filter((id) => id !== persona.user.id)
			instance.users.push(fixtureInstanceUser(persona, 'invite'))
		})
		empty(response)
		return true
	}
	if (suffix === '/pending' && method === 'DELETE') {
		await store.mutate((current) => {
			const instance = requireInstance(current, instanceId)
			instance.pending_user_ids = instance.pending_user_ids.filter((id) => id !== persona.user.id)
		})
		empty(response)
		return true
	}

	const inviteId = decodeURIComponent(suffix.replace(/^\//, ''))
	if (!inviteId) return false
	if (method === 'POST') {
		await store.mutate((current) => {
			const instance = requireInstance(current, instanceId)
			const invite = instance.invites.find((item) => item.id === inviteId)
			if (!invite || invite.uses >= invite.max_uses)
				throw new HttpError(404, 'Invite is not available')
			if (!instance.users.some((user) => user.id === persona.user.id)) {
				instance.users.push(fixtureInstanceUser(persona, 'link'))
			}
			invite.uses++
		})
		empty(response)
		return true
	}
	if (method === 'DELETE') {
		await store.mutate((current) => {
			const instance = requireOwnedInstance(current, instanceId, persona)
			instance.invites = instance.invites.filter((item) => item.id !== inviteId)
			instance.tokens = instance.invites.length
		})
		empty(response)
		return true
	}
	return false
}

async function handleVersions(context: SharedContext, persona: Persona): Promise<boolean> {
	const { request, response, url, store } = context
	const method = request.method ?? 'GET'
	const match = url.pathname.match(/^\/v1\/instances\/([^/]+)\/versions(?:\/(\d+))?$/)
	if (!match) return false
	const instanceId = decodeURIComponent(match[1]!)
	const state = store.read()
	const instance = requireInstanceAccess(state, instanceId, persona)

	if (method === 'GET') {
		const version = match[2] ? requireVersion(instance, Number(match[2])) : latestVersion(instance)
		if (!version) throw new HttpError(404, `Shared instance ${instanceId} has no published version`)
		json(response, 200, publicVersion(url.origin, instance, version))
		return true
	}
	if (method !== 'POST' || match[2]) return false

	const owned = requireOwnedInstance(state, instanceId, persona)
	const body = objectBody(await readJson(request))
	const externalFiles = Array.isArray(body.external_files)
		? body.external_files.map(objectBody)
		: []
	const version = await store.mutate((current) => {
		const mutableInstance = requireOwnedInstance(current, owned.id, persona)
		const versionNumber = current.next_shared_version++
		const created: SharedVersion = {
			version: versionNumber,
			modrinth_ids: Array.isArray(body.modrinth_ids)
				? body.modrinth_ids.filter((id): id is string => typeof id === 'string')
				: [],
			ready: externalFiles.length === 0,
			external_files: externalFiles.map((file, index) => ({
				file_name: stringField(file, 'file_name'),
				file_type: stringField(file, 'file_type'),
				url: `${url.origin}/v1/uploads/${encodeURIComponent(instanceId)}/${versionNumber}/${index}`,
				file_size: 0,
				uploaded: false,
			})),
			modpack_id: typeof body.modpack_id === 'string' ? body.modpack_id : null,
			game_version: stringField(body, 'game_version'),
			loader: stringField(body, 'loader'),
			loader_version: stringField(body, 'loader_version'),
		}
		mutableInstance.versions.push(created)
		return created
	})
	json(response, 201, uploadVersion(version))
	return true
}

function isSharedPath(path: string): boolean {
	return /^\/v1\/(instances|invites|blacklist|uploads|files|moderation)(\/|$)/.test(path)
}

function requirePersona(persona: Persona | null): Persona {
	if (!persona) throw new HttpError(401, 'Use a proxy-lab fixture token for shared-instance routes')
	return persona
}

function requireInstance(
	state: ReturnType<StateStore['read']>,
	instanceId: string,
): SharedInstanceState {
	const instance = state.shared_instances.find((item) => item.id === instanceId)
	if (!instance) throw new HttpError(404, `Shared instance ${instanceId} was not found`)
	return instance
}

function requireInstanceAccess(
	state: ReturnType<StateStore['read']>,
	instanceId: string,
	persona: Persona,
): SharedInstanceState {
	const instance = requireInstance(state, instanceId)
	if (!instance.users.some((user) => user.id === persona.user.id)) {
		throw new HttpError(401, `Persona ${persona.id} cannot access shared instance ${instanceId}`)
	}
	return instance
}

function requireOwnedInstance(
	state: ReturnType<StateStore['read']>,
	instanceId: string,
	persona: Persona,
): SharedInstanceState {
	const instance = requireInstance(state, instanceId)
	if (instance.owner_id !== persona.user.id) {
		throw new HttpError(403, `Persona ${persona.id} does not own shared instance ${instanceId}`)
	}
	return instance
}

function latestVersion(instance: SharedInstanceState): SharedVersion | undefined {
	return instance.versions.at(-1)
}

function requireVersion(instance: SharedInstanceState, version: number): SharedVersion {
	const found = instance.versions.find((item) => item.version === version)
	if (!found) throw new HttpError(404, `Shared version ${version} was not found`)
	return found
}

function publicVersion(origin: string, instance: SharedInstanceState, version: SharedVersion) {
	return {
		...version,
		external_files: version.external_files.map(({ uploaded: _uploaded, ...file }, index) => ({
			...file,
			url: `${origin}/v1/files/${encodeURIComponent(instance.id)}/${version.version}/${index}`,
		})),
	}
}

function uploadVersion(version: SharedVersion) {
	return {
		...version,
		external_files: version.external_files.map(({ uploaded: _uploaded, ...file }) => file),
	}
}

function findInvite(instances: SharedInstanceState[], inviteId: string) {
	for (const instance of instances) {
		const invite = instance.invites.find((item) => item.id === inviteId)
		if (invite) return { instance, invite }
	}
	return undefined
}

function personaByUserId(userId: string): Persona | undefined {
	return Object.values(PERSONAS).find((persona) => persona.user.id === userId)
}

function fixtureInstanceUser(
	persona: Persona,
	joinType: SharedInstances.Instances.v1.JoinType,
): SharedInstances.Instances.v1.InstanceUser {
	return {
		id: persona.user.id,
		joined_at: new Date().toISOString(),
		join_type: joinType,
		last_played: null,
	}
}

async function writeStoredFile(
	store: StateStore,
	instanceId: string,
	version: number,
	index: number,
	bytes: Buffer,
): Promise<void> {
	const directory = join(store.dataDir, 'shared-files', instanceId, String(version))
	await mkdir(directory, { recursive: true })
	await writeFile(join(directory, String(index)), bytes)
}

async function readStoredFile(
	store: StateStore,
	instanceId: string,
	version: number,
	index: number,
): Promise<Buffer> {
	try {
		return await readFile(
			join(store.dataDir, 'shared-files', instanceId, String(version), String(index)),
		)
	} catch (error) {
		if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
			if (instanceId === 'proxy-lab-shared-instance' && version === 1 && index === 0) {
				return Buffer.from('UEsFBgAAAAAAAAAAAAAAAAAAAAAAAA==', 'base64')
			}
			throw new HttpError(404, 'Shared fixture file bytes were not found')
		}
		throw error
	}
}
