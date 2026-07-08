<script setup lang="ts">
import type { AmberitePublicProfile } from '@amberite/amberite-api'
import type { Labrinth } from '@modrinth/api-client'
import {
	BadgeCheckIcon,
	BoxIcon,
	CalendarIcon,
	ClipboardCopyIcon,
	DownloadIcon,
	ExternalIcon,
	GlobeIcon,
	LibraryIcon,
	LinkIcon,
	LockIcon,
	MoreVerticalIcon,
	ReportIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	Card,
	commonMessages,
	ContentPageHeader,
	defineMessages,
	EmptyState,
	getProjectTypeCategoryMessage,
	injectModrinthClient,
	injectNotificationManager,
	LoadingIndicator,
	NavTabs,
	OverflowMenu,
	ProjectCard,
	ProjectCardList,
	UserBadges,
	useCompactNumber,
	useFormatDateTime,
	useFormatNumber,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { Component } from 'vue'
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import {
	hasActivePride26Midas,
	hasMidasBadge,
	hasPride26Badge,
} from '@/helpers/user-campaigns.ts'
import { useSocialClient } from '@/composables/useSocialClient'
import { get_loaders } from '@/helpers/tags'
import { useBreadcrumbs } from '@/store/breadcrumbs'

type ProfileProject = Labrinth.Projects.v2.Project
type ProfileUser = Labrinth.Users.v3.User
type ProjectUrlType =
	| 'mod'
	| 'modpack'
	| 'resourcepack'
	| 'shader'
	| 'plugin'
	| 'datapack'
	| 'server'
	| 'collection'
	| 'project'
type ProjectSorting = 'publish_time' | 'queue_time' | 'downloads'
type ProjectStatusPriority = { order: number; sort: ProjectSorting }

const projectStatusPriority: Record<
	Labrinth.Projects.v2.ProjectStatus,
	ProjectStatusPriority
> = {
	approved: { order: 1, sort: 'downloads' },
	scheduled: { order: 1, sort: 'downloads' },
	archived: { order: 2, sort: 'downloads' },
	unlisted: { order: 3, sort: 'downloads' },
	private: { order: 4, sort: 'downloads' },
	processing: { order: 5, sort: 'queue_time' },
	withheld: { order: 6, sort: 'publish_time' },
	rejected: { order: 7, sort: 'publish_time' },
	draft: { order: 8, sort: 'publish_time' },
	unknown: { order: 9, sort: 'publish_time' },
}

const PROJECT_TYPE_ORDER: ProjectUrlType[] = [
	'mod',
	'modpack',
	'resourcepack',
	'datapack',
	'shader',
	'plugin',
	'server',
	'collection',
]

const messages = defineMessages({
	profileProjectsLabel: {
		id: 'profile.label.projects',
		defaultMessage: '{count} {countPlural, plural, one {project} other {projects}}',
	},
	profileDownloadsLabel: {
		id: 'profile.label.downloads',
		defaultMessage: '{count} {countPlural, plural, one {download} other {downloads}}',
	},
	profileJoinedLabel: {
		id: 'profile.label.joined',
		defaultMessage: 'Joined',
	},
	bioFallbackUser: {
		id: 'profile.bio.fallback.user',
		defaultMessage: 'A Modrinth user.',
	},
	bioFallbackCreator: {
		id: 'profile.bio.fallback.creator',
		defaultMessage: 'A Modrinth creator.',
	},
	bioFallbackAmberite: {
		id: 'profile.bio.fallback.amberite',
		defaultMessage: 'An Amberite user.',
	},
	collectionLabel: {
		id: 'profile.label.collection',
		defaultMessage: 'Collection',
	},
	collectionsLabel: {
		id: 'profile.label.collections',
		defaultMessage: 'Collections',
	},
	collectionProjectsCount: {
		id: 'profile.collection.projects-count',
		defaultMessage: '{count, plural, one {# project} other {# projects}}',
	},
	profileOrganizations: {
		id: 'profile.label.organizations',
		defaultMessage: 'Organizations',
	},
	profileNoProjectsLabel: {
		id: 'profile.label.no-projects',
		defaultMessage: 'This user has no projects!',
	},
	profileNoCollectionsLabel: {
		id: 'profile.label.no-collections',
		defaultMessage: 'This user has no collections!',
	},
	userNotFoundError: {
		id: 'profile.error.not-found',
		defaultMessage: 'User not found',
	},
	loadErrorTitle: {
		id: 'profile.error.load-title',
		defaultMessage: 'Failed to load profile',
	},
	loadErrorText: {
		id: 'profile.error.load-text',
		defaultMessage: 'The profile could not be loaded.',
	},
	officialAccount: {
		id: 'profile.official-account',
		defaultMessage: 'Official Modrinth account',
	},
	officialAccountBio: {
		id: 'profile.official-account.bio.app',
		defaultMessage:
			'The official user account of Modrinth. Get support at support.modrinth.com or by emailing support@modrinth.com.',
	},
	openInBrowser: {
		id: 'profile.button.open-in-browser',
		defaultMessage: 'Open in browser',
	},
})

const client = injectModrinthClient()
const social = useSocialClient()
const notifications = injectNotificationManager()
const route = useRoute()
const breadcrumbs = useBreadcrumbs()
const { formatMessage } = useVIntl()
const formatNumber = useFormatNumber()
const { formatCompactNumber, formatCompactNumberPlural } = useCompactNumber()
const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({
	timeStyle: 'short',
	dateStyle: 'long',
})

const notifiedError = ref<unknown>(null)

const routeUser = computed(() => String(route.params.user ?? ''))
const routeUserPath = computed(() => encodeURIComponent(routeUser.value))
const routeProjectType = computed(() => {
	const param = route.params.projectType
	return Array.isArray(param) ? param[0] : param
})

const loadersQuery = useQuery({
	queryKey: ['tags', 'loaders'],
	queryFn: get_loaders,
	staleTime: 60 * 60 * 1000,
})

const userQuery = useQuery({
	queryKey: computed(() => ['user-profile', routeUser.value]),
	queryFn: () => client.labrinth.users_v3.get(routeUser.value),
	enabled: () => routeUser.value.length > 0,
	retry: false,
})

const amberiteProfileQuery = useQuery({
	queryKey: computed(() => ['amberite-user-profile', routeUser.value]),
	queryFn: () => social.getProfile(routeUser.value),
	enabled: () => routeUser.value.length > 0,
	retry: false,
})

const modrinthUserId = computed(() => userQuery.data.value?.id ?? '')

const projectsQuery = useQuery({
	queryKey: computed(() => ['user-profile', modrinthUserId.value, 'projects']),
	queryFn: () => client.labrinth.users_v2.getProjects(modrinthUserId.value),
	enabled: () => modrinthUserId.value.length > 0,
})

const organizationsQuery = useQuery({
	queryKey: computed(() => ['user-profile', modrinthUserId.value, 'organizations']),
	queryFn: () => client.labrinth.users_v2.getOrganizations(modrinthUserId.value),
	enabled: () => modrinthUserId.value.length > 0,
})

const collectionsQuery = useQuery({
	queryKey: computed(() => ['user-profile', modrinthUserId.value, 'collections']),
	queryFn: () => client.labrinth.users_v2.getCollections(modrinthUserId.value),
	enabled: () => modrinthUserId.value.length > 0,
})

const amberiteProfile = computed(() => amberiteProfileQuery.data.value ?? null)
const shouldUseAmberiteProfile = computed(
	() => !userQuery.data.value && !userQuery.isLoading.value && !!amberiteProfile.value,
)
const user = computed<ProfileUser | undefined>(
	() =>
		userQuery.data.value ??
		(shouldUseAmberiteProfile.value && amberiteProfile.value
			? amberiteProfileUser(amberiteProfile.value)
			: undefined),
)
const isAmberiteProfile = computed(() => shouldUseAmberiteProfile.value)
const projects = computed(() => projectsQuery.data.value ?? [])
const organizations = computed(() => organizationsQuery.data.value ?? [])
const collections = computed(() => collectionsQuery.data.value ?? [])
const loaders = computed(() => loadersQuery.data.value ?? [])

const profileLoading = computed(
	() => !user.value && (userQuery.isLoading.value || amberiteProfileQuery.isLoading.value),
)
const loadError = computed(() => {
	if (profileLoading.value) return null
	if (!user.value) return userQuery.error.value ?? amberiteProfileQuery.error.value
	if (isAmberiteProfile.value) return null
	return projectsQuery.error.value ?? organizationsQuery.error.value ?? collectionsQuery.error.value
})
const isLoading = computed(
	() =>
		profileLoading.value ||
		(!isAmberiteProfile.value &&
			(projectsQuery.isLoading.value ||
				organizationsQuery.isLoading.value ||
				collectionsQuery.isLoading.value)),
)

watch(
	user,
	(value) => {
		if (value) {
			breadcrumbs.setName('User', value.username)
		}
	},
	{ immediate: true },
)

watch(loadError, (error) => {
	if (!error || notifiedError.value === error) return

	notifiedError.value = error
	notifications.addNotification({
		type: 'error',
		title: formatMessage(messages.loadErrorTitle),
		text: getErrorMessage(error),
	})
})

const sortedOrganizations = computed(() =>
	[...organizations.value].sort((first, second) => first.name.localeCompare(second.name)),
)

const sortedCollections = computed(() =>
	[...collections.value].sort((first, second) => {
		const updatedDiff = new Date(second.updated).getTime() - new Date(first.updated).getTime()
		if (updatedDiff !== 0) return updatedDiff

		return new Date(second.created).getTime() - new Date(first.created).getTime()
	}),
)

const loaderProjectTypes = computed(() => {
	const map = new Map<string, Set<string>>()

	for (const loader of loaders.value) {
		map.set(loader.name, new Set(loader.supported_project_types))
	}

	return map
})

const normalizedRouteProjectType = computed<ProjectUrlType | undefined>(() => {
	const value = routeProjectType.value
	if (!value) return undefined

	if (value === 'collections') return 'collection'
	const singular = value.endsWith('s') ? value.slice(0, -1) : value

	return PROJECT_TYPE_ORDER.includes(singular as ProjectUrlType)
		? (singular as ProjectUrlType)
		: undefined
})

const visibleProjects = computed(() => {
	const selectedType = normalizedRouteProjectType.value
	const sortedProjects = [...projects.value].sort(projectUserSorting)

	if (!selectedType || selectedType === 'collection') {
		return selectedType === 'collection' ? [] : sortedProjects
	}

	return sortedProjects.filter((project) => getProjectUrlType(project) === selectedType)
})

const projectTypes = computed(() => {
	const types = new Set<ProjectUrlType>()

	for (const project of projects.value) {
		const type = getProjectUrlType(project)
		if (type !== 'project') {
			types.add(type)
		}
	}

	if (collections.value.length > 0) {
		types.add('collection')
	}

	return [...types].sort((first, second) => {
		const firstIndex = PROJECT_TYPE_ORDER.indexOf(first)
		const secondIndex = PROJECT_TYPE_ORDER.indexOf(second)
		return firstIndex - secondIndex
	})
})

const navLinks = computed(() => [
	{
		label: formatMessage(commonMessages.allProjectType),
		href: `/user/${routeUserPath.value}`,
	},
	...projectTypes.value.map((type) => ({
		label:
			type === 'collection'
				? formatMessage(messages.collectionsLabel)
				: formatMessage(getProjectTypeCategoryMessage(type)),
		href: `/user/${routeUserPath.value}/${pluralizeProjectType(type)}`,
	})),
])

const sumDownloads = computed(() =>
	projects.value.reduce((sum, project) => sum + project.downloads, 0),
)
const joinDate = computed(() => (user.value ? new Date(user.value.created) : new Date()))
const isModrinthUser = computed(() => user.value?.id === '2REoufqX')
const isAutoMod = computed(() => user.value?.id === '')
const isOfficialAccount = computed(
	() =>
		!isAmberiteProfile.value &&
		(isModrinthUser.value || isAutoMod.value || user.value?.id === 'GVFjtWTf'),
)
const profileSummary = computed(() => {
	if (isAmberiteProfile.value) return user.value?.bio || formatMessage(messages.bioFallbackAmberite)
	if (isModrinthUser.value) return formatMessage(messages.officialAccountBio)
	if (user.value?.bio) return user.value.bio
	if (projects.value.length > 0) return formatMessage(messages.bioFallbackCreator)
	return formatMessage(messages.bioFallbackUser)
})
const earliestProjectByType = computed<Record<string, Date>>(() => {
	const result: Record<string, Date> = {}

	for (const project of projects.value) {
		const type = getProjectUrlType(project)
		if (type === 'collection' || type === 'project') continue

		const date = new Date(project.published)
		const existingDate = result[type]
		if (!existingDate || date < existingDate) {
			result[type] = date
		}
	}

	return result
})
const hasMidas = computed(
	() => hasMidasBadge(user.value) || hasActivePride26Midas(user.value?.campaigns?.pride_26),
)
const hasPride = computed(() => hasPride26Badge(user.value?.campaigns?.pride_26))
const userBrowserUrl = computed(() =>
	!isAmberiteProfile.value && user.value
		? `https://modrinth.com/user/${user.value.username}`
		: 'https://modrinth.com',
)
const userPermalink = computed(() =>
	!isAmberiteProfile.value && user.value
		? `https://modrinth.com/user/${user.value.id}`
		: 'https://modrinth.com',
)

const overflowOptions = computed(() => [
	{
		id: 'open-in-browser',
		shown: !isAmberiteProfile.value,
		action: () => openExternalUrl(userBrowserUrl.value),
	},
	{
		id: 'copy-id',
		action: () => copyId(),
	},
	{
		id: 'copy-permalink',
		shown: !isAmberiteProfile.value,
		action: () => copyPermalink(),
	},
	{
		divider: true,
		shown: !isAmberiteProfile.value,
	},
	{
		id: 'report',
		shown: !isAmberiteProfile.value,
		color: 'red' as const,
		hoverFilled: true,
		action: () =>
			user.value && openExternalUrl(`https://modrinth.com/report?item=user&itemID=${user.value.id}`),
	},
])

const showProjectEmptyState = computed(
	() =>
		!isLoading.value &&
		!isAmberiteProfile.value &&
		normalizedRouteProjectType.value !== 'collection' &&
		visibleProjects.value.length === 0 &&
		(normalizedRouteProjectType.value !== undefined || collections.value.length === 0),
)
const showCollectionGrid = computed(
	() =>
		!isAmberiteProfile.value &&
		(!normalizedRouteProjectType.value || normalizedRouteProjectType.value === 'collection'),
)
const showCollectionEmptyState = computed(
	() =>
		!isLoading.value &&
		!isAmberiteProfile.value &&
		normalizedRouteProjectType.value === 'collection' &&
		sortedCollections.value.length === 0,
)

function getProjectSortValue(project: ProfileProject, sorting: ProjectSorting): number {
	switch (sorting) {
		case 'publish_time':
			return new Date(project.published).getTime()
		case 'queue_time':
			return new Date(project.queued || project.published).getTime()
		case 'downloads':
			return project.downloads
		default:
			return 0
	}
}

function projectUserSorting(first: ProfileProject, second: ProfileProject): number {
	const priority1 = projectStatusPriority[first.status] || projectStatusPriority.unknown
	const priority2 = projectStatusPriority[second.status] || projectStatusPriority.unknown

	if (priority1.order !== priority2.order) {
		return priority1.order - priority2.order
	} else if (priority1.sort !== priority2.sort) {
		return 0
	}

	return getProjectSortValue(second, priority2.sort) - getProjectSortValue(first, priority1.sort)
}

function getProjectUrlType(project: ProfileProject): ProjectUrlType {
	if ((project.project_type as string) === 'minecraft_java_server') return 'server'
	if (project.project_type !== 'mod') return project.project_type

	const projectLoaders = project.loaders ?? []
	const hasDataPackLoader = projectLoaders.some((loader) =>
		loaderProjectTypes.value.get(loader)?.has('datapack'),
	)
	const hasPluginLoader = projectLoaders.some((loader) =>
		loaderProjectTypes.value.get(loader)?.has('plugin'),
	)

	if (hasDataPackLoader) return 'datapack'
	if (hasPluginLoader) return 'plugin'
	return 'mod'
}

function pluralizeProjectType(type: ProjectUrlType) {
	return `${type}s`
}

function getProjectCardTags(project: ProfileProject) {
	return [...project.categories, ...project.loaders]
}

function getProjectCardAllTags(project: ProfileProject) {
	return [...project.categories, ...project.loaders, ...project.additional_categories]
}

function getProjectBanner(project: ProfileProject) {
	return project.gallery?.find((image) => image.featured)?.url
}

function getProjectLink(project: ProfileProject) {
	return `/project/${project.slug || project.id}`
}

function getCollectionUrl(collection: Labrinth.Collections.Collection) {
	return `https://modrinth.com/collection/${collection.id}`
}

function getOrganizationUrl(organization: Labrinth.Organizations.v3.Organization) {
	return `https://modrinth.com/organization/${organization.slug}`
}

function getCollectionStatusIcon(status: Labrinth.Collections.CollectionStatus): Component {
	switch (status) {
		case 'listed':
			return GlobeIcon
		case 'unlisted':
			return LinkIcon
		case 'private':
			return LockIcon
		case 'rejected':
			return XIcon
		default:
			return LibraryIcon
	}
}

function getCollectionStatusMessage(status: Labrinth.Collections.CollectionStatus) {
	switch (status) {
		case 'listed':
			return commonMessages.publicLabel
		case 'unlisted':
			return commonMessages.unlistedLabel
		case 'private':
			return commonMessages.privateLabel
		case 'rejected':
			return commonMessages.rejectedLabel
		default:
			return messages.collectionLabel
	}
}

function getErrorMessage(error: unknown) {
	if (error instanceof Error && error.message) return error.message
	return formatMessage(messages.loadErrorText)
}

function amberiteProfileUser(profile: AmberitePublicProfile): ProfileUser {
	const username = profile.username ?? profile.name ?? profile.displayName ?? profile.userId
	return {
		id: profile.userId,
		username,
		avatar_url: profile.avatar_url ?? profile.image ?? undefined,
		bio: profile.bio ?? undefined,
		created: profile.created,
		role: 'developer',
		badges: 0,
		campaigns: {
			pride_26: null,
		},
	}
}

function openExternalUrl(url: string) {
	openUrl(url).catch((error) => {
		notifications.addNotification({
			type: 'error',
			title: formatMessage(messages.loadErrorTitle),
			text: getErrorMessage(error),
		})
	})
}

function copyId() {
	if (!user.value) return
	void navigator.clipboard.writeText(user.value.id)
}

function copyPermalink() {
	void navigator.clipboard.writeText(userPermalink.value)
}
</script>

<template>
	<Teleport v-if="user && !isAmberiteProfile" defer to="#sidebar-teleport-target">
		<div class="flex flex-col gap-4">
			<Card v-if="sortedOrganizations.length > 0">
				<template #header>
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.profileOrganizations) }}
					</h2>
				</template>
				<div class="flex flex-wrap gap-2">
					<a
						v-for="organization in sortedOrganizations"
						:key="organization.id"
						v-tooltip="organization.name"
						:href="getOrganizationUrl(organization)"
						target="_blank"
						rel="noopener noreferrer"
						class="rounded-xl no-underline custom-focus-indicator"
						@click.prevent="openExternalUrl(getOrganizationUrl(organization))"
					>
						<Avatar
							:src="organization.icon_url"
							:alt="`Icon for ${organization.name}`"
							size="3rem"
						/>
					</a>
				</div>
			</Card>
			<UserBadges
				:downloads="sumDownloads"
				:join-date="joinDate"
				:role="user.role"
				:badges="user.badges"
				:has-midas="hasMidas"
				:has-pride="hasPride"
				:earliest-project-by-type="earliestProjectByType"
				class="rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4 pt-3"
			/>
		</div>
	</Teleport>

	<div class="box-border flex min-h-full flex-col gap-4 p-6">
		<div v-if="user" class="flex flex-col gap-4">
			<ContentPageHeader>
				<template #icon>
					<Avatar
						:src="user.avatar_url"
						:alt="user.username"
						:size="isModrinthUser ? '64px' : '96px'"
						circle
					/>
				</template>
				<template #title>
					{{ user.username }}
				</template>
				<template v-if="isOfficialAccount" #title-suffix>
					<BadgeCheckIcon
						v-tooltip="formatMessage(messages.officialAccount)"
						class="size-5 text-brand"
						fill="var(--color-brand-highlight)"
					/>
				</template>
				<template #summary>
					{{ profileSummary }}
				</template>
				<template v-if="!isModrinthUser" #stats>
					<template v-if="!isAmberiteProfile">
						<div
							class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold"
						>
							<BoxIcon class="h-6 w-6 text-secondary" />
							{{
								formatMessage(messages.profileProjectsLabel, {
									count: formatCompactNumber(projects.length),
									countPlural: formatCompactNumberPlural(projects.length),
								})
							}}
						</div>
						<div
							v-tooltip="formatNumber(sumDownloads)"
							class="flex items-center gap-2 border-0 border-r border-solid border-divider pr-4 font-semibold"
						>
							<DownloadIcon class="h-6 w-6 text-secondary" />
							{{
								formatMessage(messages.profileDownloadsLabel, {
									count: formatCompactNumber(sumDownloads),
									countPlural: formatCompactNumberPlural(sumDownloads),
								})
							}}
						</div>
					</template>
					<div v-tooltip="formatDateTime(user.created)" class="flex items-center gap-2 font-semibold">
						<CalendarIcon class="h-6 w-6 text-secondary" />
						{{ formatMessage(messages.profileJoinedLabel) }}
						{{ formatRelativeTime(user.created) }}
					</div>
				</template>
				<template #actions>
					<ButtonStyled size="large" circular type="transparent">
						<OverflowMenu
							:options="overflowOptions"
							:tooltip="`More options`"
							aria-label="More options"
						>
							<MoreVerticalIcon aria-hidden="true" />
							<template #open-in-browser>
								<ExternalIcon aria-hidden="true" />
								{{ formatMessage(messages.openInBrowser) }}
							</template>
							<template #copy-id>
								<ClipboardCopyIcon aria-hidden="true" />
								{{ formatMessage(commonMessages.copyIdButton) }}
							</template>
							<template #copy-permalink>
								<ClipboardCopyIcon aria-hidden="true" />
								{{ formatMessage(commonMessages.copyPermalinkButton) }}
							</template>
							<template #report>
								<ReportIcon aria-hidden="true" />
								{{ formatMessage(commonMessages.reportButton) }}
							</template>
						</OverflowMenu>
					</ButtonStyled>
				</template>
			</ContentPageHeader>

			<div v-if="navLinks.length > 2" class="max-w-full overflow-x-auto">
				<NavTabs :links="navLinks" replace />
			</div>

			<ProjectCardList v-if="visibleProjects.length > 0" layout="list">
				<ProjectCard
					v-for="project in visibleProjects"
					:key="project.id"
					:link="getProjectLink(project)"
					:title="project.title"
					:icon-url="project.icon_url"
					:date-updated="project.updated"
					:downloads="project.downloads"
					:summary="project.description"
					:tags="getProjectCardTags(project)"
					:all-tags="getProjectCardAllTags(project)"
					:followers="project.followers"
					:banner="getProjectBanner(project)"
					:color="project.color ?? undefined"
					:environment="{
						clientSide: project.client_side,
						serverSide: project.server_side,
					}"
					layout="list"
					:status="project.status"
				/>
			</ProjectCardList>

			<EmptyState
				v-if="showProjectEmptyState"
				type="empty"
				:heading="formatMessage(messages.profileNoProjectsLabel)"
			/>

			<div
				v-if="showCollectionGrid && sortedCollections.length > 0"
				class="collections-grid"
				:class="{ 'mt-2': visibleProjects.length > 0 }"
			>
				<a
					v-for="collection in sortedCollections"
					:key="collection.id"
					:href="getCollectionUrl(collection)"
					target="_blank"
					rel="noopener noreferrer"
					class="block text-primary no-underline custom-focus-indicator"
					@click.prevent="openExternalUrl(getCollectionUrl(collection))"
				>
					<Card class="h-full">
						<div class="flex h-full flex-col gap-4">
							<div class="grid grid-cols-[auto_1fr] gap-4">
								<Avatar :src="collection.icon_url" size="64px" />
								<div class="flex min-w-0 flex-col gap-2">
									<h2 class="m-0 text-lg font-bold text-contrast">
										{{ collection.name }}
									</h2>
									<div class="flex items-center gap-2 font-semibold text-secondary">
										<LibraryIcon class="size-5" aria-hidden="true" />
										{{ formatMessage(messages.collectionLabel) }}
									</div>
								</div>
							</div>
							<p class="m-0 grow text-primary">
								{{ collection.description }}
							</p>
							<div class="flex flex-wrap items-center gap-4 text-secondary">
								<div class="flex items-center gap-2">
									<BoxIcon class="size-5" aria-hidden="true" />
									{{
										formatMessage(messages.collectionProjectsCount, {
											count: collection.projects?.length || 0,
										})
									}}
								</div>
								<div class="flex items-center gap-2">
									<component :is="getCollectionStatusIcon(collection.status)" class="size-5" />
									{{ formatMessage(getCollectionStatusMessage(collection.status)) }}
								</div>
							</div>
						</div>
					</Card>
				</a>
			</div>

			<EmptyState
				v-if="showCollectionEmptyState"
				type="empty"
				:heading="formatMessage(messages.profileNoCollectionsLabel)"
			/>
		</div>

		<div v-else-if="isLoading" class="flex min-h-[24rem] items-center justify-center">
			<LoadingIndicator />
		</div>

		<EmptyState
			v-else
			type="error"
			:heading="formatMessage(messages.userNotFoundError)"
			:description="getErrorMessage(loadError)"
		/>
	</div>
</template>

<style scoped>
.collections-grid {
	display: grid;
	grid-template-columns: repeat(auto-fit, minmax(min(100%, 20rem), 1fr));
	gap: 1rem;
}
</style>
