import { type ContentItem, type Endpoint, instanceContent } from '@amberite/contracts'
import { describe, expect, it } from 'vite-plus/test'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { content } from './content'

const SERVER = 'server'

function item(fileName: string, filePath: string, enabled: boolean): ContentItem {
	return {
		file_name: fileName,
		file_path: filePath,
		id: filePath,
		size: 1,
		enabled,
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
	}
}

/** A client whose Core answers the engine's content lists with `own` and `pack`. */
function fake(own: ContentItem[], pack: ContentItem[]) {
	const calls: { endpoint: Endpoint; input: unknown }[] = []
	const answers = new Map<Endpoint, unknown>([
		[instanceContent.ListContentItems, own],
		[instanceContent.ListLinkedModpackContent, pack],
	])
	const client = {
		archon: { cores_v1: { coreOf: async () => ({ id: 'core' }) } },
		core: () => ({
			call: async (endpoint: Endpoint, input: unknown) => {
				calls.push({ endpoint, input })
				return answers.get(endpoint)
			},
		}),
	} as unknown as AbstractModrinthClient
	return { adapter: content(client), calls }
}

describe('content on a Core', () => {
	it('changes the addon the panel names by kind and file name at the engine path', async () => {
		const { adapter, calls } = fake([item('a.jar', 'mods/a.jar.disabled', false)], [])
		await adapter.enableAddon!(SERVER, SERVER, { kind: 'mod', filename: 'a.jar' })
		expect(calls.at(-1)).toEqual({
			endpoint: instanceContent.ToggleProject,
			input: {
				params: { id: SERVER },
				body: { project_path: 'mods/a.jar.disabled', desired_enabled: true },
			},
		})
		await expect(
			adapter.enableAddon!(SERVER, SERVER, { kind: 'plugin', filename: 'a.jar' }),
		).rejects.toMatchObject({ code: 'not_found' })
	})
})
