import { describe, expect, it, vi } from 'vitest'

import {
	findPreferredVersion,
	canManageAmberiteAccess,
	getNextCoreServerPort,
	installSyncedProfileFromCore,
	installVersionDependencies,
	isVersionCompatible,
	normalizeCoreLoader,
	resolveInstallableSyncedProfiles,
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

describe('synced install logic helpers', () => {
	it('resolves friend-group synced profiles that can become local app instances', () => {
		const profiles = resolveInstallableSyncedProfiles({
			socialProfiles: [
				{
					_id: 'social-a',
					coreInstanceId: 'core-a',
					name: 'Shared Client',
					gameVersion: '1.21.1',
					loader: 'fabric',
					syncEnabled: true,
				},
				{
					_id: 'server-only',
					coreInstanceId: 'core-server',
					name: 'Server Only',
					gameVersion: '1.21.1',
					loader: 'paper',
					syncEnabled: true,
				},
				{
					_id: 'installed',
					coreInstanceId: 'core-installed',
					name: 'Installed',
					gameVersion: '1.21.1',
					loader: 'fabric',
					syncEnabled: true,
				},
				{
					_id: 'no-snapshot',
					coreInstanceId: 'core-no-snapshot',
					name: 'No Snapshot',
					gameVersion: '1.21.1',
					loader: 'quilt',
					syncEnabled: true,
				},
			],
			coreProfiles: [
				{
					id: 'sync-a',
					core_instance_id: 'core-a',
					name: 'Shared Client',
					game_version: '1.21.1',
					loader: 'fabric',
					sync_enabled: true,
					current_snapshot_id: 'snapshot-a',
					created_at: '',
					updated_at: '',
				},
				{
					id: 'sync-no-snapshot',
					core_instance_id: 'core-no-snapshot',
					name: 'No Snapshot',
					game_version: '1.21.1',
					loader: 'quilt',
					sync_enabled: true,
					current_snapshot_id: null,
					created_at: '',
					updated_at: '',
				},
			],
			localProfiles: [
				{
					path: 'installed',
					profile_type: 'synced',
					core_instance_id: 'core-installed',
				},
			],
		})

		expect(profiles.map((profile) => profile.socialProfileId)).toEqual(['social-a', 'no-snapshot'])
		expect(profiles[0]).toMatchObject({
			coreProfileId: 'sync-a',
			currentSnapshotId: 'snapshot-a',
			availability: 'installable',
		})
		expect(profiles[1]).toMatchObject({
			coreProfileId: 'sync-no-snapshot',
			currentSnapshotId: null,
			availability: 'missing-snapshot',
		})
	})

	it('keeps shared client profiles visible when Core is unavailable', () => {
		const profiles = resolveInstallableSyncedProfiles({
			socialProfiles: [
				{
					_id: 'offline',
					coreInstanceId: 'core-offline',
					name: 'Offline Core',
					gameVersion: '1.21.1',
					loader: 'neoforge',
					syncEnabled: true,
				},
			],
			coreProfiles: null,
			coreAvailable: false,
			localProfiles: [],
		})

		expect(profiles).toHaveLength(1)
		expect(profiles[0]).toMatchObject({
			availability: 'core-unavailable',
			coreProfileId: null,
			currentSnapshotId: null,
		})
	})

	it('installs a Core snapshot through injected app callbacks', async () => {
		const archiveBytes = Uint8Array.from([1, 2, 3])
		const downloadSyncSnapshot = vi.fn(async () => new Blob([archiveBytes]))
		const createProfile = vi.fn(async () => 'created-profile')
		const getProfileFullPath = vi.fn(async () => 'profiles/created-profile')
		const joinPath = vi.fn(async (...parts: string[]) => parts.join('/'))
		const writeFile = vi.fn(async () => undefined)
		const removeFile = vi.fn(async () => undefined)
		const installMrpackFromPath = vi.fn(async () => undefined)
		const editProfile = vi.fn(async () => undefined)
		const linkServerId = vi.fn()

		const installedPath = await installSyncedProfileFromCore({
			core: { downloadSyncSnapshot },
			profile: {
				socialProfileId: 'social-a',
				coreProfileId: 'sync-a',
				coreInstanceId: 'core-a',
				clientProfileId: null,
				name: 'Shared Client',
				gameVersion: '1.21.1',
				loader: 'fabric',
				currentSnapshotId: 'snapshot-a',
				availability: 'installable',
				unavailableReason: null,
			},
			createProfile,
			getProfileFullPath,
			joinPath,
			writeFile,
			removeFile,
			installMrpackFromPath,
			editProfile,
			linkServerId,
		})

		expect(installedPath).toBe('created-profile')
		expect(downloadSyncSnapshot).toHaveBeenCalledWith('sync-a', 'snapshot-a')
		expect(createProfile).toHaveBeenCalledWith({
			name: 'Shared Client',
			gameVersion: '1.21.1',
			loader: 'fabric',
			profileType: 'synced',
		})
		expect(writeFile).toHaveBeenCalledWith(
			'profiles/created-profile/sync-a-snapshot-a.mrpack',
			archiveBytes,
		)
		expect(installMrpackFromPath).toHaveBeenCalledWith(
			'profiles/created-profile/sync-a-snapshot-a.mrpack',
			'created-profile',
		)
		expect(editProfile).toHaveBeenCalledWith('created-profile', {
			profile_type: 'synced',
			core_instance_id: 'core-a',
		})
		expect(removeFile).toHaveBeenCalledWith('profiles/created-profile/sync-a-snapshot-a.mrpack')
		expect(linkServerId).toHaveBeenCalledWith('created-profile', 'core-a')
	})
})
