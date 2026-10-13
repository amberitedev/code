<template>
	<div class="w-full p-2">
		<UserProfilePageLayout
			v-if="profileReady"
			:user-id="userId"
			:account-source="isPublicProfile ? 'modrinth' : 'self-hosted'"
			:project-type="projectType"
			variant="app"
			site-url="https://modrinth.com"
			project-link-mode="app"
			:edit-profile-link="openProfileSettings"
			external-navigation
		>
			<template #project-actions="{ project }">
				<Button
					v-if="project.minecraft_server == null"
					type="outlined"
					class="!text-brand [&>svg]:!text-brand !shadow-[inset_0_0_0_1px_var(--color-brand)]"
					:disabled="isProjectInstalling(project.id)"
					@click.stop="installProject(project)"
				>
					<SpinnerIcon v-if="isProjectInstalling(project.id)" class="animate-spin" />
					<DownloadIcon v-else-if="project.project_types.includes('modpack')" />
					<PlusIcon v-else />
					{{
						formatMessage(
							isProjectInstalling(project.id)
								? commonMessages.installingLabel
								: project.project_types.includes('modpack')
									? commonMessages.installButton
									: messages.installToInstance,
						)
					}}
				</Button>
			</template>
		</UserProfilePageLayout>
	</div>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { DownloadIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Button,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	provideUserProfile,
	provideAuth,
	injectAuth,
	UserProfilePageLayout,
	useLoadingBarToken,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, inject, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import {
	block_user,
	change_user_avatar,
	delete_user_avatar,
	get_blocked_users,
	get_user_collections,
	get_user_organizations,
	get_user_profile,
	get_user_projects,
	patch_user,
	unblock_user,
} from '@/helpers/users'
import { apiClient } from '@/services/api-client'
import { config } from '@/config'

import { appSettingsModalOpenProfileKey } from '@/providers/app-settings-modal'
import { useBreadcrumb } from '@/providers/breadcrumbs'
import { injectContentInstall } from '@/providers/content-install'

const route = useRoute()
const isPublicProfile = computed(
	() => Boolean(config.accountApiUrl) && route.query.source === 'modrinth',
)
const inheritedAuth = injectAuth()
provideAuth({
	...inheritedAuth,
	user: computed(() => (isPublicProfile.value ? null : inheritedAuth.user.value)),
	session_token: computed(() => (isPublicProfile.value ? null : inheritedAuth.session_token.value)),
})
function publicProfileRequest<T>(path: string, version = 3) {
	return apiClient.request<T>(path, {
		api: 'labrinth',
		version,
		accountSource: 'modrinth',
		skipAuth: true,
	})
}
const router = useRouter()
const openProfileSettings = inject(appSettingsModalOpenProfileKey, () => {})
const queryClient = useQueryClient()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { install: installVersion } = injectContentInstall()
const installingProjectIds = ref(new Set<string>())

const messages = defineMessages({
	installToInstance: {
		id: 'app.user.project.install-to-instance',
		defaultMessage: 'Install to instance',
	},
})

function isProjectInstalling(projectId: string): boolean {
	return installingProjectIds.value.has(projectId)
}

function setProjectInstalling(projectId: string, installing: boolean): void {
	const next = new Set(installingProjectIds.value)
	if (installing) {
		next.add(projectId)
	} else {
		next.delete(projectId)
	}
	installingProjectIds.value = next
}

async function installProject(project: Labrinth.Projects.v3.Project): Promise<void> {
	if (isProjectInstalling(project.id)) return

	setProjectInstalling(project.id, true)
	try {
		await installVersion(
			project.id,
			null,
			null,
			'UserPage',
			() => setProjectInstalling(project.id, false),
			(instanceId) => router.push(`/instance/${encodeURIComponent(instanceId)}`),
		)
	} catch (error) {
		setProjectInstalling(project.id, false)
		handleError(error)
	}
}

const userProfile = provideUserProfile({
	getUser: (id: string) =>
		isPublicProfile.value
			? publicProfileRequest<Labrinth.Users.v3.User>(`/user/${encodeURIComponent(id)}`)
			: get_user_profile(id),
	getProjects: (id: string) =>
		isPublicProfile.value
			? publicProfileRequest<Labrinth.Projects.v3.Project[]>(
					`/user/${encodeURIComponent(id)}/projects`,
				)
			: get_user_projects(id),
	getOrganizations: (id: string) =>
		isPublicProfile.value
			? publicProfileRequest<Labrinth.Organizations.v3.Organization[]>(
					`/user/${encodeURIComponent(id)}/organizations`,
				)
			: get_user_organizations(id),
	getCollections: (id: string) =>
		isPublicProfile.value
			? publicProfileRequest<Labrinth.Collections.Collection[]>(
					`/user/${encodeURIComponent(id)}/collections`,
				)
			: get_user_collections(id),
	patchUser: patch_user,
	changeAvatar: async (userId, file, extension) => {
		await change_user_avatar(userId, new Uint8Array(await file.arrayBuffer()), extension)
	},
	deleteAvatar: delete_user_avatar,
	getBlockedUsers: get_blocked_users,
	blockUser: block_user,
	unblockUser: unblock_user,
})

function readRouteParam(param: unknown): string | undefined {
	const value = Array.isArray(param) ? param[0] : param
	return typeof value === 'string' && value.length > 0 ? value : undefined
}

// The profile on screen. It trails the route while the next profile loads, see the watcher below.
const userId = ref(readRouteParam(route.params.user) ?? '')
const projectType = computed(() => readRouteParam(route.params.projectType))

function getCachedUserSummary(id: string) {
	return queryClient.getQueryData<Labrinth.Users.v3.User>(
		isPublicProfile.value ? ['users', 'modrinth', 'summary', id] : ['users', 'summary', id],
	)
}

const profileQueryKey = (id: string) =>
	isPublicProfile.value ? ['user', 'modrinth', id] : ['user', id]

const { data: user } = useQuery({
	queryKey: computed(() => profileQueryKey(userId.value)),
	queryFn: () => userProfile.getUser(userId.value),
	enabled: false,
	staleTime: 30_000,
})

const breadcrumbUserId = ref(userId.value)
const breadcrumbLabel = ref(getCachedUserSummary(userId.value)?.username ?? userId.value)
const breadcrumbTo = ref(route.fullPath)
watch(
	[userId, user, () => route.fullPath],
	([currentUserId, currentUser, currentPath]) => {
		if (route.name !== 'User') return
		// While the next profile loads, keep the breadcrumb on the one still on screen.
		if (readRouteParam(route.params.user) !== currentUserId) return
		breadcrumbUserId.value = currentUserId
		breadcrumbLabel.value = currentUser?.username ?? currentUserId
		breadcrumbTo.value = currentPath
	},
	{ immediate: true, flush: 'sync' },
)

useBreadcrumb({
	slot: 'user',
	id: () => `user:${breadcrumbUserId.value}`,
	label: breadcrumbLabel,
	to: breadcrumbTo,
	visual: () => ({
		type: 'image',
		src: user.value?.avatar_url ?? getCachedUserSummary(breadcrumbUserId.value)?.avatar_url,
		alt: breadcrumbLabel.value,
		circle: true,
		tintBy: breadcrumbUserId.value,
	}),
})

/** Loads everything the profile shows, in parallel. Never rejects: the layout surfaces errors. */
async function ensureUserProfileData(id: string): Promise<void> {
	if (!id) return

	await Promise.allSettled([
		queryClient.ensureQueryData({
			queryKey: profileQueryKey(id),
			queryFn: () => userProfile.getUser(id),
			staleTime: 30_000,
		}),
		queryClient.ensureQueryData({
			queryKey: [...profileQueryKey(id), 'projects'],
			queryFn: () => userProfile.getProjects(id),
			staleTime: 30_000,
		}),
		queryClient.ensureQueryData({
			queryKey: [...profileQueryKey(id), 'organizations'],
			queryFn: () => userProfile.getOrganizations(id),
			staleTime: 30_000,
		}),
		queryClient.ensureQueryData({
			queryKey: [...profileQueryKey(id), 'collections'],
			queryFn: () => userProfile.getCollections(id),
			staleTime: 30_000,
		}),
	])
}

// The page opens at once and the layout mounts when the first profile has loaded, because the
// layout reads a missing user as "not found" and missing projects as "no projects".
const profileReady = ref(false)
useLoadingBarToken(computed(() => !profileReady.value))
void ensureUserProfileData(userId.value).then(() => (profileReady.value = true))

// Going from one profile to another keeps the current one on screen until the next is loaded, so
// the whole profile swaps at once. The navigation itself is not held back.
watch(
	() => readRouteParam(route.params.user),
	async (next) => {
		if (!next || next === userId.value) return
		await ensureUserProfileData(next)
		if (readRouteParam(route.params.user) === next) userId.value = next
	},
)
</script>
