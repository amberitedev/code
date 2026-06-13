export interface InstallTargetProfile {
	path: string
	game_version: string
	loader: string
}

export interface ModrinthProjectLike {
	id?: string
	title?: string
	project_type?: string
	versions?: string[]
}

export interface ModrinthDependencyLike {
	project_id?: string | null
	version_id?: string | null
	dependency_type?: string | null
	project_type?: string
}

export interface ModrinthVersionLike {
	id: string
	project_id?: string | null
	game_versions: string[]
	loaders: string[]
	date_published?: string
	dependencies?: ModrinthDependencyLike[]
}

export type InstallReason = 'standalone' | 'dependency' | 'modpack' | 'update'

export interface InstallVersionDependenciesOptions<
	TProject extends ModrinthProjectLike,
	TVersion extends ModrinthVersionLike,
	TProfile extends InstallTargetProfile,
> {
	profile: TProfile
	version: TVersion
	reason: InstallReason
	getProject: (projectId: string) => Promise<TProject | null>
	getVersion: (versionId: string) => Promise<TVersion | null>
	getVersions: (versionIds: string[]) => Promise<TVersion[]>
	isProjectInstalled: (profilePath: string, projectId: string) => Promise<boolean>
	installVersion: (
		profilePath: string,
		versionId: string,
		reason: InstallReason,
	) => Promise<unknown>
	onDependencyInstalling?: (project: TProject, version?: TVersion) => void
}

export function isVersionCompatible(
	version: ModrinthVersionLike,
	project: ModrinthProjectLike,
	instance: InstallTargetProfile,
): boolean {
	return (
		version.game_versions.includes(instance.game_version) &&
		(project.project_type === 'mod'
			? version.loaders.includes(instance.loader) || version.loaders.includes('datapack')
			: true)
	)
}

export function findPreferredVersion<TVersion extends ModrinthVersionLike>(
	versions: TVersion[],
	project: ModrinthProjectLike,
	instance: InstallTargetProfile,
): TVersion | undefined {
	const projectType = project.project_type ?? 'mod'
	const strictVersion = versions.find(
		(version) =>
			version.game_versions.includes(instance.game_version) &&
			(projectType === 'mod' ? version.loaders.includes(instance.loader) : true),
	)

	return (
		strictVersion ?? versions.find((version) => isVersionCompatible(version, project, instance))
	)
}

export async function installVersionDependencies<
	TProject extends ModrinthProjectLike,
	TVersion extends ModrinthVersionLike,
	TProfile extends InstallTargetProfile,
>(options: InstallVersionDependenciesOptions<TProject, TVersion, TProfile>): Promise<void> {
	const projectNames = new Map<string, string>()
	const visitedVersions = new Set<string>()
	const announcedProjects = new Set<string>()
	const queuedVersionIds = new Set<string>()
	const queuedProjectVersions = new Map<string, string>()
	const queuedInstalls: Array<{ versionId: string; projectId: string | null }> = []
	const installedProjectCache = new Map<string, boolean>()

	const storeProjectName = (project: TProject | null | undefined) => {
		if (project?.id && project.title) projectNames.set(project.id, project.title)
	}

	const isProjectInstalled = async (projectId: string | null | undefined) => {
		if (!projectId) return false
		if (installedProjectCache.has(projectId)) {
			return installedProjectCache.get(projectId) ?? false
		}
		const installed = await options.isProjectInstalled(options.profile.path, projectId)
		installedProjectCache.set(projectId, installed)
		return installed
	}

	const queueInstall = async (projectId: string | null, resolvedVersion: TVersion | null) => {
		if (!resolvedVersion?.id) return false

		const versionId = resolvedVersion.id
		const resolvedProjectId = projectId ?? resolvedVersion.project_id ?? null

		if (resolvedProjectId) {
			if (await isProjectInstalled(resolvedProjectId)) return false

			const existingVersionId = queuedProjectVersions.get(resolvedProjectId)
			if (existingVersionId && existingVersionId !== versionId) return false
			if (existingVersionId === versionId) return false
		}

		if (queuedVersionIds.has(versionId)) return false

		queuedVersionIds.add(versionId)
		if (resolvedProjectId) queuedProjectVersions.set(resolvedProjectId, versionId)
		queuedInstalls.push({ versionId, projectId: resolvedProjectId })
		return true
	}

	const announceDependency = async (projectId: string | null, resolvedVersion: TVersion | null) => {
		if (!options.onDependencyInstalling || !projectId) return
		if (announcedProjects.has(projectId)) return

		const depProject = await options.getProject(projectId).catch(() => null)
		if (!depProject) return

		storeProjectName(depProject)
		options.onDependencyInstalling(depProject, resolvedVersion ?? undefined)
		announcedProjects.add(projectId)
	}

	const resolveDependency = async (dep: ModrinthDependencyLike) => {
		let depVersion: TVersion | null = null
		let depProjectId = dep.project_id ?? null

		if (dep.version_id) {
			depVersion = await options.getVersion(dep.version_id).catch(() => null)
			if (!depVersion) return null

			depProjectId = depProjectId ?? depVersion.project_id ?? null
			if (depProjectId && !projectNames.has(depProjectId)) {
				const project = await options.getProject(depProjectId).catch(() => null)
				storeProjectName(project)
			}
		} else if (dep.project_id) {
			const depProject = await options.getProject(dep.project_id).catch(() => null)
			if (!depProject?.versions) return null

			storeProjectName(depProject)

			const depVersions = await options.getVersions(depProject.versions).catch(() => [])
			depVersion =
				findPreferredVersion(
					sortVersionsByPublishedDate(depVersions),
					{ ...depProject, ...dep },
					options.profile,
				) ?? null
			if (!depVersion) return null

			depProjectId = dep.project_id
		} else {
			return null
		}

		return { depVersion, depProjectId }
	}

	const collectDependenciesForVersion = async (inputVersion: TVersion | null | undefined) => {
		if (!inputVersion?.id || visitedVersions.has(inputVersion.id)) return
		visitedVersions.add(inputVersion.id)

		if (inputVersion.project_id && !projectNames.has(inputVersion.project_id)) {
			const project = await options.getProject(inputVersion.project_id).catch(() => null)
			storeProjectName(project)
		}

		for (const dep of inputVersion.dependencies ?? []) {
			if (dep.dependency_type !== 'required') continue
			if (dep.project_id === 'P7dR8mSH' && options.profile.loader === 'quilt') continue

			const resolved = await resolveDependency(dep)
			if (!resolved) continue

			const { depVersion, depProjectId } = resolved
			const queued = await queueInstall(depProjectId, depVersion)
			if (queued) await announceDependency(depProjectId, depVersion)

			await collectDependenciesForVersion(depVersion)
		}
	}

	await collectDependenciesForVersion(options.version)

	const batchSize = 8
	for (let index = 0; index < queuedInstalls.length; index += batchSize) {
		const batch = queuedInstalls.slice(index, index + batchSize)
		await Promise.all(
			batch.map(({ versionId }) =>
				options.installVersion(options.profile.path, versionId, options.reason),
			),
		)
	}
}

function sortVersionsByPublishedDate<TVersion extends ModrinthVersionLike>(
	versions: TVersion[],
): TVersion[] {
	return [...versions].sort(
		(a, b) => Date.parse(b.date_published ?? '') - Date.parse(a.date_published ?? ''),
	)
}
