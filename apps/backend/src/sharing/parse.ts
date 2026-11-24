import {
	type FollowedShare,
	MAX_VERSION_FILES,
	type NewInstanceVersion,
	type NewInviteLink,
	type NewServerShare,
	type NewSharedInstance,
	type OutsideAddress,
	REMOVED_FILE_FOLDERS,
	REMOVED_FILE_SUFFIXES,
	type RemovedFile,
	type SharedIconConfig,
	type SharedInstanceUpdate,
	type UserIds,
	type VersionFault,
} from '@amberite/contracts'

import { ApiError } from '../common/http'
import { externalFile } from './pool'

// The sharing routes' bodies, checked into their contract types. A body that does not fit throws
// `invalid_input`.

export function string(value: unknown, field: string, max = 256): string {
	if (typeof value !== 'string' || value.length > max || !value.trim())
		throw new ApiError(400, 'invalid_input', `Invalid ${field}`)
	return value
}

/**
 * A shared file's name is its path inside the instance, such as `config/foo.toml`, or a bare file
 * name for content files. Slashes separate; nothing may escape the instance folder.
 */
function fileName(value: unknown): string {
	const name = string(value, 'file_name', 1024)
	if (
		/[\\\u0000-\u001f]/.test(name) ||
		name.split('/').some((part) => part === '' || part === '.' || part === '..')
	)
		throw new ApiError(400, 'invalid_input', 'Invalid file path')
	return name
}

function fileType(value: unknown): string {
	const type = string(value, 'file_type', 64)
	if (!/^[a-z0-9_-]+$/.test(type)) throw new ApiError(400, 'invalid_input', 'Invalid file_type')
	return type
}

/** Windows device names, as `is_reserved_windows_device_name` in `packages/path-util`. */
const RESERVED_DEVICE_NAMES = [
	...['CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'],
	...['COM', 'LPT'].flatMap((name) => [...'123456789¹²³'].map((digit) => name + digit)),
]

/**
 * Whether `name` is one plain file name, as `path_util::is_safe_file_name` decides: no
 * separator of either kind, no drive prefix, not `.` or `..`, and no Windows device name
 * (`CON`, `con.jar`, `NUL:x`).
 */
function safeFileName(name: string): boolean {
	if (!name || name === '.' || name === '..' || /[/\\]/.test(name) || /^[a-z]:/i.test(name))
		return false
	const upper = name.replace(/[a-z]/g, (letter) => letter.toUpperCase())
	return !RESERVED_DEVICE_NAMES.some(
		(device) => upper.startsWith(device) && /^($|[.:])/.test(upper.slice(device.length)),
	)
}

/** `removed_files` by the App's own rules (`SharedInstanceRemovedFile::validate` in app-lib). */
function removedFiles(value: unknown, modpackId: string | null): RemovedFile[] {
	if (value === undefined || value === null) return []
	if (!Array.isArray(value) || value.length > MAX_VERSION_FILES)
		throw new ApiError(400, 'invalid_input', 'Invalid removed_files')
	if (value.length && !modpackId)
		throw new ApiError(400, 'invalid_input', 'Removed modpack files require a modpack_id')
	return value.map((item): RemovedFile => {
		const entry = object(item)
		if (entry.type === 'version')
			return { type: 'version', version_id: string(entry.version_id, 'version_id') }
		const filename = typeof entry.filename === 'string' ? entry.filename : ''
		if (
			entry.type !== 'path' ||
			typeof entry.parent !== 'string' ||
			!REMOVED_FILE_FOLDERS.includes(entry.parent) ||
			/[\u0000-\u001f]/.test(filename) ||
			!safeFileName(filename) ||
			!REMOVED_FILE_SUFFIXES.some((suffix) => filename.endsWith(suffix))
		)
			throw new ApiError(400, 'invalid_input', 'Invalid removed modpack file')
		return { type: 'path', parent: entry.parent, filename }
	})
}

export function strings(value: unknown, field: string, max = 250): string[] {
	if (!Array.isArray(value) || value.length > max)
		throw new ApiError(400, 'invalid_input', `Invalid ${field}`)
	return [...new Set(value.map((item) => string(item, field)))]
}

export function object(value: unknown): Record<string, unknown> {
	if (!value || typeof value !== 'object' || Array.isArray(value))
		throw new ApiError(400, 'invalid_input', 'Expected an object')
	return value as Record<string, unknown>
}

function natural(value: unknown, fallback: number, max: number): number {
	if (value === undefined) return fallback
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > max)
		throw new ApiError(400, 'invalid_input', 'Invalid invite limit')
	return value
}

/** A push's body, checked as far as it can be without hashing. */
export function parseNewVersion(raw: unknown): NewInstanceVersion {
	const body = object(raw)
	const loader = string(body.loader, 'loader')
	if (!['vanilla', 'fabric', 'forge', 'quilt', 'neoforge'].includes(loader))
		throw new ApiError(400, 'invalid_input', 'Invalid loader')
	if (!Array.isArray(body.external_files))
		throw new ApiError(400, 'invalid_input', 'Invalid external_files')
	if (body.external_files.length > MAX_VERSION_FILES)
		throw new ApiError(
			413,
			'too_many_files',
			`A shared instance can hold at most ${MAX_VERSION_FILES} files`,
		)
	const seen = new Set<string>()
	const external = body.external_files.map((value) => {
		const entry = object(value)
		const file_name = fileName(entry.file_name)
		const file_type = fileType(entry.file_type)
		const name = `${file_type}:${file_name}`
		if (seen.has(name)) throw new ApiError(400, 'invalid_input', 'Duplicate external file')
		seen.add(name)
		return externalFile(entry, file_name, file_type)
	})
	const modpackId =
		body.modpack_id === undefined || body.modpack_id === null
			? null
			: string(body.modpack_id, 'modpack_id')
	return {
		modrinth_ids: strings(body.modrinth_ids ?? [], 'modrinth_ids', 1000),
		modpack_id: modpackId,
		removed_files: removedFiles(body.removed_files, modpackId),
		game_version: string(body.game_version, 'game_version'),
		loader,
		loader_version: typeof body.loader_version === 'string' ? body.loader_version : '',
		version_hash: typeof body.version_hash === 'string' ? body.version_hash : '',
		external_files: external,
		server_core:
			body.server_core === undefined || body.server_core === null
				? undefined
				: string(body.server_core, 'server_core', 64),
	}
}

export function parseFault(raw: unknown): VersionFault {
	const body = object(raw)
	const kind = string(body.kind, 'kind', 16)
	if (kind !== 'object' && kind !== 'content')
		throw new ApiError(400, 'invalid_input', 'Invalid kind')
	return { core_id: string(body.core_id, 'core_id', 64), kind }
}

/** Seconds a link lives, a week by default; 0 is a year. Uses, 0 for no limit. */
export function parseInviteLink(body: Record<string, unknown>): NewInviteLink {
	return {
		max_age: natural(body.max_age, 604800, 31536000),
		max_uses: natural(body.max_uses, 0, 10000),
	}
}

export function parseUserIds(raw: unknown): UserIds {
	return { user_ids: strings(object(raw).user_ids, 'user_ids', 100) }
}

/** The server a players write names: the share it follows, its name, its game port, and `net.outside`. */
export function parseFollowedShare(body: Record<string, unknown>): FollowedShare {
	return {
		instance_id: string(body.instance_id, 'instance_id', 64),
		name: string(body.name, 'name'),
		port: gamePort(body.port),
		outside: outsideAddress(body.outside),
	}
}

function gamePort(value: unknown): number {
	if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 65535)
		throw new ApiError(400, 'invalid_input', 'Invalid port')
	return value
}

/** `net.outside`; null while the Core has none. */
function outsideAddress(value: unknown): OutsideAddress | null {
	if (value === undefined || value === null) return null
	const outside = object(value)
	const ip = string(outside.ip, 'outside', 45)
	if (!/^[\da-f.:]+$/i.test(ip)) throw new ApiError(400, 'invalid_input', 'Invalid outside')
	return { ip, port: gamePort(outside.port) }
}

export const parseUpdate = (raw: unknown): SharedInstanceUpdate => ({
	name: string(object(raw).name, 'name'),
})

export const parseNewShare = (raw: unknown): NewSharedInstance => ({
	name: string(object(raw).name, 'name'),
})

export const parseNewServerShare = (raw: unknown): NewServerShare => {
	const body = object(raw)
	return { name: string(body.name, 'name'), server_id: string(body.server_id, 'server_id', 64) }
}

const ICON_COLOR = /^#[0-9a-fA-F]{6}$/
const ICON_SYMBOL = /^[a-z0-9_]{1,64}$/

/** The icon editor settings, checked by the rules of `modrinth-content-management`'s
 * `IconConfig::validate`. */
export function validIconConfig(value: unknown): SharedIconConfig {
	const invalid = () => new ApiError(400, 'invalid_input', 'Invalid icon config')
	if (typeof value !== 'object' || value === null) throw invalid()
	const { background, symbol } = value as { background?: unknown; symbol?: unknown }
	if (typeof symbol !== 'string' || !ICON_SYMBOL.test(symbol)) throw invalid()
	if (typeof background !== 'object' || background === null) throw invalid()
	const color = (field: unknown): field is string =>
		typeof field === 'string' && ICON_COLOR.test(field)
	const bg = background as Record<string, unknown>
	if (bg.type === 'color' && color(bg.value))
		return { background: { type: 'color', value: bg.value }, symbol }
	if (bg.type === 'linear-top-down-gradient' && color(bg.top_color) && color(bg.bottom_color))
		return {
			background: { type: bg.type, top_color: bg.top_color, bottom_color: bg.bottom_color },
			symbol,
		}
	throw invalid()
}
