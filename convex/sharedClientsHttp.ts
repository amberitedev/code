import { httpAction } from './_generated/server'
import { internal } from './_generated/api'
import type { Id } from './_generated/dataModel'

type JsonRecord = Record<string, unknown>

export const handleGet = httpAction(async (ctx, request) => {
	try {
		const route = routeParts(request)
		if (route[0] === 'blacklist' && route[1]) return json({ blacklisted: false })
		if (route[0] === 'invites' && route[1]) {
			return json(
				await ctx.runQuery(internal.sharedClients.httpInviteInfo, {
					inviteId: route[1] as Id<'sharedClientInvites'>,
					now: Date.now(),
				}),
			)
		}
		if (route[0] !== 'instances') return notFound()
		if (!route[1]) return json(await ctx.runQuery(internal.sharedClients.httpListMine, {}))
		const clientId = route[1] as Id<'sharedClients'>
		if (route.length === 2)
			return json(await ctx.runQuery(internal.sharedClients.httpGet, { clientId }))
		if (route[2] === 'users')
			return json(
				await ctx.runQuery(internal.sharedClients.httpUsers, { clientId, now: Date.now() }),
			)
		if (route[2] === 'invites' && route.length === 3)
			return json(
				await ctx.runQuery(internal.sharedClients.httpListInvites, {
					clientId,
					now: Date.now(),
				}),
			)
		if (route[2] === 'versions' && !route[3])
			return json(await ctx.runQuery(internal.sharedClients.httpLatestVersion, { clientId }))
		if (route[2] === 'versions' && route[3]) {
			const version = Number(route[3])
			if (!Number.isInteger(version) || version < 1) return badRequest('invalid version')
			return json(await ctx.runQuery(internal.sharedClients.httpVersion, { clientId, version }))
		}
		return notFound()
	} catch (error) {
		return errorResponse(error)
	}
})

export const handlePost = httpAction(async (ctx, request) => {
	try {
		const route = routeParts(request)
		const body = await jsonBody(request)
		if (route[0] !== 'instances') return notFound()
		if (!route[1])
			return json(
				await ctx.runMutation(internal.sharedClients.httpCreate, {
					name: stringField(body, 'name'),
				}),
			)
		const clientId = route[1] as Id<'sharedClients'>
		if (route[2] === 'users') {
			await ctx.runMutation(internal.sharedClients.httpInviteUsers, {
				clientId,
				userIds: idArray(body, 'user_ids'),
			})
			return empty()
		}
		if (route[2] === 'invites' && route[3] === 'pending') {
			const accepted = await ctx.runMutation(internal.sharedClients.httpAcceptPending, { clientId })
			return accepted ? empty() : notFound()
		}
		if (route[2] === 'invites' && route[3]) {
			await ctx.runMutation(internal.sharedClients.httpAcceptInvite, {
				clientId,
				inviteId: route[3] as Id<'sharedClientInvites'>,
			})
			return empty()
		}
		if (route[2] === 'invites') {
			return json(
				await ctx.runMutation(internal.sharedClients.httpCreateInvite, {
					clientId,
					maxAgeSeconds: optionalNumberField(body, 'max_age') ?? 7 * 24 * 60 * 60,
					maxUses: optionalNumberField(body, 'max_uses') ?? 20,
				}),
			)
		}
		if (route[2] === 'versions') {
			const externalFiles = arrayField(body, 'external_files').map((entry) => {
				const file = record(entry)
				return {
					fileName: stringField(file, 'file_name'),
					fileType: stringField(file, 'file_type'),
				}
			})
			return json(
				await ctx.runMutation(internal.sharedClients.httpCreateVersion, {
					clientId,
					modrinthIds: stringArray(body, 'modrinth_ids'),
					externalFiles,
					...(optionalStringField(body, 'modpack_id')
						? { modpackId: optionalStringField(body, 'modpack_id')! }
						: {}),
					gameVersion: stringField(body, 'game_version'),
					loader: stringField(body, 'loader'),
					loaderVersion: optionalStringField(body, 'loader_version') ?? '',
					origin: new URL(request.url).origin,
				}),
			)
		}
		return notFound()
	} catch (error) {
		return errorResponse(error)
	}
})

export const handlePatch = httpAction(async (ctx, request) => {
	try {
		const route = routeParts(request)
		if (route[0] !== 'instances' || !route[1] || route.length !== 2) return notFound()
		await ctx.runMutation(internal.sharedClients.httpUpdate, {
			clientId: route[1] as Id<'sharedClients'>,
			name: stringField(await jsonBody(request), 'name'),
		})
		return empty()
	} catch (error) {
		return errorResponse(error)
	}
})

export const handleDelete = httpAction(async (ctx, request) => {
	try {
		const route = routeParts(request)
		if (route[0] !== 'instances' || !route[1]) return notFound()
		const clientId = route[1] as Id<'sharedClients'>
		if (route.length === 2) {
			await ctx.runMutation(internal.sharedClients.httpDelete, { clientId })
			return empty()
		}
		if (route[2] === 'icon') {
			await ctx.runMutation(internal.sharedClients.setIcon, { clientId, storageId: null })
			return empty()
		}
		if (route[2] === 'users') {
			await ctx.runMutation(internal.sharedClients.httpRemoveUsers, {
				clientId,
				userIds: idArray(await jsonBody(request), 'user_ids'),
			})
			return empty()
		}
		if (route[2] === 'invites' && route[3] === 'pending') {
			await ctx.runMutation(internal.sharedClients.httpDeclinePending, { clientId })
			return empty()
		}
		if (route[2] === 'invites' && route[3]) {
			await ctx.runMutation(internal.sharedClients.httpRevokeInvite, {
				clientId,
				inviteId: route[3] as Id<'sharedClientInvites'>,
			})
			return empty()
		}
		return notFound()
	} catch (error) {
		return errorResponse(error)
	}
})

export const handlePut = httpAction(async (ctx, request) => {
	try {
		const route = routeParts(request)
		if (route[0] === 'uploads' && route[1]) {
			const blob = await request.blob()
			const storageId = await ctx.storage.store(blob)
			const attached = await ctx.runMutation(internal.sharedClients.attachExternalFile, {
				uploadToken: route[1],
				storageId,
				size: blob.size,
			})
			if (!attached) {
				await ctx.storage.delete(storageId)
				return notFound()
			}
			return empty()
		}
		if (route[0] === 'instances' && route[1] && route[2] === 'icon') {
			const blob = await request.blob()
			if (blob.size > 2 * 1_024 * 1_024) return badRequest('icon is too large')
			const storageId = await ctx.storage.store(blob)
			try {
				await ctx.runMutation(internal.sharedClients.setIcon, {
					clientId: route[1] as Id<'sharedClients'>,
					storageId,
				})
			} catch (error) {
				await ctx.storage.delete(storageId)
				throw error
			}
			return empty()
		}
		return notFound()
	} catch (error) {
		return errorResponse(error)
	}
})

function routeParts(request: Request) {
	const path = new URL(request.url).pathname.replace(/^\/v1\/?/, '')
	return path.split('/').filter(Boolean).map(decodeURIComponent)
}

async function jsonBody(request: Request): Promise<JsonRecord> {
	if (!request.headers.get('content-length') && !request.headers.get('transfer-encoding')) return {}
	return record(await request.json())
}

function record(value: unknown): JsonRecord {
	if (!value || typeof value !== 'object' || Array.isArray(value))
		throw new Error('invalid JSON body')
	return value as JsonRecord
}

function stringField(value: JsonRecord, key: string) {
	const field = value[key]
	if (typeof field !== 'string') throw new Error(`${key} must be a string`)
	return field
}

function optionalStringField(value: JsonRecord, key: string) {
	const field = value[key]
	if (field === undefined || field === null) return undefined
	if (typeof field !== 'string') throw new Error(`${key} must be a string`)
	return field
}

function optionalNumberField(value: JsonRecord, key: string) {
	const field = value[key]
	if (field === undefined || field === null) return undefined
	if (typeof field !== 'number' || !Number.isFinite(field))
		throw new Error(`${key} must be a number`)
	return field
}

function arrayField(value: JsonRecord, key: string) {
	const field = value[key]
	if (field === undefined) return []
	if (!Array.isArray(field)) throw new Error(`${key} must be an array`)
	return field
}

function stringArray(value: JsonRecord, key: string) {
	const values = arrayField(value, key)
	if (!values.every((entry): entry is string => typeof entry === 'string'))
		throw new Error(`${key} must contain strings`)
	return values
}

function idArray(value: JsonRecord, key: string) {
	return stringArray(value, key) as Id<'users'>[]
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { 'content-type': 'application/json' },
	})
}

function empty() {
	return new Response(null, { status: 204 })
}

function notFound() {
	return json({ error: 'not found' }, 404)
}

function badRequest(message: string) {
	return json({ error: message }, 400)
}

function errorResponse(error: unknown) {
	const message = error instanceof Error ? error.message : 'request failed'
	if (message.includes('not authenticated') || message.includes('not authorized'))
		return json({ error: message }, 401)
	if (message.includes('not found')) return notFound()
	return badRequest(message)
}
