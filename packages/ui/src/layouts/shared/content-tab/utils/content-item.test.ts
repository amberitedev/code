import type { ContentItem as EngineContentItem, ServerSides } from '@amberite/contracts/engine'
import { describe, expect, test } from 'vitest'

import { toContentItem } from './content-item'

function engineItem(overrides: Partial<EngineContentItem> = {}): EngineContentItem {
	return {
		file_name: 'sodium.jar',
		file_path: 'mods/sodium.jar',
		id: 'sha1-hash',
		size: 10,
		enabled: true,
		locked: false,
		project_type: 'mod',
		project: null,
		version: null,
		environment: null,
		owner: null,
		has_update: false,
		update_version_id: null,
		date_added: null,
		source_kind: null,
		embedded_metadata: null,
		...overrides,
	}
}

const placed: ServerSides = {
	disabled_server: false,
	disabled_player: false,
	sides_unlocked: false,
	server_override: false,
	installed: true,
}

describe('toContentItem', () => {
	test('a file on Modrinth keeps its project, version and owner, by its path', () => {
		const item = toContentItem(
			engineItem({
				project: {
					id: 'AANobbMI',
					slug: null,
					title: 'Sodium',
					icon_url: null,
					license: { id: 'LGPL-3.0-only', name: 'LGPL', url: null },
					categories: ['optimization'],
					additional_categories: [],
				},
				version: { id: 'v1', version_number: '0.6', file_name: 'sodium.jar', date_published: null },
				owner: { id: 'u1', name: 'jellysquid', avatar_url: null, type: 'user' },
				date_added: null,
			}),
		)
		expect(item).toMatchObject({
			id: 'mods/sodium.jar',
			external: false,
			project: { id: 'AANobbMI', slug: 'AANobbMI', title: 'Sodium', icon_url: undefined },
			version: { id: 'v1', date_published: undefined },
			owner: { avatar_url: undefined },
			date_added: undefined,
		})
		expect(item.project?.license?.url).toBeUndefined()
	})

	test('an unmatched file has no project; its embedded icon loads only where the host can', () => {
		const embedded = { name: 'My Mod', version: '1.0', icon_path: 'cache/icon.png' }
		const local = toContentItem(engineItem({ embedded_metadata: embedded }), {
			iconUrl: (path) => `asset://${path}`,
		})
		expect(local).toMatchObject({ external: true, project: undefined })
		expect(local.embedded_metadata?.icon_url).toBe('asset://cache/icon.png')
		expect(local.enabledFor).toBeUndefined()

		const core = toContentItem(engineItem({ embedded_metadata: embedded }))
		expect(core.embedded_metadata?.icon_url).toBeUndefined()
	})

	test.each([
		{
			name: 'a resource pack runs for players only',
			item: { project_type: 'resourcepack' as const, environment: 'client_only' as const },
			enabledFor: { server: false, player: true, locked: true, disabledSides: ['server'] },
			lockedTooltip: 'Resource packs can only be enabled for players.',
			warningKind: undefined,
		},
		{
			name: 'a client-only mod enabled on the server warns',
			item: { environment: 'client_only' as const },
			enabledFor: { server: true, player: true, locked: false },
			warningKind: 'environment',
		},
		{
			name: 'a file with no detected environment warns',
			item: {},
			enabledFor: { server: true, player: true, locked: false },
			warningKind: 'unknown-environment',
		},
	])('on a server, $name', ({ item, enabledFor, lockedTooltip, warningKind }) => {
		const mapped = toContentItem(engineItem({ ...item, server: placed }))
		expect(mapped.enabledFor).toMatchObject(enabledFor)
		expect(mapped.enabledFor?.lockedTooltip).toBe(lockedTooltip)
		expect(mapped.enabledFor?.warningKind).toBe(warningKind)
	})

	test("a pack's file takes the pack's sides without an unknown-environment warning", () => {
		const mapped = toContentItem(engineItem({ server: placed }), {}, true)
		expect(mapped.enabledFor?.warningKind).toBeUndefined()
	})

	test('a client-only pack file the Core never placed is off on the server, on for players', () => {
		const mapped = toContentItem(
			engineItem({
				environment: 'client_only',
				server: { ...placed, installed: false, disabled_server: true },
			}),
			{ formatMessage: (descriptor) => descriptor.id },
			true,
		)
		expect(mapped.enabledFor).toMatchObject({
			server: false,
			player: true,
			locked: true,
			disabledSides: ['player'],
			lockedTooltip: 'hosting.content.enabled-for.client-only-not-installed-lock',
		})
		expect(mapped.version?.version_number).toBe(
			'hosting.content.instance.not-installed-client-only',
		)
	})

	test('a server file off for both sides shows as disabled', () => {
		const mapped = toContentItem(
			engineItem({
				environment: 'client_and_server',
				server: { ...placed, disabled_server: true, disabled_player: true },
			}),
		)
		expect(mapped.enabled).toBe(false)
	})
})
