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
		v-else-if="serverError || !nodeAccessible"
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
			'server-panel-' + revealState,
			fillLayout
				? 'flex-1 pb-6'
				: constrainWidth
					? 'min-h-[100svh] max-w-[1280px] pb-16'
					: 'min-h-[calc(100svh-100px)] pb-6',
		]"
	>
		<template v-if="revealState !== 'pending' || isOnboarding || globalInviteForServer">
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
								v-if="serverData.flows?.intro"
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
								<PageHeaderMetadataItem
									v-if="serverData.net?.domain && !serverPreferences.hideSubdomainLabel"
									:icon="LinkIcon"
									tooltip="Copy server address"
									:action="copyServerAddress"
								>
									{{ serverData.net.domain }}.modrinth.gg
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
						v-if="!isConnected && !isReconnecting && !isLoading"
						data-pyro-server-ws-error
						class="mb-4 flex w-full flex-row items-center gap-4 rounded-2xl bg-bg-red p-4 text-contrast"
					>
						<IssuesIcon class="size-5 text-red" />
						Something went wrong...
					</div>

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
						:curseforge-file-warning-count="serverContent?.files_need_side_type_review ?? 0"
						@installation-retry="handleInstallationRetry"
					/>
					<slot :on-reinstall="onReinstall" :on-reinstall-failed="onReinstallFailed" />
				</div>
			</template>
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
			:resolve-viewer="resolveViewerOnce"
			:browse-modpacks="handleBrowseModpacks"
		/>
	</Suspense>
	<ServerPlayMigrationModal
		ref="serverPlayMigrationModal"
		:kind="serverPlayMigration.kind.value"
		:modpack-name="serverPlayMigration.modpackName.value"
		:has-uploaded-mrpack="serverPlayMigration.hasUploadedMrpack.value"
		:world-id="worldId"
		:disabled="!canSetup || serverPlayMigration.active.value"
		:upload-progress="serverPlayMigration.uploadProgress.value"
		@confirm="confirmServerPlayMigration"
	/>
	<ServerInviteLinkModal
		ref="serverInviteLinkModal"
		:server-id="serverId"
		:world-id="worldId"
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
import type { Archon } from '@modrinth/api-client'
import { ModrinthApiError } from '@modrinth/api-client'
import {
	BoxesIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	HistoryIcon,
	IssuesIcon,
	LayoutTemplateIcon,
	LinkIcon,
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
import ServerPlayMigrationModal from '#ui/components/servers/play-migration/ServerPlayMigrationModal.vue'
import { PanelServerActionButton } from '#ui/components/servers/server-header'
import ServerSettingsModal from '#ui/components/servers/ServerSettingsModal.vue'
import ServerInviteLinkModal from '#ui/components/sharing/server-invite-link/ServerInviteLinkModal.vue'
import {
	hasServerPermission,
	useDebugLogger,
	useFormatDateTime,
	useGeneratedIconSrc,
	useLoadingBarToken,
	useModrinthServersConsole,
	useReadyState,
	useServerIcon,
	useServerProject,
} from '#ui/composables'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerBackupsQueue } from '#ui/composables/server-backups-queue'
import type {
	ServerInstallationKey,
	ServerInstallationState,
} from '#ui/composables/server-installation-tracker'
import { useServerManageCoreRuntime } from '#ui/composables/server-manage-core-runtime'
import { useServerPanelSync } from '#ui/composables/server-panel-sync'
import { useServerPreferences } from '#ui/composables/server-preferences'
import { serverKeys } from '#ui/composables/server-queries'
import type { LogLine } from '#ui/layouts/shared/console'
import type { ServerSettingsTabId } from '#ui/layouts/shared/server-settings'
import {
	injectModrinthClient,
	injectNotificationManager,
	provideServerSettingsModal,
} from '#ui/providers'
import type { ServerStats } from '#ui/providers/server-context'
import { commonMessages } from '#ui/utils/common-messages'
import { formatLoaderLabel, resolveServerPlatform } from '#ui/utils/loaders'

import ServerOnboardingPanelPage from './[id]/onboarding.vue'
import { useServerPlayMigrationFlow } from './composables/use-server-play-migration-flow'

interface Tab {
	label: string
	href: string
	icon?: object
	subpages?: string[]
}

const props = withDefaults(
	defineProps<{
		serverId: string
		reloadPage: () => void
		resolveViewer: () => Promise<{ userId: string | null; userRole: string | null }>
		showAdvancedDebugInfo?: boolean
		showUptime?: boolean
		additionalTabs?: Tab[]
		siteUrl?: string
		navigateToServers?: () => void
		browseModpacks?: (args: {
			serverId: string
			worldId: string | null
			from: 'reset-server' | 'onboarding'
		}) => void | Promise<void>
		browseContent?: (args: {
			serverId: string
			worldId: string | null
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

// disabled, keeping the animation logic cos it's really nice and we might want to re-enable in future
const DISABLE_LOADING_ANIM = true

const { addNotification } = injectNotificationManager()
const client = injectModrinthClient()
const constrainWidth = computed(() => props.constrainWidth)
const fillLayout = computed(() => props.layoutMode === 'fill')
const queryClient = useQueryClient()
const route = useRoute()
const router = useRouter()
const onboardingFlow = injectServerOnboardingFlow(null)
const debug = useDebugLogger('ServerManage')

const isReconnecting = ref(false)
const isLoading = ref(true)
const isMounted = ref(true)

const serverPreferences = useServerPreferences(props.serverId)

const serverSettingsModal = ref<InstanceType<typeof ServerSettingsModal> | null>(null)
let viewerPromise: ReturnType<typeof props.resolveViewer> | null = null

function resolveViewerOnce() {
	viewerPromise ??= props.resolveViewer().catch((error) => {
		viewerPromise = null
		throw error
	})
	return viewerPromise
}
const confirmLeaveModal = ref<InstanceType<typeof ConfirmLeaveModal>>()
const formatDateTime = useFormatDateTime({ dateStyle: 'long', timeStyle: 'short' })

// Matches the tab pages and the hover prefetches, so opening a server or switching tabs reuses
// what was just fetched. Changes arrive over the websocket and invalidate these queries.
const PANEL_STALE_MS = 30_000

const {
	data: serverData,
	error: serverQueryError,
	isLoading: serverLoading,
} = useQuery({
	queryKey: serverKeys.detail(props.serverId),
	queryFn: () => client.archon.servers_v0.get(props.serverId)!,
	// A Core that does not answer is offline: say so at once instead of retrying for half a minute.
	retry: false,
	// The server list already holds this server, so the panel shows at once and the full answer
	// replaces it when it lands.
	placeholderData: () =>
		queryClient
			.getQueryData<Archon.Servers.v0.ServerGetResponse>(['servers'])
			?.servers.find((server) => server.server_id === props.serverId),
	staleTime: PANEL_STALE_MS,
})

useLoadingBarToken(useReadyState({ isLoading: serverLoading, data: serverData }))

function updateServerData(patch: Partial<Archon.Servers.v0.Server>) {
	if (!serverData.value) return
	queryClient.setQueryData(serverKeys.detail(props.serverId), {
		...serverData.value,
		...patch,
	})
}

const serverError = computed(() => {
	const err = serverQueryError.value
	if (err instanceof ModrinthApiError) return err
	return err ? ModrinthApiError.fromUnknown(err) : null
})

const { data: serverFull } = useQuery({
	queryKey: serverKeys.detailV1(props.serverId),
	queryFn: () => client.archon.servers_v1.get(props.serverId),
	staleTime: PANEL_STALE_MS,
})

const worldId = computed(() => {
	if (!serverFull.value) return null
	const activeWorld = serverFull.value.worlds.find((w) => w.is_active)
	return activeWorld?.id ?? serverFull.value.worlds[0]?.id ?? null
})

const { data: serverContent } = useQuery({
	queryKey: serverKeys.content(props.serverId),
	queryFn: () =>
		client.archon.content_v1.getAddons(props.serverId, worldId.value!, { from_modpack: false }),
	enabled: computed(() => worldId.value !== null),
	staleTime: PANEL_STALE_MS,
})

const headerPlatform = computed(() => resolveServerPlatform(serverData.value, serverContent.value))

const backupsQueue = useServerBackupsQueue(
	computed(() => props.serverId),
	worldId,
)
const { handleWsBackupProgress, busyReasons: backupsBusy } = backupsQueue

const { disconnect: disconnectPanelSync } = useServerPanelSync({
	serverId: computed(() => props.serverId),
	worldId,
})

const { config: serverIconConfig, image: serverIconImage } = useServerIcon(() => props.serverId, {
	icon: () => serverData.value?.icon,
})
const serverIconGeneratedSrc = useGeneratedIconSrc(serverIconConfig)
const serverImage = computed(() => serverIconGeneratedSrc.value ?? serverIconImage.value)
const { data: serverProject } = useServerProject(computed(() => serverData.value?.upstream ?? null))

const onStateEvent = (data: Archon.Websocket.v0.WSStateEvent) => {
	debug('[root.vue] handleState received:', {
		power_variant: data.power_variant,
		serverStatus: serverData.value?.status,
	})
	hasReceivedWsData.value = true
}

const {
	beginInstallation,
	cancelUpload,
	cancelOptimisticInstallation,
	cleanupCoreRuntime,
	connectSocket,
	cpuData,
	dismissInstallation,
	fsOps,
	fsQueuedOps,
	installation,
	isConnected,
	ramData,
	serverPowerState,
	stats,
	uptimeSeconds,
	uploadState,
} = useServerManageCoreRuntime({
	serverId: computed(() => props.serverId),
	worldId,
	server: serverData,
	serverFull,
	content: serverContent,
	extraBusyReasons: backupsBusy,
	setDisconnectedOnAuthIncorrect: false,
	syncUptimeFromState: true,
	incrementUptimeLocally: true,
	eventGuard: () => isMounted.value,
	onStateEvent,
})

const serverPlayMigrationModal = ref<InstanceType<typeof ServerPlayMigrationModal>>()
const serverInviteLinkModal = ref<InstanceType<typeof ServerInviteLinkModal>>()
const canSetup = computed(() =>
	hasServerPermission(serverData.value?.current_user_permissions ?? 0, 'SETUP'),
)
const { migration: serverPlayMigration, confirmMigration: confirmServerPlayMigration } =
	useServerPlayMigrationFlow({
		serverId: props.serverId,
		worldId,
		serverFull,
		content: serverContent,
		backupsQueue,
		canSetup,
		serverLoaded: computed(() => !!serverData.value),
		migrationModal: serverPlayMigrationModal,
		inviteLinkModal: serverInviteLinkModal,
	})

const globalInviteForServer = computed(
	() =>
		onboardingFlow?.inviteActive.value && onboardingFlow.request.value?.serverId === props.serverId,
)
const isOnboarding = computed(
	() => !globalInviteForServer.value && !!serverData.value?.flows?.intro && !installation.value,
)

const showServerUptime = computed(() => props.showUptime && serverPowerState.value === 'running')

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

function copyServerAddress() {
	const domain = serverData.value?.net?.domain
	if (!domain) return

	void navigator.clipboard.writeText(`${domain}.modrinth.gg`)
	addNotification({
		title: 'Server address copied',
		text: "Your server's address has been copied to your clipboard.",
		type: 'success',
	})
}

const isUploading = computed(() => uploadState.value.isUploading)
const permissionDeniedMessage = computed(() => formatMessage(commonMessages.noPermissionAction))

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

type CachedWsState = {
	stats: ServerStats
	cpuData: number[]
	ramData: number[]
	powerState: Archon.Websocket.v0.PowerState
	uptimeSeconds: number
	consoleLines: LogLine[]
}

const modrinthServersConsole = useModrinthServersConsole()
const wsStateCacheKey = ['servers', 'ws-state', props.serverId] as const
const cachedWsState = queryClient.getQueryData<CachedWsState>(wsStateCacheKey)
if (cachedWsState) {
	stats.value = cachedWsState.stats
	cpuData.value = cachedWsState.cpuData
	ramData.value = cachedWsState.ramData
	serverPowerState.value = cachedWsState.powerState
	uptimeSeconds.value = cachedWsState.uptimeSeconds
}

const log = useDebugLogger('server-panel-reveal')

const hasReceivedWsData = ref(!!cachedWsState)
log('init', {
	hasCachedWsState: !!cachedWsState,
	hasReceivedWsData: hasReceivedWsData.value,
	isConnected: isConnected.value,
	serverData: !!serverData.value,
})

const saveWsStateToCache = () => {
	if (!hasReceivedWsData.value) return
	queryClient.setQueryData(wsStateCacheKey, {
		stats: stats.value,
		cpuData: cpuData.value,
		ramData: ramData.value,
		powerState: serverPowerState.value,
		uptimeSeconds: uptimeSeconds.value,
		consoleLines: modrinthServersConsole.output.value,
	} satisfies CachedWsState)
}

watch([stats, serverPowerState], () => {
	if (!isConnected.value) return
	hasReceivedWsData.value = true
})

const canReveal = computed(() => serverData.value && hasReceivedWsData.value)
log('canReveal initial', {
	canReveal: canReveal.value,
	serverData: !!serverData.value,
	hasReceivedWsData: hasReceivedWsData.value,
})

const revealState = ref<'pending' | 'revealing' | 'visible'>(
	DISABLE_LOADING_ANIM || canReveal.value ? 'visible' : 'pending',
)
log('revealState initial', revealState.value)

const REVEAL_TOTAL_MS = 2 * 80 + 400

watch(canReveal, (ready) => {
	log('canReveal changed', { ready, revealState: revealState.value })
	if (ready && revealState.value === 'pending') {
		if (DISABLE_LOADING_ANIM) {
			revealState.value = 'visible'
		} else {
			revealState.value = 'revealing'
			setTimeout(() => {
				revealState.value = 'visible'
				log('revealState -> visible')
			}, REVEAL_TOTAL_MS)
		}
	}
})

watch(isConnected, (connected) => {
	log('isConnected changed', connected)
})

watch(serverData, (data) => {
	log('serverData changed', !!data)
})

// Content and backups are already loaded by this layout; these two tabs fetch on their own, so
// hovering their link warms the first request.
function prefetchFiles() {
	void queryClient.prefetchQuery({
		queryKey: ['files', props.serverId, '/'],
		queryFn: () => client.kyros.files_v0.listDirectory('/', 1, 2000),
		staleTime: PANEL_STALE_MS,
	})
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

async function handleInstallationRetry() {
	if (!worldId.value) return
	if (!canSetup.value) {
		addNotification({
			type: 'error',
			text: permissionDeniedMessage.value,
		})
		return
	}
	const failedInstallationId =
		installation.value?.status === 'failed' ? installation.value.id : null
	if (failedInstallationId) dismissInstallation(failedInstallationId)
	beginInstallation({ type: 'unknown' })
	updateServerData({ status: 'installing' })
	try {
		await client.archon.content_v1.repair(props.serverId, worldId.value)
	} catch (err) {
		cancelOptimisticInstallation()
		updateServerData({ status: 'available' })
		addNotification({
			type: 'error',
			text: err instanceof Error ? err.message : 'Failed to retry installation',
		})
	}
}

const handleBackupProgress = (data: Archon.Websocket.v0.WSBackupProgressEvent) => {
	handleWsBackupProgress(data)
}

const handleFilesystemOps = (data: Archon.Websocket.v0.WSFilesystemOpsEvent) => {
	const allOps = data.all

	if (JSON.stringify(fsOps.value) !== JSON.stringify(allOps)) {
		fsOps.value = allOps
	}

	fsQueuedOps.value = fsQueuedOps.value.filter(
		(queuedOp) => !allOps.some((x) => x.src === queuedOp.src),
	)

	const cancelled = allOps.filter((x) => x.state === 'cancelled')
	Promise.all(
		cancelled.map((x) =>
			client.kyros.files_v0.modifyOperation(x.id, 'dismiss').catch((error) => {
				console.error('Failed to dismiss cancelled operation:', error)
			}),
		),
	)
}

type InstallationServerSnapshot = Pick<
	Archon.Servers.v0.Server,
	'loader' | 'loader_version' | 'mc_version'
>

let installationServerSnapshot: InstallationServerSnapshot | null = null

function applyInstallationTarget(current: ServerInstallationState) {
	if (!serverData.value) return

	if (!installationServerSnapshot) {
		installationServerSnapshot = {
			loader: serverData.value.loader,
			loader_version: serverData.value.loader_version,
			mc_version: serverData.value.mc_version,
		}
	}

	const patch: Partial<Archon.Servers.v0.Server> = { status: 'installing' }
	if (current.key.type === 'platform') {
		patch.loader = formatLoaderLabel(current.key.platform) as Archon.Servers.v0.Loader
		patch.loader_version = current.key.platform === 'vanilla' ? null : current.key.platform_version
		patch.mc_version = current.key.game_version
	}

	if (
		serverData.value.status === patch.status &&
		(current.key.type !== 'platform' ||
			(serverData.value.loader === patch.loader &&
				serverData.value.loader_version === patch.loader_version &&
				serverData.value.mc_version === patch.mc_version))
	) {
		return
	}

	void queryClient.cancelQueries({
		queryKey: serverKeys.detail(props.serverId),
		exact: true,
	})
	updateServerData(patch)
}

function restoreInstallationServerSnapshot() {
	const snapshot = installationServerSnapshot
	updateServerData({
		...(snapshot ?? {}),
		status: 'available',
	})
	installationServerSnapshot = null
}

const onReinstall = async (
	potentialArgs: { loader?: string; lVersion?: string; mVersion?: string } | undefined,
) => {
	debug('[root.vue] onReinstall called with:', potentialArgs)

	if (serverData.value?.flows?.intro) {
		await client.archon.servers_v1.endIntro(props.serverId)
		queryClient.invalidateQueries({ queryKey: serverKeys.detail(props.serverId) })
	}

	if (!serverData.value) return

	if (
		!installation.value ||
		installation.value.status === 'complete' ||
		installation.value.status === 'failed'
	) {
		if (potentialArgs?.loader && potentialArgs.mVersion) {
			beginInstallation({
				type: 'platform',
				platform: potentialArgs.loader as Extract<
					Archon.Websocket.v0.InstallProgressKey,
					{ type: 'platform' }
				>['platform'],
				platform_version: potentialArgs.lVersion ?? '',
				game_version: potentialArgs.mVersion,
			})
		} else {
			beginInstallation({ type: 'unknown' })
		}
	}

	modrinthServersConsole.clear()
}

const onReinstallFailed = () => {
	debug('[root.vue] onReinstallFailed: reverting status to available')
	cancelOptimisticInstallation()
	restoreInstallationServerSnapshot()
}

function applyInstallationCompletion(key: ServerInstallationKey) {
	const platformKey = key?.type === 'platform' ? key : null
	const patch: Partial<Archon.Servers.v0.Server> = { status: 'available' }
	if (platformKey) {
		patch.loader = formatLoaderLabel(platformKey.platform) as Archon.Servers.v0.Loader
		patch.loader_version = platformKey.platform === 'vanilla' ? null : platformKey.platform_version
		patch.mc_version = platformKey.game_version
	} else if (serverContent.value?.modloader && serverContent.value.game_version) {
		patch.loader = formatLoaderLabel(serverContent.value.modloader) as Archon.Servers.v0.Loader
		patch.mc_version = serverContent.value.game_version
	}

	debug('[root.vue] applyInstallationCompletion: patch:', patch)
	updateServerData(patch)

	const addonsQueries = queryClient.getQueriesData<Archon.Content.v1.Addons>({
		queryKey: serverKeys.content(props.serverId),
	})
	for (const [key, data] of addonsQueries) {
		if (!data || !platformKey) continue
		queryClient.setQueryData(key, {
			...data,
			modloader: platformKey.platform === 'neoforge' ? 'neo_forge' : platformKey.platform,
			modloader_version: platformKey.platform === 'vanilla' ? null : platformKey.platform_version,
			game_version: platformKey.game_version,
		})
	}
}

async function invalidateAfterInstall() {
	debug('[root.vue] invalidateAfterInstall: scheduling 2s delayed invalidation')
	setTimeout(async () => {
		try {
			await Promise.all([
				queryClient.invalidateQueries({ queryKey: serverKeys.detail(props.serverId) }),
				queryClient.invalidateQueries({
					queryKey: serverKeys.startup(props.serverId),
				}),
			])
		} catch (err: unknown) {
			console.error('Error refreshing data after installation:', err)
		}
	}, 2000)
}

let handledFailedInstallationId: string | null = null
watch(
	installation,
	(current, previous) => {
		if (!current) {
			if (
				isMounted.value &&
				previous?.source === 'optimistic' &&
				previous.status === 'pending' &&
				serverData.value?.status === 'installing'
			) {
				restoreInstallationServerSnapshot()
			}
			return
		}
		if (current.status === 'pending' || current.status === 'installing') {
			handledFailedInstallationId = null
			applyInstallationTarget(current)
			return
		}

		if (current.status === 'failed') {
			if (handledFailedInstallationId === current.id) return
			handledFailedInstallationId = current.id
			if (current.source === 'server') return
			onReinstallFailed()
			void invalidateAfterInstall()
			return
		}

		dismissInstallation(current.id)
		installationServerSnapshot = null
		applyInstallationCompletion(current.key)
		void invalidateAfterInstall()
	},
	{ flush: 'sync' },
)

const nodeAccessible = ref(true)

const nodeUnavailableDetails = computed(() => [
	{
		label: 'Server ID',
		value: props.serverId,
		type: 'inline' as const,
	},
	{
		label: 'Node',
		value:
			(serverError.value?.responseData as { hostname?: string } | undefined)?.hostname ??
			serverData.value?.datacenter ??
			'Unknown',
		type: 'inline' as const,
	},
	{
		label: 'Error message',
		value: nodeAccessible.value
			? (serverError.value?.message ?? 'Unknown')
			: 'Unable to establish the node WebSocket connection.',
		type: 'block' as const,
	},
])

const generalErrorDetails = computed(() => [
	{
		label: 'Server ID',
		value: props.serverId,
		type: 'inline' as const,
	},
	{
		label: 'Timestamp',
		value: String(new Date().toISOString()),
		type: 'inline' as const,
	},
	{
		label: 'Error Name',
		value: serverError.value?.name,
		type: 'inline' as const,
	},
	{
		label: 'Error Message',
		value: serverError.value?.message,
		type: 'block' as const,
	},
	...(serverError.value?.originalError
		? [
				{
					label: 'Original Error',
					value: String(serverError.value.originalError),
					type: 'hidden' as const,
				},
			]
		: []),
	...(serverError.value?.stack
		? [
				{
					label: 'Stack Trace',
					value: serverError.value.stack,
					type: 'hidden' as const,
				},
			]
		: []),
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

function handleBrowseModpacks(args: {
	serverId: string
	worldId: string | null
	from: 'reset-server' | 'onboarding'
}) {
	props.browseModpacks?.(args)
}

function handleBrowseContent(args: {
	serverId: string
	worldId: string | null
	type: 'mod' | 'plugin' | 'datapack'
}) {
	props.browseContent?.(args)
}

provideServerSettingsModal({
	openServerSettings: (options) => openServerSettingsModal(options?.tabId),
	browseServerContent: (args) => handleBrowseContent(args),
})

function safeStringify(obj: unknown, indent = ' '): string {
	const seen = new WeakSet()
	return JSON.stringify(
		obj,
		(_key, value) => {
			if (typeof value === 'object' && value !== null) {
				if (seen.has(value)) {
					return '[Circular]'
				}
				seen.add(value)
			}
			return value
		},
		indent,
	)
}

function initializeServer() {
	if (serverError.value) {
		isLoading.value = false
	} else {
		void connectSocket(props.serverId, {
			extraSubscriptions: (targetServerId) => [
				client.archon.sockets.on(targetServerId, 'backup-progress', handleBackupProgress),
				client.archon.sockets.on(targetServerId, 'filesystem-ops', handleFilesystemOps),
				// A Core install that fails before reporting any progress is only visible as the
				// content's `error`, so the content is fetched again to show it.
				client.archon.sockets.on(targetServerId, 'installation-result', () =>
					queryClient.invalidateQueries({ queryKey: serverKeys.content(props.serverId) }),
				),
			],
		})
			.then((connected) => {
				nodeAccessible.value = connected
				if (connected && cachedWsState?.consoleLines?.length) {
					modrinthServersConsole.clear()
					modrinthServersConsole.addLines(cachedWsState.consoleLines)
				}
			})
			.finally(() => {
				isLoading.value = false
			})
	}

	if (serverData.value?.flows?.intro && serverProject.value) {
		client.archon.servers_v1.endIntro(props.serverId).then(() => {
			queryClient.invalidateQueries({ queryKey: serverKeys.detail(props.serverId) })
		})
	}
}

const cleanup = () => {
	isMounted.value = false

	saveWsStateToCache()

	cleanupCoreRuntime(props.serverId)
	disconnectPanelSync()

	isReconnecting.value = false
	isLoading.value = true

	DOMPurify.removeHook('afterSanitizeAttributes')
}

onMounted(() => {
	isMounted.value = true

	if (serverData.value) {
		initializeServer()
	} else {
		const stopWatch = watch(serverData, (data) => {
			if (data) {
				stopWatch()
				initializeServer()
			}
		})
	}

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
	cleanup()
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

.server-panel-pending .server-stagger-item {
	opacity: 0;
}

.server-panel-revealing .server-stagger-item {
	animation: serverReveal 0.4s ease-out both;
	animation-delay: calc(var(--si) * 80ms);
}

@keyframes serverReveal {
	from {
		opacity: 0;
		transform: translateY(12px);
	}
	to {
		opacity: 1;
		transform: translateY(0);
	}
}
</style>
