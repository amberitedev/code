import { describe, expect, it, vi } from 'vitest'

import {
	findPreferredVersion,
	canManageAmberiteAccess,
	getNextCoreServerPort,
	installVersionDependencies,
	isVersionCompatible,
	normalizeCoreLoader,
	toAmberiteAccessUiMember,
	uiAccessRoleToCore,
} from '../logic'

describe('core logic helpers', () => {
	it('normalizes Core loaders', () => {
		expect(normalizeCoreLoader('purpur')).toBe('paper')
		expect(normalizeCoreLoader('fabric')).toBe('fabric')
		expect(normalizeCoreLoader('unknown')).toBe('vanilla')
	})

	it('selects the next available server port', () => {
		expect(getNextCoreServerPort([25565, 25566])).toBe(25567)
		expect(getNextCoreServerPort([25566])).toBe(25565)
	})
})

describe('access logic helpers', () => {
	it('maps Core access members into shared access-table rows', () => {
		const member = toAmberiteAccessUiMember({
			user_id: 'dev-two',
			display_name: 'Dev Two',
			role: 'admin',
			permission_preset: 'admin',
			status: 'active',
			joined_at: '2026-01-01T00:00:00Z',
			updated_at: '2026-01-01T00:00:00Z',
			source: 'core',
		})

		expect(member.user.username).toBe('Dev Two')
		expect(member.role).toBe('editor')
		expect(member.source).toBe('core')
	})

	it('maps UI roles back to Core roles and checks manage permission', () => {
		expect(uiAccessRoleToCore('editor')).toBe('admin')
		expect(uiAccessRoleToCore('viewer')).toBe('member')
		expect(canManageAmberiteAccess(['server:view'])).toBe(false)
		expect(canManageAmberiteAccess(['members:manage'])).toBe(true)
	})
})

describe('install logic helpers', () => {
	const profile = { path: 'profile-a', game_version: '1.21.1', loader: 'fabric' }

	it('prefers a strict loader match before datapack-compatible fallback', () => {
		const versions = [
			{ id: 'datapack', game_versions: ['1.21.1'], loaders: ['datapack'] },
			{ id: 'fabric', game_versions: ['1.21.1'], loaders: ['fabric'] },
		]

		expect(findPreferredVersion(versions, { project_type: 'mod' }, profile)?.id).toBe('fabric')
		expect(isVersionCompatible(versions[0], { project_type: 'mod' }, profile)).toBe(true)
	})

	it('resolves required dependencies once per project', async () => {
		const installVersion = vi.fn()
		const getProject = vi.fn(async (projectId: string) => ({
			id: projectId,
			title: projectId,
			project_type: 'mod',
			versions: [`${projectId}-version`],
		}))
		const getVersion = vi.fn(async (versionId: string) => ({
			id: versionId,
			project_id: 'dep-a',
			game_versions: ['1.21.1'],
			loaders: ['fabric'],
			dependencies: [],
		}))
		const getVersions = vi.fn(async (versionIds: string[]) =>
			versionIds.map((id) => ({
				id,
				project_id: id.replace('-version', ''),
				game_versions: ['1.21.1'],
				loaders: ['fabric'],
				dependencies: [],
			})),
		)

		await installVersionDependencies({
			profile,
			version: {
				id: 'root',
				project_id: 'root-project',
				game_versions: ['1.21.1'],
				loaders: ['fabric'],
				dependencies: [
					{ dependency_type: 'required', project_id: 'dep-a' },
					{ dependency_type: 'required', version_id: 'dep-a-version', project_id: 'dep-a' },
				],
			},
			reason: 'dependency',
			getProject,
			getVersion,
			getVersions,
			isProjectInstalled: vi.fn(async () => false),
			installVersion,
		})

		expect(installVersion).toHaveBeenCalledTimes(1)
		expect(installVersion).toHaveBeenCalledWith('profile-a', 'dep-a-version', 'dependency')
	})
})
