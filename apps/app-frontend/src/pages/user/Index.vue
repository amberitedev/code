<script setup lang="ts">
import type {
	AmberiteProfile,
	ProfileSectionVisibility,
	ProfileView,
	ProfileVisibilitySettings,
} from '@amberite/amberite-api'
import type { Labrinth } from '@modrinth/api-client'
import {
	BadgeCheckIcon,
	BanIcon,
	BoxIcon,
	CalendarIcon,
	CheckIcon,
	ClipboardCopyIcon,
	CrownIcon,
	DownloadIcon,
	EditIcon,
	ExternalIcon,
	GlobeIcon,
	LibraryIcon,
	LinkIcon,
	LockIcon,
	MoreVerticalIcon,
	ReportIcon,
	SendIcon,
	SettingsIcon,
	ShieldIcon,
	UserIcon,
	UserPlusIcon,
	UsersIcon,
	UserXIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	Card,
	commonMessages,
	ContentPageHeader,
	defineMessages,
	DropdownSelect,
	EmptyState,
	getProjectTypeCategoryMessage,
	injectModrinthClient,
	injectNotificationManager,
	LoadingIndicator,
	NavTabs,
	OverflowMenu,
	ProjectCard,
	ProjectCardList,
	SettingsLabel,
	StyledInput,
	useCompactNumber,
	useFormatDateTime,
	useFormatNumber,
	UserBadges,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { Component } from 'vue'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import { useSocialClient } from '@/composables/useSocialClient'
import { get_loaders } from '@/helpers/tags'
import {
	hasActivePride26Midas,
	hasMidasBadge,
	hasPride26Badge,
} from '@/helpers/user-campaigns.ts'
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
type FriendGroupManageRole = 'admin' | 'member'
type ModalRef = { show: (event?: MouseEvent) => void; hide: () => void }
type AmberiteHeaderStat = {
	id: string
	icon: Component
	label: string
	tooltip?: string
}

const visibilityOptions: ProfileSectionVisibility[] = [
	'everyone',
	'friends',
	'friend_group',
	'private',
]

const visibilityLabels: Record<ProfileSectionVisibility, string> = {
	everyone: 'Everyone',
	friends: 'Friends',
	friend_group: 'Friend group',
	private: 'Private',
}

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
const queryClient = useQueryClient()
const notifications = injectNotificationManager()
const route = useRoute()
const router = useRouter()
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
const profileModal = ref<ModalRef | null>(null)
const accountModal = ref<ModalRef | null>(null)
const profileSaving = ref(false)
const profileActionPending = ref<string | null>(null)
const managementRole = ref<FriendGroupManageRole>('member')
const managementPreset = ref('')
const profileForm = ref({
	displayName: '',
	username: '',
	bio: '',
	avatarUrl: '',
	favoriteModpackProjectIds: '',
	showcaseAchievementIds: '',
	visibility: {
		friends: 'friends',
		friendGroup: 'friend_group',
		corePresence: 'friend_group',
		favoriteModpacks: 'everyone',
		achievements: 'everyone',
	} as ProfileVisibilitySettings,
})

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
	queryFn: () => social.getProfileView(routeUser.value),
	enabled: () => routeUser.value.length > 0,
	retry: false,
})

const amberiteCurrentProfileQuery = useQuery({
	queryKey: ['amberite-current-profile'],
	queryFn: () => social.currentProfile(),
	enabled: () => !!amberiteProfileQuery.data.value?.viewer.isSelf,
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
const amberiteFavoriteProjectIds = computed(
	() => amberiteProfile.value?.favoriteModpacks?.projectIds ?? [],
)
const amberiteFavoriteProjectsQuery = useQuery({
	queryKey: computed(() => [
		'amberite-user-profile',
		routeUser.value,
		'favorite-modpacks',
		amberiteFavoriteProjectIds.value,
	]),
	queryFn: () => client.labrinth.projects_v2.getMultiple(amberiteFavoriteProjectIds.value),
	enabled: () => amberiteFavoriteProjectIds.value.length > 0,
	retry: false,
})
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
const amberiteFavoriteProjects = computed(() => {
	const projectsById = new Map(
		(amberiteFavoriteProjectsQuery.data.value ?? []).map((project) => [project.id, project]),
	)
	return amberiteFavoriteProjectIds.value
		.map((id) => projectsById.get(id))
		.filter((project): project is ProfileProject => !!project)
})
const amberiteFavoriteProjectsLoading = computed(
	() => amberiteFavoriteProjectsQuery.isLoading.value,
)
const amberiteAchievements = computed(() => amberiteProfile.value?.achievements?.achievementIds ?? [])
const amberiteFriends = computed(() => amberiteProfile.value?.friends ?? null)
const amberiteManagement = computed(() => amberiteProfile.value?.management ?? null)
const amberiteActions = computed<ProfileView['actions']>(() => amberiteProfile.value?.actions ?? {})
const currentAmberiteProfile = computed<AmberiteProfile | null>(
	() => amberiteCurrentProfileQuery.data.value ?? null,
)
const showAmberiteFavoriteModpacks = computed(
	() =>
		amberiteFavoriteProjectIds.value.length > 0 &&
		(amberiteFavoriteProjectsLoading.value || amberiteFavoriteProjects.value.length > 0),
)
const showAmberiteAchievements = computed(() => amberiteAchievements.value.length > 0)
const showAmberiteFriends = computed(
	() =>
		!!amberiteFriends.value &&
		(amberiteFriends.value.mutual.length > 0 || amberiteFriends.value.items.length > 0),
)
const showAmberiteManagement = computed(() => {
	const management = amberiteManagement.value
	return !!(
		management?.canUpdateRole ||
		management?.canKick ||
		management?.canBan ||
		management?.canUnban
	)
})
const hasAmberiteProfileActions = computed(() => {
	const actions = amberiteActions.value
	return !!(
		isAmberiteProfile.value &&
		amberiteProfile.value &&
		(actions.editProfile ||
			actions.editAccount ||
			actions.addFriend ||
			actions.cancelFriendRequest ||
			actions.acceptFriendRequest ||
			actions.removeFriend ||
			actions.inviteToFriendGroup ||
			actions.unblock ||
			actions.block)
	)
})
const amberiteHeaderStats = computed<AmberiteHeaderStat[]>(() => {
	const profile = amberiteProfile.value
	if (!profile) return []

	const items: AmberiteHeaderStat[] = [
		{
			id: 'relationship',
			icon: UserIcon,
			label: relationshipHeaderLabel(profile),
			tooltip: profile.viewer.blocked ? 'Private sections unavailable' : undefined,
		},
	]

	if (amberiteFriends.value && (amberiteFriends.value.count > 0 || amberiteFriends.value.hasMore)) {
		items.push({
			id: 'friends',
			icon: UsersIcon,
			label: `Shows ${countLabel(amberiteFriends.value.count, amberiteFriends.value.hasMore)} visible ${
				amberiteFriends.value.count === 1 && !amberiteFriends.value.hasMore ? 'friend' : 'friends'
			}`,
			tooltip:
				amberiteFriends.value.mutualCount > 0
					? `${countLabel(amberiteFriends.value.mutualCount, amberiteFriends.value.mutualHasMore)} mutual`
					: undefined,
		})
	}

	if (profile.friendGroup) {
		items.push({
			id: 'friend-group',
			icon: UsersIcon,
			label: `Member of ${profile.friendGroup.group.name ?? 'a friend group'}`,
			tooltip: formatMembershipDetail(
				profile.friendGroup.membership.role,
				profile.friendGroup.membership.permissionPreset,
			),
		})
	}

	if (profile.corePresence) {
		items.push({
			id: 'core-presence',
			icon: ShieldIcon,
			label: `Uses ${profile.corePresence.name ?? 'Core'}`,
			tooltip: formatCorePresenceDetail(profile.corePresence),
		})
	}

	const updatedLabel = formatTimestamp(profile.user.profileUpdatedAt)
	if (updatedLabel) {
		items.push({
			id: 'updated',
			icon: EditIcon,
			label: `Updated ${updatedLabel}`,
		})
	}

	return items
})
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

watch(
	amberiteProfile,
	(value) => {
		if (value) {
			syncProfileForm(value)
			syncManagementForm(value)
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
		id: 'edit-profile',
		shown: isAmberiteProfile.value && !!amberiteActions.value.editProfile,
		disabled: profileSaving.value,
		action: openProfileSettings,
	},
	{
		id: 'account',
		shown: isAmberiteProfile.value && !!amberiteActions.value.editAccount,
		action: openAccountSettings,
	},
	{
		id: 'add-friend',
		shown: isAmberiteProfile.value && !!amberiteActions.value.addFriend,
		disabled: !!profileActionPending.value,
		action: addFriendFromProfile,
	},
	{
		id: 'cancel-friend-request',
		shown: isAmberiteProfile.value && !!amberiteActions.value.cancelFriendRequest,
		disabled: !!profileActionPending.value,
		action: cancelFriendRequestFromProfile,
	},
	{
		id: 'accept-friend-request',
		shown: isAmberiteProfile.value && !!amberiteActions.value.acceptFriendRequest,
		disabled: !!profileActionPending.value,
		action: acceptFriendRequestFromProfile,
	},
	{
		id: 'remove-friend',
		shown: isAmberiteProfile.value && !!amberiteActions.value.removeFriend,
		disabled: !!profileActionPending.value,
		action: removeFriendFromProfile,
	},
	{
		id: 'invite-to-friend-group',
		shown: isAmberiteProfile.value && !!amberiteActions.value.inviteToFriendGroup,
		disabled: !!profileActionPending.value,
		action: inviteToFriendGroupFromProfile,
	},
	{
		id: 'unblock',
		shown: isAmberiteProfile.value && !!amberiteActions.value.unblock,
		disabled: !!profileActionPending.value,
		action: unblockFromProfile,
	},
	{
		id: 'block-user',
		shown: isAmberiteProfile.value && !!amberiteActions.value.block && !amberiteActions.value.unblock,
		color: 'red' as const,
		hoverFilled: true,
		disabled: !!profileActionPending.value,
		action: blockFromProfile,
	},
	{
		divider: true,
		shown: hasAmberiteProfileActions.value,
	},
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

function syncProfileForm(profile: ProfileView) {
	const settings = profile.settings ?? {
		friends: 'friends',
		friendGroup: 'friend_group',
		corePresence: 'friend_group',
		favoriteModpacks: 'everyone',
		achievements: 'everyone',
	}
	profileForm.value = {
		displayName: profile.user.displayName ?? profile.user.name ?? profile.user.username ?? '',
		username: profile.user.username ?? '',
		bio: profile.user.bio ?? '',
		avatarUrl: profile.user.avatar_url ?? profile.user.image ?? '',
		favoriteModpackProjectIds: (profile.favoriteModpacks?.projectIds ?? []).join(', '),
		showcaseAchievementIds: (profile.achievements?.achievementIds ?? []).join(', '),
		visibility: { ...settings },
	}
}

function syncManagementForm(profile: ProfileView) {
	const management = profile.management
	if (!management?.roles?.length) {
		managementRole.value = 'member'
		managementPreset.value = ''
		return
	}
	managementRole.value = management.roles.includes(management.role as FriendGroupManageRole)
		? (management.role as FriendGroupManageRole)
		: management.roles[0]
	managementPreset.value = management.permissionPreset ?? managementRole.value
}

function openProfileSettings(event?: MouseEvent) {
	if (amberiteProfile.value) syncProfileForm(amberiteProfile.value)
	profileModal.value?.show(event)
}

function openAccountSettings(event?: MouseEvent) {
	accountModal.value?.show(event)
}

async function saveProfileSettings() {
	if (!amberiteProfile.value || profileSaving.value) return
	profileSaving.value = true
	try {
		const avatarUrl = profileForm.value.avatarUrl.trim()
		const updated = await social.updateCurrentProfile({
			displayName: profileForm.value.displayName.trim(),
			username: profileForm.value.username.trim(),
			bio: profileForm.value.bio,
			avatar: avatarUrl ? { url: avatarUrl } : null,
			profileVisibility: profileForm.value.visibility as ProfileVisibilitySettings,
			favoriteModpackProjectIds: parseShowcaseInput(profileForm.value.favoriteModpackProjectIds),
			showcaseAchievementIds: parseShowcaseInput(profileForm.value.showcaseAchievementIds),
		})
		await refreshAmberiteProfile()
		profileModal.value?.hide()
		notifications.addNotification({
			type: 'success',
			title: 'Profile updated',
			text: 'Your Amberite profile settings were saved.',
		})
		if (updated.username && updated.username !== routeUser.value) {
			void router.replace(`/user/${encodeURIComponent(updated.username)}`)
		}
	} catch (error) {
		notifications.addNotification({
			type: 'error',
			title: 'Profile update failed',
			text: getErrorMessage(error),
		})
	} finally {
		profileSaving.value = false
	}
}

function parseShowcaseInput(value: string): string[] {
	return value
		.split(/[\n,]/)
		.map((item) => item.trim())
		.filter(Boolean)
}

async function refreshAmberiteProfile() {
	await queryClient.invalidateQueries({ queryKey: ['amberite-user-profile', routeUser.value] })
	await queryClient.invalidateQueries({ queryKey: ['amberite-current-profile'] })
}

async function runProfileAction(
	id: string,
	successTitle: string,
	action: () => Promise<unknown>,
) {
	if (profileActionPending.value) return
	profileActionPending.value = id
	try {
		await action()
		await refreshAmberiteProfile()
		notifications.addNotification({
			type: 'success',
			title: successTitle,
			text: 'The profile state was updated.',
		})
	} catch (error) {
		notifications.addNotification({
			type: 'error',
			title: 'Profile action failed',
			text: getErrorMessage(error),
		})
	} finally {
		profileActionPending.value = null
	}
}

function targetAmberiteUserId(): string | null {
	return amberiteProfile.value?.user.userId ?? null
}

async function addFriendFromProfile() {
	const userId = targetAmberiteUserId()
	if (!userId) return
	await runProfileAction('add-friend', 'Friend request sent', () =>
		social.sendFriendRequest({ targetUserId: userId }),
	)
}

async function cancelFriendRequestFromProfile() {
	const requestId = amberiteActions.value.cancelFriendRequest?.requestId
	if (!requestId) return
	await runProfileAction('cancel-friend-request', 'Friend request canceled', () =>
		social.cancelFriendRequest(requestId),
	)
}

async function acceptFriendRequestFromProfile() {
	const requestId = amberiteActions.value.acceptFriendRequest?.requestId
	if (!requestId) return
	await runProfileAction('accept-friend-request', 'Friend request accepted', () =>
		social.respondFriendRequest(requestId, true),
	)
}

async function removeFriendFromProfile() {
	const userId = targetAmberiteUserId()
	if (!userId) return
	await runProfileAction('remove-friend', 'Friend removed', () => social.removeFriend(userId))
}

async function blockFromProfile() {
	const userId = targetAmberiteUserId()
	if (!userId) return
	await runProfileAction('block-user', 'User blocked', () => social.blockUser(userId))
}

async function unblockFromProfile() {
	const userId = targetAmberiteUserId()
	if (!userId) return
	await runProfileAction('unblock-user', 'User unblocked', () => social.unblockUser(userId))
}

async function inviteToFriendGroupFromProfile() {
	const userId = targetAmberiteUserId()
	const action = amberiteActions.value.inviteToFriendGroup
	if (!userId || !action) return
	await runProfileAction('invite-to-friend-group', 'Friend group invitation sent', () =>
		social.createFriendGroupInvite({
			friendGroupId: action.friendGroupId,
			inviteeUserId: userId,
		}),
	)
}

async function saveManagedMember() {
	const management = amberiteManagement.value
	if (!management?.canUpdateRole) return
	await runProfileAction('update-member-role', 'Member role updated', () =>
		social.updateMemberRole({
			friendGroupId: management.friendGroupId,
			userId: management.userId,
			role: managementRole.value,
			permissionPreset: managementPreset.value.trim() || managementRole.value,
		}),
	)
}

async function kickManagedMember() {
	const management = amberiteManagement.value
	if (!management?.canKick) return
	await runProfileAction('kick-member', 'Member removed', () =>
		social.removeMember(management.friendGroupId, management.userId),
	)
}

async function banManagedMember() {
	const management = amberiteManagement.value
	if (!management?.canBan) return
	await runProfileAction('ban-member', 'Member banned', () =>
		social.banMember(management.friendGroupId, management.userId),
	)
}

async function unbanManagedMember() {
	const management = amberiteManagement.value
	if (!management?.canUnban) return
	await runProfileAction('unban-member', 'User unbanned', () =>
		social.unbanMember(management.friendGroupId, management.userId),
	)
}

function visibilityDisplayName(value: ProfileSectionVisibility) {
	return visibilityLabels[value]
}

function relationshipHeaderLabel(profile: ProfileView) {
	if (profile.viewer.viewerBlockedTarget) return 'Blocked'
	if (profile.viewer.targetBlockedViewer) return 'Unavailable'
	switch (profile.viewer.relationship) {
		case 'self':
			return 'Your profile'
		case 'group_manager':
			return 'Managed member'
		case 'friend_group':
			return 'Shared group'
		case 'friend':
			return 'Direct friend'
		default:
			return 'Amberite user'
	}
}

function countLabel(count: number, hasMore?: boolean) {
	return `${formatNumber(count)}${hasMore ? '+' : ''}`
}

function formatTimestamp(timestamp: number | null | undefined) {
	if (!timestamp) return null
	return formatRelativeTime(new Date(timestamp).toISOString())
}

function formatDisplayLabel(value: string | null | undefined) {
	const label = value?.replace(/[_-]/g, ' ').trim()
	if (!label) return null
	return label.charAt(0).toUpperCase() + label.slice(1)
}

function formatMembershipDetail(role: string, permissionPreset?: string | null) {
	const roleLabel = formatDisplayLabel(role) ?? 'Member'
	const presetLabel = formatDisplayLabel(permissionPreset)
	if (!presetLabel || presetLabel === roleLabel) return roleLabel
	return `${roleLabel} · ${presetLabel}`
}

function formatCorePresenceDetail(corePresence: NonNullable<ProfileView['corePresence']>) {
	const details = [
		formatDisplayLabel(corePresence.status),
		corePresence.lastSeenAt
			? `Last seen ${formatRelativeTime(new Date(corePresence.lastSeenAt).toISOString())}`
			: null,
	].filter((detail): detail is string => !!detail)

	return details.join(' · ') || undefined
}

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

function amberiteProfileUser(profile: ProfileView): ProfileUser {
	const user = profile.user
	const username = user.username ?? user.name ?? user.displayName ?? user.userId
	return {
		id: user.userId,
		username,
		avatar_url: user.avatar_url ?? user.image ?? undefined,
		bio: user.bio ?? undefined,
		created: user.created,
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

function copyText(value: string | null | undefined) {
	if (value) void navigator.clipboard.writeText(value)
}
</script>

<template>
	<ModalWrapper ref="profileModal" header="Profile settings">
		<div class="profile-settings-modal flex w-[min(42rem,calc(100vw-4rem))] flex-col gap-5">
			<section class="flex flex-col gap-3">
				<SettingsLabel title="Identity" description="Profile fields shown on your Amberite page." />
				<div class="grid gap-3 md:grid-cols-2">
					<StyledInput
						v-model="profileForm.displayName"
						type="text"
						placeholder="Display name"
						wrapper-class="w-full"
					/>
					<StyledInput
						v-model="profileForm.username"
						type="text"
						placeholder="Username"
						wrapper-class="w-full"
					/>
				</div>
				<StyledInput
					v-model="profileForm.avatarUrl"
					type="text"
					placeholder="Avatar URL"
					wrapper-class="w-full"
				/>
				<textarea
					v-model="profileForm.bio"
					rows="4"
					class="box-border min-h-28 w-full resize-y rounded-xl border border-solid border-surface-5 bg-surface-3 p-3 text-primary outline-none focus:border-brand"
					placeholder="Bio"
				/>
			</section>

			<section class="flex flex-col gap-3">
				<SettingsLabel
					title="Showcase"
					description="Comma-separated Modrinth project IDs and achievement IDs."
				/>
				<StyledInput
					v-model="profileForm.favoriteModpackProjectIds"
					type="text"
					placeholder="Favorite modpack project IDs"
					wrapper-class="w-full"
				/>
				<StyledInput
					v-model="profileForm.showcaseAchievementIds"
					type="text"
					placeholder="Achievement IDs"
					wrapper-class="w-full"
				/>
			</section>

			<section class="flex flex-col gap-3">
				<SettingsLabel title="Section visibility" />
				<div class="grid gap-3">
					<div class="grid items-center gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
						<span class="font-semibold text-primary">Friends</span>
						<DropdownSelect
							v-model="profileForm.visibility.friends"
							name="friends-visibility"
							:options="visibilityOptions"
							:display-name="visibilityDisplayName"
						/>
					</div>
					<div class="grid items-center gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
						<span class="font-semibold text-primary">Friend group</span>
						<DropdownSelect
							v-model="profileForm.visibility.friendGroup"
							name="friend-group-visibility"
							:options="visibilityOptions"
							:display-name="visibilityDisplayName"
						/>
					</div>
					<div class="grid items-center gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
						<span class="font-semibold text-primary">Core presence</span>
						<DropdownSelect
							v-model="profileForm.visibility.corePresence"
							name="core-presence-visibility"
							:options="visibilityOptions"
							:display-name="visibilityDisplayName"
						/>
					</div>
					<div class="grid items-center gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
						<span class="font-semibold text-primary">Favorite modpacks</span>
						<DropdownSelect
							v-model="profileForm.visibility.favoriteModpacks"
							name="favorite-modpacks-visibility"
							:options="visibilityOptions"
							:display-name="visibilityDisplayName"
						/>
					</div>
					<div class="grid items-center gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
						<span class="font-semibold text-primary">Achievements</span>
						<DropdownSelect
							v-model="profileForm.visibility.achievements"
							name="achievements-visibility"
							:options="visibilityOptions"
							:display-name="visibilityDisplayName"
						/>
					</div>
				</div>
			</section>

			<div class="flex justify-end">
				<ButtonStyled color="brand">
					<button :disabled="profileSaving" @click="saveProfileSettings">
						<CheckIcon aria-hidden="true" />
						{{ profileSaving ? 'Saving...' : 'Save profile' }}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</ModalWrapper>

	<ModalWrapper ref="accountModal" header="Account settings">
		<div class="flex w-[min(34rem,calc(100vw-4rem))] flex-col gap-4">
			<SettingsLabel title="Private account" description="Account-only Amberite fields." />
			<div class="grid gap-3 text-sm">
				<div class="grid gap-1">
					<span class="font-semibold text-secondary">User ID</span>
					<div class="flex min-w-0 items-center justify-between gap-2 rounded-xl bg-surface-3 p-3">
						<span class="truncate text-primary">{{ currentAmberiteProfile?.userId ?? user?.id }}</span>
						<ButtonStyled circular type="transparent">
							<button aria-label="Copy user ID" @click="copyText(currentAmberiteProfile?.userId ?? user?.id)">
								<ClipboardCopyIcon aria-hidden="true" />
							</button>
						</ButtonStyled>
					</div>
				</div>
				<div class="grid gap-1">
					<span class="font-semibold text-secondary">Friend code</span>
					<div class="flex min-w-0 items-center justify-between gap-2 rounded-xl bg-surface-3 p-3">
						<span class="truncate text-primary">{{ currentAmberiteProfile?.friendCode ?? 'Unavailable' }}</span>
						<ButtonStyled circular type="transparent">
							<button
								:disabled="!currentAmberiteProfile?.friendCode"
								aria-label="Copy friend code"
								@click="copyText(currentAmberiteProfile?.friendCode)"
							>
								<ClipboardCopyIcon aria-hidden="true" />
							</button>
						</ButtonStyled>
					</div>
				</div>
				<div class="grid gap-1">
					<span class="font-semibold text-secondary">Email</span>
					<span class="rounded-xl bg-surface-3 p-3 text-primary">
						{{ currentAmberiteProfile?.email ?? 'No email on account' }}
					</span>
				</div>
				<div class="grid gap-1">
					<span class="font-semibold text-secondary">Auth providers</span>
					<span class="rounded-xl bg-surface-3 p-3 text-primary">
						{{ currentAmberiteProfile?.auth_providers?.join(', ') || 'Minecraft' }}
					</span>
				</div>
			</div>
		</div>
	</ModalWrapper>

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
						<div v-tooltip="formatDateTime(user.created)" class="flex items-center gap-2 font-semibold">
							<CalendarIcon class="h-6 w-6 text-secondary" />
							{{ formatMessage(messages.profileJoinedLabel) }}
							{{ formatRelativeTime(user.created) }}
						</div>
					</template>
					<template v-else>
						<div
							v-for="(stat, index) in amberiteHeaderStats"
							:key="stat.id"
							v-tooltip="stat.tooltip"
							class="flex min-w-0 max-w-[15rem] items-center gap-2 font-semibold"
							:class="{
								'border-0 border-l border-solid border-divider pl-4': index > 0,
							}"
						>
							<component :is="stat.icon" class="h-6 w-6 shrink-0 text-secondary" aria-hidden="true" />
							<span class="truncate">{{ stat.label }}</span>
						</div>
						<div
							v-tooltip="formatDateTime(user.created)"
							class="flex items-center gap-2 border-0 border-l border-solid border-divider pl-4 font-semibold"
						>
							<CalendarIcon class="h-6 w-6 text-secondary" />
							{{ formatMessage(messages.profileJoinedLabel) }}
							{{ formatRelativeTime(user.created) }}
						</div>
					</template>
				</template>
				<template #actions>
					<ButtonStyled size="large" circular type="transparent">
						<OverflowMenu
							:options="overflowOptions"
							:tooltip="`More options`"
							aria-label="More options"
						>
							<MoreVerticalIcon aria-hidden="true" />
							<template #edit-profile>
								<EditIcon aria-hidden="true" />
								Edit profile
							</template>
							<template #account>
								<SettingsIcon aria-hidden="true" />
								Account
							</template>
							<template #add-friend>
								<UserPlusIcon aria-hidden="true" />
								Add friend
							</template>
							<template #cancel-friend-request>
								<XIcon aria-hidden="true" />
								Cancel request
							</template>
							<template #accept-friend-request>
								<CheckIcon aria-hidden="true" />
								Accept request
							</template>
							<template #remove-friend>
								<UserXIcon aria-hidden="true" />
								Remove friend
							</template>
							<template #invite-to-friend-group>
								<SendIcon aria-hidden="true" />
								Invite to group
							</template>
							<template #unblock>
								<ShieldIcon aria-hidden="true" />
								Unblock
							</template>
							<template #block-user>
								<BanIcon aria-hidden="true" />
								Block
							</template>
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

			<div
				v-if="
					isAmberiteProfile &&
					amberiteProfile &&
					(showAmberiteFavoriteModpacks ||
						showAmberiteAchievements ||
						showAmberiteFriends ||
						showAmberiteManagement)
				"
				class="flex flex-col gap-6"
			>
				<section v-if="showAmberiteFavoriteModpacks" class="flex flex-col gap-3">
					<div class="flex items-center gap-2">
						<LibraryIcon class="size-5 text-secondary" aria-hidden="true" />
						<h2 class="m-0 text-base font-semibold text-contrast">Favorite modpacks</h2>
					</div>
					<div v-if="amberiteFavoriteProjectsLoading" class="flex min-h-24 items-center justify-center">
						<LoadingIndicator />
					</div>
					<ProjectCardList v-else-if="amberiteFavoriteProjects.length > 0" layout="list">
						<ProjectCard
							v-for="project in amberiteFavoriteProjects"
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
				</section>

				<section v-if="showAmberiteAchievements" class="flex flex-col gap-3">
					<div class="flex items-center gap-2">
						<CrownIcon class="size-5 text-secondary" aria-hidden="true" />
						<h2 class="m-0 text-base font-semibold text-contrast">Achievements</h2>
					</div>
					<div class="flex flex-wrap gap-2">
						<span
							v-for="achievement in amberiteAchievements"
							:key="achievement"
							class="rounded-full bg-surface-3 px-3 py-1 text-sm font-semibold text-primary"
						>
							{{ achievement }}
						</span>
					</div>
				</section>

				<section v-if="showAmberiteFriends && amberiteFriends" class="flex flex-col gap-3">
					<div class="flex items-center gap-2">
						<UsersIcon class="size-5 text-secondary" aria-hidden="true" />
						<h2 class="m-0 text-base font-semibold text-contrast">
							Friends · {{ countLabel(amberiteFriends.count, amberiteFriends.hasMore) }}
						</h2>
					</div>
					<div v-if="amberiteFriends.mutual.length > 0" class="flex flex-col gap-2">
						<h3 class="m-0 text-sm font-semibold text-primary">
							Mutual friends ·
							{{ countLabel(amberiteFriends.mutualCount, amberiteFriends.mutualHasMore) }}
						</h3>
						<div class="grid gap-2 md:grid-cols-2 xl:grid-cols-4">
							<RouterLink
								v-for="friend in amberiteFriends.mutual"
								:key="friend.userId"
								:to="`/user/${encodeURIComponent(friend.username ?? friend.userId)}`"
								class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-xl bg-surface-3 p-2 text-primary no-underline"
							>
								<Avatar
									:src="friend.avatar_url ?? friend.image"
									:alt="friend.username ?? friend.userId"
									size="32px"
									circle
								/>
								<span class="truncate text-sm font-semibold">
									{{ friend.displayName ?? friend.username ?? friend.userId }}
								</span>
							</RouterLink>
						</div>
					</div>
					<div v-if="amberiteFriends.items.length > 0" class="grid gap-2 md:grid-cols-2 xl:grid-cols-4">
						<RouterLink
							v-for="friend in amberiteFriends.items"
							:key="friend.userId"
							:to="`/user/${encodeURIComponent(friend.username ?? friend.userId)}`"
							class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-xl bg-surface-3 p-2 text-primary no-underline"
						>
							<Avatar
								:src="friend.avatar_url ?? friend.image"
								:alt="friend.username ?? friend.userId"
								size="32px"
								circle
							/>
							<span class="truncate text-sm font-semibold">
								{{ friend.displayName ?? friend.username ?? friend.userId }}
							</span>
						</RouterLink>
					</div>
				</section>

				<Card v-if="showAmberiteManagement && amberiteManagement">
					<div class="flex flex-col gap-4">
						<div class="flex items-center gap-2">
							<ShieldIcon class="size-5 text-secondary" aria-hidden="true" />
							<h2 class="m-0 text-base font-semibold text-contrast">Management</h2>
						</div>
						<div
							v-if="amberiteManagement.canUpdateRole"
							class="profile-management-controls grid gap-3 md:grid-cols-[auto_auto_1fr]"
						>
							<DropdownSelect
								v-model="managementRole"
								name="managed-member-role"
								:options="amberiteManagement.roles ?? ['member']"
							/>
							<StyledInput
								v-model="managementPreset"
								type="text"
								placeholder="Permission preset"
								wrapper-class="w-full"
							/>
							<ButtonStyled color="brand">
								<button :disabled="!!profileActionPending" @click="saveManagedMember">
									<CheckIcon aria-hidden="true" />
									Save role
								</button>
							</ButtonStyled>
						</div>
						<div class="flex flex-wrap gap-2">
							<ButtonStyled v-if="amberiteManagement.canKick" type="outlined">
								<button :disabled="!!profileActionPending" @click="kickManagedMember">
									<UserXIcon aria-hidden="true" />
									Kick
								</button>
							</ButtonStyled>
							<ButtonStyled v-if="amberiteManagement.canBan" type="outlined" color="red">
								<button :disabled="!!profileActionPending" @click="banManagedMember">
									<BanIcon aria-hidden="true" />
									Ban
								</button>
							</ButtonStyled>
							<ButtonStyled v-if="amberiteManagement.canUnban" type="outlined">
								<button :disabled="!!profileActionPending" @click="unbanManagedMember">
									<ShieldIcon aria-hidden="true" />
									Unban
								</button>
							</ButtonStyled>
						</div>
					</div>
				</Card>
			</div>

			<ProjectCardList v-else-if="visibleProjects.length > 0" layout="list">
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

.profile-settings-modal :deep(.animated-dropdown),
.profile-management-controls :deep(.animated-dropdown) {
	width: min(20rem, 100%);
}
</style>
