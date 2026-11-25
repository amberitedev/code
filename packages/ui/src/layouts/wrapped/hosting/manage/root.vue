<template>
	<div
		v-if="serverError?.statusCode === 403 || serverError?.statusCode === 404"
		class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast"
	>
		<ErrorInformationCard
			title="An error occured."
			description="Please contact Modrinth Support."
			:icon="TransferIcon"
			icon-color="orange"
			:error-details="generalErrorDetails"
			:action="generalErrorAction"
		/>
	</div>
	<div
		v-else-if="serverError"
		class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast"
	>
		<ErrorInformationCard
			title="Can't reach your Core"
			description="Check that your Core is running and that you're online, then try again."
			:icon="TriangleAlertIcon"
			icon-color="red"
			:action="nodeUnavailableAction"
			:error-details="nodeUnavailableDetails"
		/>
	</div>
	<!-- SERVER START -->
	<div
		v-else-if="serverData"
		data-pyro-server-manager-root
		class="relative mx-auto box-border flex w-full min-w-0 flex-col gap-4 px-6"
		:style="{
			'--server-bg-image': serverImage
				? `url(${serverImage})`
				: `linear-gradient(180deg, rgba(153,153,153,1) 0%, rgba(87,87,87,1) 100%)`,
		}"
		:class="[
			fillLayout
				? 'flex-1 pb-6'
				: constrainWidth
					? 'min-h-[100svh] max-w-[1280px] pb-16'
					: 'min-h-[calc(100svh-100px)] pb-6',
		]"
	>
		<div
			v-if="!isOnboarding"
			class="w-full flex flex-col gap-4"
			:class="['server-stagger-item', fillLayout ? 'shrink-0' : '']"
			:style="{ '--si': 0 }"
		>
			<PageHeader :title="serverData?.name || 'Server'">
				<template #leading>
					<ServerIcon
						:image="serverIconImage"
						:config="serverIconConfig"
						class="size-16 !rounded-xl"
					/>
				</template>

				<template #metadata>
					<PageHeaderMetadata>
						<PageHeaderMetadataItem
							v-if="serverData.flows.intro"
							:icon="SettingsIcon"
							class="font-semibold"
						>
							Configuring server...
						</PageHeaderMetadataItem>

						<template v-else>
							<PageHeaderMetadataItem
								v-if="headerPlatform.loader"
								:icon="TagIcon"
								:icon-props="{ tag: headerPlatform.loader, enforceType: 'loader' }"
							>
								{{ formatLoaderLabel(headerPlatform.loader) }} {{ headerPlatform.gameVersion }}
							</PageHeaderMetadataItem>
							<PageHeaderMetadataItem v-if="showServerUptime" :icon="TimerIcon">
								{{ formattedUptime }}
							</PageHeaderMetadataItem>
							<PageHeaderMetadataItem
								v-if="serverProject"
								:to="serverProjectLink"
								class="!text-primary"
							>
								Linked to
								<Avatar :src="serverProject.icon_url" :alt="serverProject.title" size="24px" />
								{{ serverProject.title }}
							</PageHeaderMetadataItem>
						</template>
					</PageHeaderMetadata>
				</template>

				<template #actions>
					<PageHeaderActions>
						<PanelServerActionButton />
						<IconButton
							v-tooltip="'Server settings'"
							size="xl"
							label="Server settings"
							native-type="button"
							@click="openServerSettingsModal()"
						>
							<SettingsIcon />
						</IconButton>
					</PageHeaderActions>
				</template>
			</PageHeader>
		</div>

		<ServerOnboardingPanelPage
			v-if="isOnboarding"
			:class="fillLayout ? 'my-auto' : 'mt-16'"
			:site-url="siteUrl ?? 'https://modrinth.com'"
		/>

		<template v-if="!isOnboarding">
			<div class="server-stagger-item -mb-3" :class="fillLayout ? 'shrink-0' : ''">
				<NavTabs :links="navLinks" replace page-nav data-pyro-navigation :style="{ '--si': 1 }" />
			</div>

			<div
				data-pyro-mount
				class="server-stagger-item w-full flex-1"
				:class="fillLayout ? 'flex flex-col' : 'h-full'"
				:style="{ '--si': 2 }"
			>
				<div
					v-if="isReconnecting"
					data-pyro-server-ws-reconnecting
					class="mb-4 flex w-full flex-row items-center gap-4 rounded-2xl bg-bg-orange p-4 text-sm text-contrast"
				>
					<LoaderCircleIcon class="h-5 w-5 animate-spin" />
					Hang on, we're reconnecting to your server.
				</div>

				<ServerPanelAdmonitions
					class="mb-4 shrink-0"
					@installation-retry="handleInstallationRetry"
				/>
				<slot />
			</div>
		</template>
	</div>
	<div
		v-if="showAdvancedDebugInfo"
		class="relative mx-auto mt-6 box-border w-full min-w-0 max-w-[1280px] px-6"
	>
		<h2 class="m-0 text-lg font-extrabold text-contrast">Server data</h2>
		<pre class="markdown-body w-full overflow-auto rounded-2xl bg-bg-raised p-4 text-sm">{{
			safeStringify(serverData)
		}}</pre>
	</div>
	<Suspense>
		<ServerSettingsModal
			ref="serverSettingsModal"
			:site-url="siteUrl"
			:browse-modpacks="handleBrowseModpacks"
		/>
	</Suspense>
	<ServerInviteLinkModal
		ref="serverInviteLinkModal"
		:server-id="serverId"
		:site-url="siteUrl ?? ''"
	/>
	<ConfirmLeaveModal
		ref="confirmLeaveModal"
		:header="formatMessage(leaveMessages.uploadInProgress)"
		:body="formatMessage(leaveMessages.leavePageBody)"
		admonition-type="critical"
	/>
</template>

<script setup lang="ts">
import { instanceContent } from '@amberite/contracts/engine'
import { ModrinthApiError } from '@modrinth/api-client'
import {
	BoxesIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	HistoryIcon,
	LayoutTemplateIcon,
	LoaderCircleIcon,
	PlayIcon,
	SettingsIcon,
	TimerIcon,
	TransferIcon,
	TriangleAlertIcon,
} from '@modrinth/assets'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import DOMPurify from 'dompurify'
import { computed, nextTick, onBeforeUnmount, onMounted, onUnmounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import Avatar from '#ui/components/base/Avatar.vue'
import { IconButton } from '#ui/components/base/buttons'
import ErrorInformationCard from '#ui/components/base/ErrorInformationCard.vue'
import NavTabs from '#ui/components/base/NavTabs.vue'
import PageHeader from '#ui/components/base/page-header/index.vue'
import PageHeaderMetadata from '#ui/components/base/page-header/metadata/index.vue'
import PageHeaderMetadataItem from '#ui/components/base/page-header/metadata/page-header-metadata-item.vue'
import PageHeaderActions from '#ui/components/base/page-header/page-header-actions.vue'
import TagIcon from '#ui/components/base/TagIcon.vue'
import ConfirmLeaveModal from '#ui/components/modal/ConfirmLeaveModal.vue'
import ServerPanelAdmonitions from '#ui/components/servers/admonitions/ServerPanelAdmonitions.vue'
import ServerIcon from '#ui/components/servers/icons/ServerIcon.vue'
import { injectServerOnboardingFlow } from '#ui/components/servers/onboarding/context'
import { PanelServerActionButton } from '#ui/components/servers/server-header'
import ServerSettingsModal from '#ui/components/servers/ServerSettingsModal.vue'
import ServerInviteLinkModal from '#ui/components/sharing/server-invite-link/ServerInviteLinkModal.vue'
import { injectServerInviteHandoff } from '#ui/components/sharing/server-sharing-update/handoff'
import {
	hasServerPermission,
	useGeneratedIconSrc,
	useLoadingBarToken,
	useReadyState,
	useServerIcon,
	useServerProject,
} from '#ui/composables'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerBackupsQueue } from '#ui/composables/server-backups-queue'
import { useServerManageCoreRuntime } from '#ui/composables/server-manage-core-runtime'
import { serverDetailQueryOptions, serverKeys } from '#ui/composables/server-queries'
import { directoryQueryOptions } from '#ui/layouts/shared/files-tab/providers/instance-files'
import type { ServerSettingsTabId } from '#ui/layouts/shared/server-settings'
import {
	injectModrinthClient,
	injectNotificationManager,
	provideServerSettingsModal,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'
import { formatLoaderLabel } from '#ui/utils/loaders'

import ServerOnboardingPanelPage from './[id]/onboarding.vue'

interface Tab {
	label: string
	href: string
	icon?: object
	subpages?: string[]
}

const props = withDefaults(
	defineProps<{
		serverId: string
		/** The Core that has the server. */
		coreId: string
		reloadPage: () => void
		showAdvancedDebugInfo?: boolean
		showUptime?: boolean
		additionalTabs?: Tab[]
		siteUrl?: string
		navigateToServers?: () => void
		browseModpacks?: (args: {
			serverId: string
			from: 'reset-server' | 'onboarding'
		}) => void | Promise<void>
		browseContent?: (args: {
			serverId: string
			type: 'mod' | 'plugin' | 'datapack'
		}) => void | Promise<void>
		constrainWidth?: boolean
		layoutMode?: 'page' | 'fill'
	}>(),
	{
		showAdvancedDebugInfo: false,
		showUptime: true,
		additionalTabs: () => [],
		siteUrl: undefined,
		navigateToServers: undefined,
		browseModpacks: undefined,
		browseContent: undefined,
		constrainWidth: false,
		layoutMode: 'page',
	},
)

const { formatMessage } = useVIntl()

const leaveMessages = defineMessages({
	uploadInProgress: {
		id: 'servers.manage.confirm-leave.upload-in-progress',
		defaultMessage: 'Upload in progress',
	},
	leavePageBody: {
		id: 'servers.manage.confirm-leave.body',
		defaultMessage: 'A file upload is in progress. Leaving this page will cancel the upload.',
	},
})

const { addNotification } = injectNotificationManager()
const client = injectModrinthClient()
const core = client.core(props.coreId)
const constrainWidth = computed(() => props.constrainWidth)
const fillLayout = computed(() => props.layoutMode === 'fill')
const queryClient = useQueryClient()
const route = useRoute()
const router = useRouter()
const onboardingFlow = injectServerOnboardingFlow(null)

const serverSettingsModal = ref<InstanceType<typeof ServerSettingsModal> | null>(null)
const confirmLeaveModal = ref<InstanceType<typeof ConfirmLeaveModal>>()

const {
	data: serverData,
	error: serverQueryError,
	isLoading: serverLoading,
} = useQuery(serverDetailQueryOptions(client, props.serverId))

useLoadingBarToken(useReadyState({ isLoading: serverLoading, data: serverData }))

const serverError = computed(() => {
	const err = serverQueryError.value
	if (err instanceof ModrinthApiError) return err
	return err ? ModrinthApiError.fromUnknown(err) : null
})

const headerPlatform = computed(() => ({
	loader: serverData.value?.loader?.toLowerCase() ?? null,
	gameVersion: serverData.value?.mc_version ?? null,
}))

const backupsQueue = useServerBackupsQueue(props.serverId, core)
const { handleBackupProgress, busyReasons: backupsBusy } = backupsQueue

const { config: serverIconConfig, image: serverIconImage } = useServerIcon(() => props.serverId, {
	icon: () => serverData.value?.icon,
})
const serverIconGeneratedSrc = useGeneratedIconSrc(serverIconConfig)
const serverImage = computed(() => serverIconGeneratedSrc.value ?? serverIconImage.value)
const { data: serverProject } = useServerProject(computed(() => serverData.value?.upstream ?? null))

const {
	cancelUpload,
	installation,
	beginInstallation,
	cancelOptimisticInstallation,
	dismissInstallation,
	isConnected,
	powerState,
	uptimeSeconds,
	uploadState,
} = useServerManageCoreRuntime({
	serverId: props.serverId,
	coreId: props.coreId,
	server: serverData,
	extraBusyReasons: backupsBusy,
	onBackupProgress: handleBackupProgress,
})

const serverInviteLinkModal = ref<InstanceType<typeof ServerInviteLinkModal>>()
const canSetup = computed(() => hasServerPermission(0, 'SETUP'))

// A server made to invite players to opens the invite link once it loads.
const inviteHandoff = injectServerInviteHandoff(null)
watch(
	() => !!inviteHandoff?.awaitingServer.value && !!serverData.value,
	(ready) => {
		if (!ready || !inviteHandoff) return
		inviteHandoff.awaitingServer.value = false
		void nextTick(() => serverInviteLinkModal.value?.show())
	},
	{ immediate: true },
)

const globalInviteForServer = computed(
	() =>
		onboardingFlow?.inviteActive.value && onboardingFlow.request.value?.serverId === props.serverId,
)
// A server with nothing installed is not set up yet: the panel opens on setting it up.
const isOnboarding = computed(
	() => !globalInviteForServer.value && !!serverData.value?.flows.intro && !installation.value,
)

/** The socket was open and dropped; it reopens by itself. */
const everConnected = ref(isConnected.value)
watch(isConnected, (connected) => {
	if (connected) everConnected.value = true
})
const isReconnecting = computed(() => everConnected.value && !isConnected.value)

const showServerUptime = computed(() => props.showUptime && powerState.value === 'running')

const formattedUptime = computed(() => formatUptime(uptimeSeconds.value))

const serverProjectLink = computed(() => {
	if (!serverProject.value) return ''
	return `/project/${serverProject.value.slug ?? serverProject.value.id}`
})

function formatUptime(uptime: number) {
	const days = Math.floor(uptime / (24 * 3600))
	const hours = Math.floor((uptime % (24 * 3600)) / 3600)
	const minutes = Math.floor((uptime % 3600) / 60)
	const seconds = uptime % 60

	let formatted = ''
	if (days > 0) formatted += `${days}d `
	if (hours > 0 || days > 0) formatted += `${hours}h `
	formatted += `${minutes}m ${seconds}s`
	return formatted.trim()
}

const isUploading = computed(() => uploadState.value.isUploading)

function handleBeforeUnload(e: BeforeUnloadEvent) {
	if (isUploading.value) {
		e.preventDefault()
		return ''
	}
}

if (typeof window !== 'undefined') {
	watch(isUploading, (uploading) => {
		if (uploading) {
			window.addEventListener('beforeunload', handleBeforeUnload)
		} else {
			window.removeEventListener('beforeunload', handleBeforeUnload)
		}
	})

	onBeforeUnmount(() => {
		window.removeEventListener('beforeunload', handleBeforeUnload)
	})

	onBeforeRouteLeave(async () => {
		if (isUploading.value) {
			const shouldLeave = (await confirmLeaveModal.value?.prompt()) ?? false
			if (shouldLeave) cancelUpload.value?.()
			return shouldLeave
		}
		return true
	})
}

// Content and backups are already loaded by this layout; these two tabs fetch on their own, so
// hovering their link warms the first request.
function prefetchFiles() {
	void queryClient.prefetchQuery(
		directoryQueryOptions(
			{
				client: core,
				instanceId: props.serverId,
				queryKey: (path) => serverKeys.directory(props.serverId, path),
			},
			'',
		),
	)
}

const navLinks = computed<Tab[]>(() => [
	{
		label: 'Overview',
		href: `/hosting/manage/${props.serverId}`,
		icon: LayoutTemplateIcon,
		subpages: [],
	},
	{
		label: 'Play',
		href: `/hosting/manage/${props.serverId}/play`,
		icon: PlayIcon,
		subpages: [],
	},
	{
		label: 'Content',
		href: `/hosting/manage/${props.serverId}/content`,
		icon: BoxesIcon,
		subpages: ['mods', 'datapacks'],
	},
	{
		label: 'Files',
		href: `/hosting/manage/${props.serverId}/files`,
		icon: FolderOpenIcon,
		subpages: [],
		onHover: prefetchFiles,
	},
	{
		label: 'Backups',
		href: `/hosting/manage/${props.serverId}/backups`,
		icon: DatabaseBackupIcon,
		subpages: [],
	},
	{
		label: 'Activity',
		href: `/hosting/manage/${props.serverId}/access`,
		icon: HistoryIcon,
		subpages: [],
	},
	...props.additionalTabs,
])

/** Installs what the server follows again, after a failed install. */
async function handleInstallationRetry() {
	if (!canSetup.value) {
		addNotification({ type: 'error', text: formatMessage(commonMessages.noPermissionAction) })
		return
	}
	const failed = installation.value?.status === 'failed' ? installation.value.id : null
	if (failed) dismissInstallation(failed)
	beginInstallation({ type: 'unknown' })
	try {
		await core.call(instanceContent.RepairContent, { params: { id: props.serverId } })
	} catch (err) {
		addNotification({
			type: 'error',
			text: err instanceof Error ? err.message : 'Failed to retry installation',
		})
	} finally {
		// A repair answers once done and runs no install job to end what began here.
		cancelOptimisticInstallation()
	}
}

// An install that ends changes what the server runs.
watch(
	() => installation.value?.status,
	(status, before) => {
		if (before && status !== before && (status === undefined || status === 'failed')) {
			void queryClient.invalidateQueries({ queryKey: serverKeys.detail(props.serverId) })
			void queryClient.invalidateQueries({ queryKey: serverKeys.startup(props.serverId) })
		}
	},
)

const generalErrorDetails = computed(() => [
	{ label: 'Server ID', value: props.serverId, type: 'inline' as const },
	{ label: 'Timestamp', value: String(new Date().toISOString()), type: 'inline' as const },
	{ label: 'Error Name', value: serverError.value?.name, type: 'inline' as const },
	{ label: 'Error Message', value: serverError.value?.message, type: 'block' as const },
	...(serverError.value?.stack
		? [{ label: 'Stack Trace', value: serverError.value.stack, type: 'hidden' as const }]
		: []),
])

const nodeUnavailableDetails = computed(() => [
	{ label: 'Server ID', value: props.serverId, type: 'inline' as const },
	{ label: 'Core', value: props.coreId, type: 'inline' as const },
	{
		label: 'Error message',
		value: serverError.value?.message ?? 'Unknown',
		type: 'block' as const,
	},
])

const generalErrorAction = computed(() => ({
	label: 'Go back to all servers',
	onClick: () => props.navigateToServers?.(),
	color: 'brand' as const,
}))

const nodeUnavailableAction = computed(() => ({
	label: 'Try again',
	onClick: () => props.reloadPage(),
	color: 'brand' as const,
	disabled: false,
}))

function openServerSettingsModal(tabId?: ServerSettingsTabId) {
	if (!props.serverId) return
	serverSettingsModal.value?.show({ serverId: props.serverId, tabId })
}

function handleBrowseModpacks(args: { serverId: string; from: 'reset-server' | 'onboarding' }) {
	props.browseModpacks?.(args)
}

provideServerSettingsModal({
	openServerSettings: (options) => openServerSettingsModal(options?.tabId),
	browseServerContent: (args) => props.browseContent?.(args),
})

function safeStringify(obj: unknown): string {
	return JSON.stringify(obj, null, ' ')
}

onMounted(() => {
	DOMPurify.addHook(
		'afterSanitizeAttributes',
		(node: {
			tagName: string
			getAttribute: (arg0: string) => string | null
			setAttribute: (arg0: string, arg1: string) => void
		}) => {
			if (node.tagName === 'A' && node.getAttribute('target')) {
				node.setAttribute('rel', 'noopener noreferrer')
			}
		},
	)

	if (route.query.openSettings) {
		const tabId = route.query.openSettings as ServerSettingsTabId
		router.replace({ query: { ...route.query, openSettings: undefined } })
		queryClient.invalidateQueries({ queryKey: serverKeys.detail(props.serverId) })
		queryClient.invalidateQueries({ queryKey: serverKeys.content(props.serverId) })
		queryClient.invalidateQueries({ queryKey: serverKeys.startup(props.serverId) })
		nextTick(() => openServerSettingsModal(tabId))
	}
})

onUnmounted(() => {
	DOMPurify.removeHook('afterSanitizeAttributes')
})
</script>

<style>
@keyframes server-action-buttons-anim {
	0% {
		opacity: 0;
		transform: translateX(1rem);
	}

	100% {
		opacity: 1;
		transform: none;
	}
}

.server-action-buttons-anim {
	animation: server-action-buttons-anim 0.2s ease-out;
}
</style>
