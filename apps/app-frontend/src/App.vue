<script setup>
import {
	AuthFeature,
	ModrinthApiError,
	NodeAuthFeature,
	nodeAuthState,
	PanelVersionFeature,
	TauriModrinthClient,
	VerboseLoggingFeature,
} from '@modrinth/api-client'
import {
	ArrowBigUpDashIcon,
	ChangeSkinIcon,
	CompassIcon,
	ExternalIcon,
	HomeIcon,
	LeftArrowIcon,
	LibraryIcon,
	LogOutIcon,
	NotepadTextIcon,
	PlusIcon,
	RefreshCwIcon,
	RightArrowIcon,
	ServerStackIcon,
	SettingsIcon,
	ShieldAlertIcon,
	UserIcon,
	WorldIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Admonition,
	Avatar,
	ButtonStyled,
	commonMessages,
	ContentInstallModal,
	ContentUpdaterModal,
	defineMessages,
	I18nDebugPanel,
	LoadingBar,
	NewModal,
	NotificationPanel,
	OverflowMenu,
	PopupNotificationPanel,
	provideModalBehavior,
	provideModrinthClient,
	provideNotificationManager,
	providePageContext,
	providePopupNotificationManager,
	useDebugLogger,
	useFormatBytes,
	useHostingIntercom,
	useVIntl,
} from '@modrinth/ui'
import { renderString } from '@modrinth/utils'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'
import { type } from '@tauri-apps/plugin-os'
import { saveWindowState, StateFlags } from '@tauri-apps/plugin-window-state'
import { $fetch } from 'ofetch'
import { computed, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'

import ModrinthAppLogo from '@/assets/modrinth_app.svg?component'
import AccountsCard from '@/components/ui/AccountsCard.vue'
import AppActionBar from '@/components/ui/AppActionBar.vue'
import Breadcrumbs from '@/components/ui/Breadcrumbs.vue'
import ErrorModal from '@/components/ui/ErrorModal.vue'
import InstanceCreationFlowModal from '@/components/ui/creation-flow/InstanceCreationFlowModal.vue'
import FriendsList from '@/components/ui/friends/FriendsList.vue'
import AddServerToInstanceModal from '@/components/ui/install_flow/AddServerToInstanceModal.vue'
import UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import MinecraftAuthErrorModal from '@/components/ui/minecraft-auth-error-modal/MinecraftAuthErrorModal.vue'
import AppSettingsModal from '@/components/ui/modal/AppSettingsModal.vue'
import AuthGrantFlowWaitModal from '@/components/ui/modal/AuthGrantFlowWaitModal.vue'
import InstallToPlayModal from '@/components/ui/modal/InstallToPlayModal.vue'
import ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import UpdateToPlayModal from '@/components/ui/modal/UpdateToPlayModal.vue'
import NavButton from '@/components/ui/NavButton.vue'
import PrideFundraiserBanner from '@/components/ui/PrideFundraiserBanner.vue'
import PromotionWrapper from '@/components/ui/PromotionWrapper.vue'
import QuickInstanceSwitcher from '@/components/ui/QuickInstanceSwitcher.vue'
import SplashScreen from '@/components/ui/SplashScreen.vue'
import WindowControls from '@/components/ui/WindowControls.vue'
import { useCheckDisableMouseover } from '@/composables/macCssFix.js'
import { useAmberiteAuth } from '@/composables/useAmberiteAuth'
import { useModrinthLink } from '@/composables/useModrinthLink'
import { config } from '@/config'
import { hide_ads_window, init_ads_window, show_ads_window } from '@/helpers/ads.js'
import { debugAnalytics, initAnalytics, trackEvent } from '@/helpers/analytics'
import { check_reachable } from '@/helpers/auth.js'
import { get_user, get_version } from '@/helpers/cache.js'
import { command_listener, notification_listener, warning_listener } from '@/helpers/events.js'
import { cancelLogin } from '@/helpers/mr_auth.ts'
import { create_profile_and_install_from_file } from '@/helpers/pack'
import { list } from '@/helpers/profile.js'
import { mergeUrlQuery, parseModrinthLink } from '@/helpers/project-links.ts'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { get_opening_command, initialize_state } from '@/helpers/state'
import {
	areUpdatesEnabled,
	enqueueUpdateForInstallation,
	getOS,
	getUpdateSize,
	isDev,
	isNetworkMetered,
	setRestartAfterPendingUpdate,
} from '@/helpers/utils.js'
import i18n from '@/i18n.config'
import {
	appUpdateState,
	downloadAvailableAppUpdate,
	getNextAppUpdatePopupTime,
	installAvailableAppUpdate,
	markAppUpdateActionable,
	markAppUpdatePopupShown,
	openAppUpdateChangelog,
	setAppUpdateActions,
} from '@/providers/app-update.ts'
import { createContentInstall, provideContentInstall } from '@/providers/content-install'
import {
	provideAppUpdateDownloadProgress,
	subscribeToDownloadProgress,
} from '@/providers/download-progress.ts'
import { createServerInstall, provideServerInstall } from '@/providers/server-install'
import { setupProviders } from '@/providers/setup'
import { setupAuthProvider } from '@/providers/setup/auth'
import { setupLoadingStateProvider } from '@/providers/setup/loading-state'
import { useError } from '@/store/error.js'
import { useTheming } from '@/store/state'

import { generateSkinPreviews } from './helpers/rendering/batch-skin-renderer'
import { get_available_capes, get_available_skins } from './helpers/skins'
import { AppNotificationManager } from './providers/app-notifications'
import { AppPopupNotificationManager } from './providers/app-popup-notifications'

const themeStore = useTheming()
const router = useRouter()
const route = useRoute()
const APP_LEFT_NAV_WIDTH = '4rem'
const APP_SIDEBAR_WIDTH = 300
const INTERCOM_BUBBLE_DEFAULT_PADDING = 20
const amberiteAuth = useAmberiteAuth()
const modrinthLink = useModrinthLink()
const PRIDE_FUNDRAISER_END_DATE = new Date('2026-07-01T00:00:00Z').getTime()
const hasMinecraftAccounts = ref(false)
const amberiteAccountMissing = computed(
	() => hasMinecraftAccounts.value && amberiteAuth.isReady.value && !amberiteAuth.user.value,
)
const AMBERITE_ACCOUNT_DISMISS_KEY = 'amberite:account-modal:dismissed'
const amberiteAccountDismissedForSession = ref(false)
try {
	amberiteAccountDismissedForSession.value =
		window.sessionStorage.getItem(AMBERITE_ACCOUNT_DISMISS_KEY) === 'true'
} catch {
	// sessionStorage can be unavailable in tests.
}
const amberiteAccountModalStep = ref('main')
const showAmberiteAccountModal = computed(
	() => amberiteAccountMissing.value && !amberiteAccountDismissedForSession.value,
)
const sidebarToggled = ref(true)
const unsubscribeSidebarToggle = themeStore.$subscribe(() => {
	sidebarToggled.value = !themeStore.toggleSidebar
})
const forceSidebar = computed(
	() => route.path.startsWith('/browse') || route.path.startsWith('/project'),
)
const sidebarVisible = computed(() => sidebarToggled.value || forceSidebar.value)
const hostingRouteActive = computed(() => route.path.startsWith('/hosting'))
const prideFundraiserEnabled = computed(
	() => themeStore.getFeatureFlag('pride_fundraiser') && Date.now() < PRIDE_FUNDRAISER_END_DATE,
)
const hostingIntercomIdentityKey = computed(() => {
	const rawServerId = route.params.id
	const serverId = Array.isArray(rawServerId) ? rawServerId[0] : rawServerId
	const userId = amberiteAuth.user.value?.id ?? 'anonymous'
	return `${userId}:${serverId ?? 'hosting'}`
})
const hostingIntercom = useHostingIntercom({
	enabled: computed(() => hostingRouteActive.value && amberiteAuth.isLoggedIn.value),
	appId: 'ykeritl9',
	fetchToken: fetchIntercomToken,
	identityKey: hostingIntercomIdentityKey,
	horizontalPadding: computed(() =>
		sidebarVisible.value
			? APP_SIDEBAR_WIDTH + INTERCOM_BUBBLE_DEFAULT_PADDING
			: INTERCOM_BUBBLE_DEFAULT_PADDING,
	),
})

const notificationManager = new AppNotificationManager()
provideNotificationManager(notificationManager)
const { handleError, addNotification } = notificationManager

const popupNotificationManager = new AppPopupNotificationManager()
providePopupNotificationManager(popupNotificationManager)
const { addPopupNotification } = popupNotificationManager

const appVersion = getVersion()
const tauriApiClient = new TauriModrinthClient({
	userAgent: async () => `modrinth/theseus/${await appVersion} (support@modrinth.com)`,
	labrinthBaseUrl: config.labrinthBaseUrl,
	archonBaseUrl: config.archonBaseUrl,
	features: [
		new NodeAuthFeature({
			getAuth: () => nodeAuthState.getAuth?.() ?? null,
			refreshAuth: async () => {
				if (nodeAuthState.refreshAuth) {
					await nodeAuthState.refreshAuth()
				}
			},
		}),
		new AuthFeature({
			token: async () => modrinthLink.credentials.value?.session ?? null,
		}),
		new PanelVersionFeature(),
		new VerboseLoggingFeature(),
	],
})
provideModrinthClient(tauriApiClient)
useQuery({
	queryKey: computed(() => ['authenticated-user', 'campaigns', modrinthLink.user.value?.id]),
	queryFn: () => tauriApiClient.labrinth.users_v3.getAuthenticated(),
	enabled: () => modrinthLink.linked.value,
	retry: false,
})
providePageContext({
	hierarchicalSidebarAvailable: ref(true),
	showAds: ref(false),
	floatingActionBarOffsets: {
		left: ref(APP_LEFT_NAV_WIDTH),
		right: computed(() => (sidebarVisible.value ? `${APP_SIDEBAR_WIDTH}px` : '0px')),
	},
	intercomBubble: hostingIntercom.intercomBubble,
	featureFlags: {
		serverRamAsBytesAlwaysOn: computed(() =>
			themeStore.getFeatureFlag('server_ram_as_bytes_always_on'),
		),
	},
	openExternalUrl: (url) => openUrl(url),
})
provideModalBehavior({
	noblur: computed(() => !themeStore.advancedRendering),
	onShow: () => hide_ads_window(),
	onHide: () => show_ads_window(),
})

const {
	installationModal,
	unknownPackWarningModal,
	fetchExistingInstanceNames,
	handleCreate,
	handleBrowseModpacks,
	searchModpacks,
	getProjectVersions,
	getLoaderManifest,
	setModpackAlreadyInstalledModal,
	handleModpackDuplicateCreateAnyway,
	handleModpackDuplicateGoToInstance,
} = setupProviders(notificationManager, popupNotificationManager)

const availableSurvey = ref(false)
const displayedServerInviteNotifications = new Set()

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
	offline.value = true
})
window.addEventListener('online', () => {
	offline.value = false
})

const showOnboarding = ref(false)
const nativeDecorations = ref(false)

const os = ref('')
const isDevEnvironment = ref(false)

const stateInitialized = ref(false)

const criticalErrorMessage = ref()

const isMaximized = ref(false)

const authUnreachableDebug = useDebugLogger('AuthReachableChecker')
const authServerQuery = useQuery({
	queryKey: ['authServerReachability'],
	queryFn: async () => {
		await check_reachable()
		authUnreachableDebug('Auth servers are reachable')
		return true
	},
	refetchInterval: 5 * 60 * 1000, // 5 minutes
	retry: false,
	refetchOnWindowFocus: false,
})

const authUnreachable = computed(() => {
	if (authServerQuery.isError.value && !authServerQuery.isLoading.value) {
		console.warn('Failed to reach auth servers', authServerQuery.error.value)
		return true
	}
	return false
})

onMounted(async () => {
	await useCheckDisableMouseover()

	document.querySelector('body').addEventListener('click', handleClick)
	document.querySelector('body').addEventListener('auxclick', handleAuxClick)

	checkUpdates()
})

onUnmounted(async () => {
	document.querySelector('body').removeEventListener('click', handleClick)
	document.querySelector('body').removeEventListener('auxclick', handleAuxClick)
	unsubscribeSidebarToggle()
	clearDelayedUpdatePopup()

	await unlistenUpdateDownload?.()
})

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()

const messages = defineMessages({
	updateInstalledToastTitle: {
		id: 'app.update.complete-toast.title',
		defaultMessage: 'Version {version} was successfully installed!',
	},
	updateInstalledToastText: {
		id: 'app.update.complete-toast.text',
		defaultMessage: 'Click here to view the changelog.',
	},
	authUnreachableHeader: {
		id: 'app.auth-servers.unreachable.header',
		defaultMessage: 'Cannot reach authentication servers',
	},
	authUnreachableBody: {
		id: 'app.auth-servers.unreachable.body',
		defaultMessage:
			'Minecraft authentication servers may be down right now. Check your internet connection and try again later.',
	},
	amberiteAccountModalHeader: {
		id: 'app.amberite-account-modal.header',
		defaultMessage: 'Connect your Amberite account',
	},
	amberiteAccountModalBody: {
		id: 'app.amberite-account-modal.body',
		defaultMessage:
			'You are signed in to Minecraft, but Amberite has not finished creating your app account.',
	},
	amberiteAccountModalHint: {
		id: 'app.amberite-account-modal.hint',
		defaultMessage:
			'Friends, Core access, invites, and sync need this account before the app can work correctly.',
	},
	amberiteAccountModalConnect: {
		id: 'app.amberite-account-modal.connect',
		defaultMessage: 'Connect Amberite account',
	},
	amberiteAccountModalConnecting: {
		id: 'app.amberite-account-modal.connecting',
		defaultMessage: 'Connecting...',
	},
	amberiteAccountModalDismiss: {
		id: 'app.amberite-account-modal.dismiss',
		defaultMessage: 'Fix later',
	},
	amberiteAccountModalDismissConfirmHeader: {
		id: 'app.amberite-account-modal.dismiss-confirm.header',
		defaultMessage: 'Are you sure?',
	},
	amberiteAccountModalDismissConfirmBody: {
		id: 'app.amberite-account-modal.dismiss-confirm.body',
		defaultMessage:
			'Not fixing this right now will result in features such as Core access, invites, friends, and sync being unavailable.',
	},
	amberiteAccountModalDismissConfirmBack: {
		id: 'app.amberite-account-modal.dismiss-confirm.back',
		defaultMessage: 'Go back',
	},
	amberiteAccountModalDismissConfirmConfirm: {
		id: 'app.amberite-account-modal.dismiss-confirm.confirm',
		defaultMessage: 'Yes, dismiss',
	},
})

async function setupApp() {
	const {
		native_decorations,
		theme,
		locale,
		telemetry,
		collapsed_navigation,
		hide_nametag_skins_page,
		advanced_rendering,
		onboarded,
		default_page,
		toggle_sidebar,
		developer_mode,
		feature_flags,
		pending_update_toast_for_version,
	} = await getSettings()

	// Initialize locale from saved settings
	if (locale) {
		i18n.global.locale.value = locale
	}

	if (default_page === 'Library') {
		await router.push('/library')
	}

	os.value = await getOS()
	const dev = await isDev()
	isDevEnvironment.value = dev
	const version = await getVersion()
	showOnboarding.value = !onboarded

	nativeDecorations.value = native_decorations
	if (os.value !== 'MacOS') await getCurrentWindow().setDecorations(native_decorations)

	themeStore.setThemeState(theme)
	themeStore.collapsedNavigation = collapsed_navigation
	themeStore.advancedRendering = advanced_rendering
	themeStore.hideNametagSkinsPage = hide_nametag_skins_page
	themeStore.toggleSidebar = toggle_sidebar
	themeStore.devMode = developer_mode
	themeStore.featureFlags = feature_flags
	stateInitialized.value = true

	isMaximized.value = await getCurrentWindow().isMaximized()

	await getCurrentWindow().onResized(async () => {
		isMaximized.value = await getCurrentWindow().isMaximized()
	})

	if (telemetry) {
		initAnalytics()
		if (dev) debugAnalytics()
		trackEvent('Launched', { version, dev, onboarded })
	}

	if (!dev) document.addEventListener('contextmenu', (event) => event.preventDefault())

	const osType = await type()
	if (osType === 'macos') {
		document.getElementsByTagName('html')[0].classList.add('mac')
	} else {
		document.getElementsByTagName('html')[0].classList.add('windows')
	}

	await warning_listener((e) =>
		addNotification({
			title: 'Warning',
			text: e.message,
			type: 'warn',
		}),
	)

	fetch(`https://api.modrinth.com/appCriticalAnnouncement.json?version=${version}`)
		.then((response) => response.json())
		.then((res) => {
			if (res && res.header && res.body) {
				criticalErrorMessage.value = res
			}
		})
		.catch(() => {
			console.log(
				`No critical announcement found at https://api.modrinth.com/appCriticalAnnouncement.json?version=${version}`,
			)
		})

	get_opening_command().then(handleCommand)
	void modrinthLink.refresh()

	try {
		const skins = (await get_available_skins()) ?? []
		const capes = (await get_available_capes()) ?? []
		generateSkinPreviews(skins, capes)
	} catch (error) {
		console.warn('Failed to generate skin previews in app setup.', error)
	}

	if (pending_update_toast_for_version !== null) {
		const settings = await getSettings()
		settings.pending_update_toast_for_version = null
		await setSettings(settings)
	}

	if (osType === 'windows') {
		await processPendingSurveys()
	} else {
		console.info('Skipping user surveys on non-Windows platforms')
	}
}

const stateFailed = ref(false)
initialize_state()
	.then(() => {
		setupApp().catch((err) => {
			stateFailed.value = true
			console.error(err)
			error.showError(err, null, false, 'state_init')
		})
	})
	.catch((err) => {
		stateFailed.value = true
		console.error('Failed to initialize app', err)
		error.showError(err, null, false, 'state_init')
	})

const handleClose = async () => {
	await saveWindowState(StateFlags.ALL)
	await getCurrentWindow().close()
}

const loading = setupLoadingStateProvider()
loading.setEnabled(false)
let initialLoadToken = loading.begin()
let routerToken = null
let suspenseToken = null

let suspensePending = false

const sidebarOverlayScrollbarsOptions = Object.freeze({
	overflow: {
		x: 'hidden',
		y: 'scroll',
	},
})

router.beforeEach(() => {
	suspensePending = false
	if (routerToken) loading.end(routerToken)
	routerToken = loading.begin()
})
router.afterEach((to, from, failure) => {
	trackEvent('PageView', {
		path: to.path,
		fromPath: from.path,
		failed: failure,
	})
	setTimeout(() => {
		if (!suspensePending && stateInitialized.value) {
			if (initialLoadToken) {
				loading.end(initialLoadToken)
				initialLoadToken = null
			}
			if (routerToken) {
				loading.end(routerToken)
				routerToken = null
			}
		}
	}, 100)
})

function onSuspensePending() {
	suspensePending = true
	if (suspenseToken) loading.end(suspenseToken)
	suspenseToken = loading.begin()
}

function onSuspenseResolve() {
	if (suspenseToken) {
		loading.end(suspenseToken)
		suspenseToken = null
	}
	if (routerToken) {
		loading.end(routerToken)
		routerToken = null
	}
}

const queryClient = useQueryClient()

watch(stateInitialized, (ready) => {
	if (ready) {
		if (initialLoadToken) {
			loading.end(initialLoadToken)
			initialLoadToken = null
		}
		if (routerToken) {
			loading.end(routerToken)
			routerToken = null
		}

		queryClient.prefetchQuery({
			queryKey: ['servers'],
			queryFn: async () => {
				const response = await tauriApiClient.archon.servers_v0.list({ limit: 100 })
				const hasMedalServers = response.servers.some((s) => s.is_medal)
				if (hasMedalServers) {
					const subscriptions = await tauriApiClient.labrinth.billing_internal.getSubscriptions()
					for (const server of response.servers) {
						if (server.is_medal) {
							const sub = subscriptions.find((s) => s.metadata?.id === server.server_id)
							if (sub) {
								server.medal_expires = new Date(
									new Date(sub.created).getTime() + 5 * 86400000,
								).toISOString()
							}
						}
					}
				}
				return response
			},
			staleTime: 30_000,
		})
		queryClient.prefetchQuery({
			queryKey: ['billing', 'subscriptions'],
			queryFn: () => tauriApiClient.labrinth.billing_internal.getSubscriptions(),
			staleTime: 30_000,
		})
		queryClient.prefetchQuery({
			queryKey: ['billing', 'payments'],
			queryFn: () => tauriApiClient.labrinth.billing_internal.getPayments(),
			staleTime: 30_000,
		})
	}
})

const error = useError()
const errorModal = ref()
const minecraftAuthErrorModal = ref()
const amberiteAccountModal = ref()

const contentInstall = createContentInstall({ router, handleError })
provideContentInstall(contentInstall)
const {
	instances: contentInstallInstances,
	compatibleLoaders: contentInstallLoaders,
	gameVersions: contentInstallGameVersions,
	loading: contentInstallLoading,
	defaultTab: contentInstallDefaultTab,
	preferredLoader: contentInstallPreferredLoader,
	preferredGameVersion: contentInstallPreferredGameVersion,
	releaseGameVersions: contentInstallReleaseGameVersions,
	projectInfo: contentInstallProjectInfo,
	handleInstallToInstance,
	handleCreateAndInstall,
	handleNavigate: handleContentInstallNavigate,
	handleCancel: handleContentInstallCancel,
	setContentInstallModal,
	setModpackAlreadyInstalledModal: setContentInstallModpackAlreadyInstalledModal,
	handleModpackDuplicateCreateAnyway: handleContentInstallModpackDuplicateCreateAnyway,
	handleModpackDuplicateGoToInstance: handleContentInstallModpackDuplicateGoToInstance,
	setIncompatibilityWarningModal: setContentIncompatibilityWarningModal,
	incompatibilityWarningVersions: contentInstallIncompatibilityWarningVersions,
	incompatibilityWarningCurrentGameVersion: contentInstallIncompatibilityWarningCurrentGameVersion,
	incompatibilityWarningCurrentLoader: contentInstallIncompatibilityWarningCurrentLoader,
	incompatibilityWarningProjectType: contentInstallIncompatibilityWarningProjectType,
	incompatibilityWarningProjectIconUrl: contentInstallIncompatibilityWarningProjectIconUrl,
	incompatibilityWarningProjectName: contentInstallIncompatibilityWarningProjectName,
	incompatibilityWarningMessage: contentInstallIncompatibilityWarningMessage,
	incompatibilityWarningInstalling: contentInstallIncompatibilityWarningInstalling,
	handleIncompatibilityWarningInstall: handleContentInstallIncompatibilityWarningInstall,
	handleIncompatibilityWarningCancel: handleContentInstallIncompatibilityWarningCancel,
} = contentInstall

const serverInstall = createServerInstall({ router, handleError, popupNotificationManager })
provideServerInstall(serverInstall)
const {
	setInstallToPlayModal: setServerInstallToPlayModal,
	setUpdateToPlayModal: setServerUpdateToPlayModal,
	setAddServerToInstanceModal: setServerAddServerToInstanceModal,
	playServerProject,
} = serverInstall

const modInstallModal = ref()
const modpackAlreadyInstalledModal = ref()
const contentInstallModpackAlreadyInstalledModal = ref()
const addServerToInstanceModal = ref()
const incompatibilityWarningModal = ref()
const installToPlayModal = ref()
const updateToPlayModal = ref()
const modrinthLoginFlowWaitModal = ref()

watch(incompatibilityWarningModal, (modal) => {
	if (modal) {
		setContentIncompatibilityWarningModal(modal)
	}
})

setupAuthProvider(amberiteAuth.user, async (_redirectPath) => {
	await signIn()
})

async function signIn() {
	await amberiteAuth.signIn()
}

async function handleMinecraftAccountChange(hasAccounts) {
	hasMinecraftAccounts.value = hasAccounts
	if (hasAccounts) {
		await signIn()
	} else {
		await amberiteAuth.logOut().catch(handleError)
	}
}

watch(showAmberiteAccountModal, (shouldShow) => {
	if (shouldShow) {
		setTimeout(() => {
			amberiteAccountModalStep.value = 'main'
			amberiteAccountModal.value?.show()
		}, 0)
	} else {
		amberiteAccountModal.value?.hide()
	}
})

watch(amberiteAuth.user, (user) => {
	if (user) {
		amberiteAccountDismissedForSession.value = false
		try {
			window.sessionStorage.removeItem(AMBERITE_ACCOUNT_DISMISS_KEY)
		} catch {
			// sessionStorage can be unavailable in tests.
		}
	}
})

async function logOut() {
	await amberiteAuth.logOut().catch(handleError)
}

function beginDismissAmberiteAccountModal() {
	amberiteAccountModalStep.value = 'confirm-dismiss'
}

function cancelDismissAmberiteAccountModal() {
	amberiteAccountModalStep.value = 'main'
}

function confirmDismissAmberiteAccountModal() {
	amberiteAccountDismissedForSession.value = true
	try {
		window.sessionStorage.setItem(AMBERITE_ACCOUNT_DISMISS_KEY, 'true')
	} catch {
		// sessionStorage can be unavailable in tests.
	}
	amberiteAccountModal.value?.hide()
}

const hasPlus = computed(() => false)

const showAd = computed(() => false)

async function fetchIntercomToken() {
	if (!amberiteAuth.isLoggedIn.value) {
		throw new Error('Not authenticated')
	}

	const params = new URLSearchParams()
	const rawServerId = route.params.id
	const serverId = Array.isArray(rawServerId) ? rawServerId[0] : rawServerId
	if (route.path.startsWith('/hosting/manage/') && typeof serverId === 'string') {
		params.set('server_id', serverId)
	}
	const query = params.size > 0 ? `?${params.toString()}` : ''

	const response = await tauriFetch(`${config.siteUrl}/api/intercom/messenger-jwt${query}`, {
		method: 'GET',
	})
	if (!response.ok) {
		throw new Error(`Failed to fetch Intercom token: ${response.status}`)
	}
	return await response.json()
}

watch(showAd, () => {
	if (!showAd.value) {
		hide_ads_window(true)
	} else {
		init_ads_window(true)
	}
})

onMounted(() => {
	invoke('show_window')

	error.setErrorModal(errorModal.value)
	error.setMinecraftAuthErrorModal(minecraftAuthErrorModal.value)

	setContentIncompatibilityWarningModal(incompatibilityWarningModal.value)
	setContentInstallModal(modInstallModal.value)
	setContentInstallModpackAlreadyInstalledModal(contentInstallModpackAlreadyInstalledModal.value)
	setModpackAlreadyInstalledModal(modpackAlreadyInstalledModal.value)
	setServerAddServerToInstanceModal(addServerToInstanceModal.value)
	setServerInstallToPlayModal(installToPlayModal.value)
	setServerUpdateToPlayModal(updateToPlayModal.value)
})

const accounts = ref(null)
provide('accountsCard', accounts)

command_listener(handleCommand)
notification_listener(handleLiveNotification)

async function markLiveNotificationRead(notification) {
	try {
		await tauriApiClient.labrinth.notifications_v2.markAsRead(notification.id)
	} catch (error) {
		if (error instanceof ModrinthApiError && error.statusCode === 404) {
			console.warn(`notification ${notification.id} could not be marked as read`, error)
			return
		}
		throw error
	}
}

async function respondToServerInvite(notification, action) {
	const serverId = notification.body?.server_id
	if (typeof serverId !== 'string') {
		throw new Error('Missing server ID for invite notification.')
	}

	await tauriApiClient.request(`/servers/${serverId}/invites/${action}`, {
		api: 'archon',
		version: 1,
		method: 'POST',
	})
	await markLiveNotificationRead(notification)

	return serverId
}

async function acceptServerInviteNotification(notification) {
	try {
		const serverId = await respondToServerInvite(notification, 'accept')
		await router.push(`/hosting/manage/${encodeURIComponent(serverId)}`)
		queryClient.invalidateQueries({ queryKey: ['servers'] })
	} catch (error) {
		handleError(error)
	}
}

async function declineServerInviteNotification(notification) {
	try {
		await respondToServerInvite(notification, 'decline')
	} catch (error) {
		handleError(error)
	}
}

function openServerInviteInviterProfile(inviterName) {
	if (!inviterName) return
	openUrl(`${config.siteUrl}/user/${encodeURIComponent(inviterName)}`)
}

async function handleLiveNotification(notification) {
	if (notification?.body?.type !== 'server_invite' || notification.read) return
	if (displayedServerInviteNotifications.has(notification.id)) return

	displayedServerInviteNotifications.add(notification.id)

	const serverName =
		typeof notification.body.server_name === 'string' ? notification.body.server_name : 'a server'
	const inviterId = notification.body.invited_by
	const invitedBy =
		typeof inviterId === 'string' ? await get_user(inviterId, 'bypass').catch(() => null) : null

	addPopupNotification({
		title: serverName,
		autoCloseMs: null,
		toast: {
			type: 'server-invite',
			actorName: invitedBy?.username ?? null,
			actorAvatarUrl: invitedBy?.avatar_url ?? null,
			entityName: serverName,
			onAccept: () => acceptServerInviteNotification(notification),
			onDecline: () => declineServerInviteNotification(notification),
			onOpenActor: () => openServerInviteInviterProfile(invitedBy?.username ?? null),
		},
	})
}

async function handleCommand(e) {
	if (!e) return

	if (e.event === 'RunMRPack') {
		// RunMRPack should directly install a local mrpack given a path
		if (e.path.endsWith('.mrpack')) {
			await create_profile_and_install_from_file(e.path, (createProfile, fileName) =>
				unknownPackWarningModal.value?.show(createProfile, fileName),
			).catch(handleError)
			trackEvent('InstanceCreate', {
				source: 'CreationModalFileDrop',
			})
		}
	} else if (e.event === 'InstallServer') {
		await router.push(`/project/${e.id}`)
		await playServerProject(e.id).catch(handleError)
	} else if (e.event === 'InstallVersion') {
		const version = await get_version(e.id, 'must_revalidate').catch(handleError)
		if (version) {
			await contentInstall
				.install(version.project_id, version.id, null, 'URLConfirmModal', undefined, undefined, {
					showProjectInfo: true,
				})
				.catch(handleError)
		}
	} else {
		await contentInstall
			.install(e.id, null, null, 'URLConfirmModal', undefined, undefined, { showProjectInfo: true })
			.catch(handleError)
	}
}

const appUpdateDownload = {
	progress: appUpdateState.progress,
	version: ref(),
}
let unlistenUpdateDownload

const {
	metered,
	finishedDownloading,
	downloading,
	restarting,
	availableUpdate,
	updateSize,
	updatesEnabled,
} = appUpdateState
let delayedUpdatePopupTimeout = null

const updatePopupMessages = defineMessages({
	updateAvailable: {
		id: 'app.update-popup.title',
		defaultMessage: 'Update available',
	},
	downloadComplete: {
		id: 'app.update-popup.download-complete',
		defaultMessage: 'Download complete',
	},
	meteredBody: {
		id: 'app.update-popup.body.metered',
		defaultMessage: `Modrinth App v{version} is available now! Since you're on a metered network, we didn't automatically download it.`,
	},
	downloadedBody: {
		id: 'app.update-popup.body.download-complete',
		defaultMessage: `Modrinth App v{version} has finished downloading. Reload to update now, or automatically when you close Modrinth App.`,
	},
	linuxBody: {
		id: 'app.update-popup.body.linux',
		defaultMessage:
			'Modrinth App v{version} is available. Use your package manager to update for the latest features and fixes!',
	},
	reload: {
		id: 'app.update-popup.reload',
		defaultMessage: 'Reload to update',
	},
	download: {
		id: 'app.update-popup.download',
		defaultMessage: 'Download ({size})',
	},
	changelog: {
		id: 'app.update-popup.changelog',
		defaultMessage: 'Changelog',
	},
})

function clearDelayedUpdatePopup() {
	if (delayedUpdatePopupTimeout !== null) {
		clearTimeout(delayedUpdatePopupTimeout)
		delayedUpdatePopupTimeout = null
	}
}

function getCurrentUpdatePromptStage() {
	return finishedDownloading.value ? 'downloaded' : 'available'
}

function scheduleDelayedUpdatePopup() {
	clearDelayedUpdatePopup()

	const version = availableUpdate.value?.version
	if (!version) {
		return
	}

	const nextPopupTime = getNextAppUpdatePopupTime(version, getCurrentUpdatePromptStage())
	if (nextPopupTime === null) {
		return
	}

	const delay = nextPopupTime - Date.now()
	if (delay <= 0) {
		showDelayedUpdatePopup()
		return
	}

	delayedUpdatePopupTimeout = setTimeout(showDelayedUpdatePopup, Math.min(delay, 2_147_483_647))
}

function showDelayedUpdatePopup() {
	const update = availableUpdate.value
	if (!update) {
		return
	}

	const stage = getCurrentUpdatePromptStage()
	const nextPopupTime = getNextAppUpdatePopupTime(update.version, stage)
	if (nextPopupTime === null) {
		return
	}

	if (Date.now() < nextPopupTime) {
		scheduleDelayedUpdatePopup()
		return
	}

	if (metered.value && !finishedDownloading.value) {
		addPopupNotification({
			title: formatMessage(updatePopupMessages.updateAvailable),
			text: formatMessage(updatePopupMessages.meteredBody, { version: update.version }),
			type: 'info',
			autoCloseMs: null,
			buttons: [
				{
					label: formatMessage(updatePopupMessages.download, {
						size: formatBytes(updateSize.value ?? 0),
					}),
					action: () => downloadAvailableAppUpdate(),
					color: 'brand',
				},
				{
					label: formatMessage(updatePopupMessages.changelog),
					action: () => openAppUpdateChangelog(),
					keepOpen: true,
				},
			],
		})
	} else if (finishedDownloading.value) {
		addPopupNotification({
			title: formatMessage(updatePopupMessages.downloadComplete),
			text: formatMessage(updatePopupMessages.downloadedBody, {
				version: update.version,
			}),
			type: 'success',
			autoCloseMs: null,
			buttons: [
				{
					label: formatMessage(updatePopupMessages.reload),
					action: () => installAvailableAppUpdate(),
					color: 'brand',
				},
				{
					label: formatMessage(updatePopupMessages.changelog),
					action: () => openAppUpdateChangelog(),
					keepOpen: true,
				},
			],
		})
	} else {
		scheduleDelayedUpdatePopup()
		return
	}

	markAppUpdatePopupShown(update.version, stage)
}

async function checkUpdates() {
	if (!(await areUpdatesEnabled())) {
		console.log('Skipping update check as updates are disabled in this build or environment')
		updatesEnabled.value = false

		if (os.value === 'Linux' && !isDevEnvironment.value) {
			checkLinuxUpdates()
			setInterval(checkLinuxUpdates, 5 * 60 * 1000)
		}
		return
	}

	async function performCheck() {
		const update = await invoke('plugin:updater|check')
		if (!update) {
			console.log('No update available')
			return
		}

		const isExistingUpdate = update.version === availableUpdate.value?.version

		if (isExistingUpdate) {
			console.log('Update is already known')
			scheduleDelayedUpdatePopup()
			return
		}

		appUpdateDownload.progress.value = 0
		finishedDownloading.value = false
		downloading.value = false
		updateSize.value = null
		availableUpdate.value = update

		console.log(`Update ${update.version} is available.`)

		metered.value = await isNetworkMetered()
		if (!metered.value) {
			console.log('Starting download of update')
			downloadUpdate(update)
		} else {
			console.log(`Metered connection detected, not auto-downloading update.`)
			markAppUpdateActionable(update.version)
			scheduleDelayedUpdatePopup()
		}

		getUpdateSize(update.rid).then((size) => (updateSize.value = size))
	}

	await performCheck()
	setTimeout(
		() => {
			checkUpdates()
		},
		5 /* min */ * 60 /* sec */ * 1000 /* ms */,
	)
}

async function checkLinuxUpdates() {
	try {
		const [response, currentVersion] = await Promise.all([
			fetch('https://launcher-files.modrinth.com/updates.json'),
			getVersion(),
		])
		const updates = await response.json()
		const latestVersion = updates?.version

		if (latestVersion && latestVersion !== currentVersion) {
			markAppUpdateActionable(latestVersion)
			const nextPopupTime = getNextAppUpdatePopupTime(latestVersion)
			if (nextPopupTime !== null && Date.now() >= nextPopupTime) {
				addPopupNotification({
					title: formatMessage(updatePopupMessages.updateAvailable),
					text: formatMessage(updatePopupMessages.linuxBody, { version: latestVersion }),
					type: 'info',
					autoCloseMs: null,
				})
				markAppUpdatePopupShown(latestVersion)
			}
		}
	} catch (e) {
		console.error('Failed to check for updates:', e)
	}
}

async function downloadAvailableUpdate() {
	return downloadUpdate(availableUpdate.value)
}

async function downloadUpdate(versionToDownload) {
	if (!versionToDownload) {
		handleError(`Failed to download update: no version available`)
		return
	}

	if (downloading.value || appUpdateDownload.progress.value !== 0) {
		console.error(`Update ${versionToDownload.version} already downloading`)
		return
	}

	console.log(`Downloading update ${versionToDownload.version}`)
	downloading.value = true

	try {
		enqueueUpdateForInstallation(versionToDownload.rid)
			.then(() => {
				downloading.value = false
				finishedDownloading.value = true
				unlistenUpdateDownload?.().then(() => {
					unlistenUpdateDownload = null
				})
				console.log('Finished downloading!')
				markAppUpdateActionable(versionToDownload.version, 'downloaded')
				scheduleDelayedUpdatePopup()
			})
			.catch((e) => {
				downloading.value = false
				appUpdateDownload.progress.value = 0
				handleError(e)
			})
		unlistenUpdateDownload = await subscribeToDownloadProgress(
			appUpdateDownload,
			versionToDownload.version,
		)
	} catch (e) {
		downloading.value = false
		appUpdateDownload.progress.value = 0
		handleError(e)
	}
}

async function installUpdate() {
	restarting.value = true

	try {
		await setRestartAfterPendingUpdate(true)
	} catch (e) {
		restarting.value = false
		handleError(e)
		return
	}
	setTimeout(async () => {
		await handleClose()
	}, 250)
}

setAppUpdateActions({
	download: downloadAvailableUpdate,
	install: installUpdate,
	changelog: () => openUrl('https://modrinth.com/news/changelog?filter=app'),
})

async function openModrinthProjectLinkInApp(parsed) {
	const { slug, pathSuffix, url } = parsed
	const loadToken = loading.begin()
	try {
		const { id } = await tauriApiClient.labrinth.projects_v2.check(slug)
		const query = mergeUrlQuery(route.query, url)
		await router.push({
			path: `/project/${id}${pathSuffix}`,
			query,
			hash: url.hash || undefined,
		})
	} catch (err) {
		if (err instanceof ModrinthApiError && err.statusCode === 404) {
			openUrl(url.href)
		} else {
			handleError(err)
		}
	} finally {
		loading.end(loadToken)
	}
}

function handleClick(e) {
	let target = e.target
	while (target != null) {
		if (target.matches('a')) {
			if (
				target.href &&
				['http://', 'https://', 'mailto:', 'tel:'].some((v) => target.href.startsWith(v)) &&
				!target.classList.contains('router-link-active') &&
				!target.href.startsWith('http://localhost') &&
				!target.href.startsWith('https://tauri.localhost') &&
				!target.href.startsWith('http://tauri.localhost')
			) {
				const parsed = parseModrinthLink(target.href)
				if (target.target !== '_blank' && parsed) {
					void openModrinthProjectLinkInApp(parsed)
				} else {
					openUrl(target.href)
				}
			}
			e.preventDefault()
			break
		}
		target = target.parentElement
	}
}

function handleAuxClick(e) {
	// disables middle click -> new tab
	if (e.button === 1) {
		e.preventDefault()
		// instead do a left click
		const event = new MouseEvent('click', {
			view: window,
			bubbles: true,
			cancelable: true,
		})
		e.target.dispatchEvent(event)
	}
}

function cleanupOldSurveyDisplayData() {
	const threeWeeksAgo = new Date()
	threeWeeksAgo.setDate(threeWeeksAgo.getDate() - 21)

	for (let i = 0; i < localStorage.length; i++) {
		const key = localStorage.key(i)

		if (key.startsWith('survey-') && key.endsWith('-display')) {
			const dateValue = new Date(localStorage.getItem(key))
			if (dateValue < threeWeeksAgo) {
				localStorage.removeItem(key)
			}
		}
	}
}

async function openSurvey() {
	if (!availableSurvey.value) {
		console.error('No survey to open')
		return
	}

	const userId = amberiteAuth.user.value?.id

	const formId = availableSurvey.value.tally_id

	const popupOptions = {
		layout: 'modal',
		width: 700,
		autoClose: 2000,
		hideTitle: true,
		hiddenFields: {
			user_id: userId,
		},
		onOpen: () => console.info('Opened user survey'),
		onClose: () => {
			console.info('Closed user survey')
			show_ads_window()
		},
		onSubmit: () => console.info('Active user survey submitted'),
	}

	try {
		hide_ads_window()
		if (window.Tally?.openPopup) {
			console.info(`Opening Tally popup for user survey (form ID: ${formId})`)
			dismissSurvey()
			window.Tally.openPopup(formId, popupOptions)
		} else {
			console.warn('Tally script not yet loaded')
			show_ads_window()
		}
	} catch (e) {
		console.error('Error opening Tally popup:', e)
		show_ads_window()
	}

	console.info(`Found user survey to show with tally_id: ${formId}`)
	window.Tally.openPopup(formId, popupOptions)
}

function dismissSurvey() {
	localStorage.setItem(`survey-${availableSurvey.value.id}-display`, new Date())
	availableSurvey.value = undefined
}

async function processPendingSurveys() {
	function isWithinLastTwoWeeks(date) {
		const twoWeeksAgo = new Date()
		twoWeeksAgo.setDate(twoWeeksAgo.getDate() - 14)
		return date >= twoWeeksAgo
	}

	cleanupOldSurveyDisplayData()

	const userId = amberiteAuth.user.value?.id

	const instances = await list().catch(handleError)
	const isActivePlayer =
		instances.findIndex(
			(instance) =>
				isWithinLastTwoWeeks(instance.last_played) && !isWithinLastTwoWeeks(instance.created),
		) >= 0

	let surveys = []
	try {
		surveys = await $fetch('https://api.modrinth.com/v2/surveys')
	} catch (e) {
		console.error('Error fetching surveys:', e)
	}

	const surveyToShow = surveys.find(
		(survey) =>
			!!(
				localStorage.getItem(`survey-${survey.id}-display`) === null &&
				survey.type === 'tally_app' &&
				((survey.condition === 'active_player' && isActivePlayer) ||
					(survey.assigned_users?.includes(userId) && !survey.dismissed_users?.includes(userId)))
			),
	)

	if (surveyToShow) {
		availableSurvey.value = surveyToShow
	} else {
		console.info('No user survey to show')
	}
}

provideAppUpdateDownloadProgress(appUpdateDownload)
</script>

<template>
	<SplashScreen v-if="!stateFailed" ref="splashScreen" data-tauri-drag-region />
	<div id="teleports"></div>
	<div
		v-if="stateInitialized"
		class="app-grid-layout relative"
		:class="{ 'disable-advanced-rendering': !themeStore.advancedRendering }"
	>
		<Transition name="fade">
			<div
				v-if="restarting"
				data-tauri-drag-region
				class="inset-0 fixed bg-black/80 backdrop-blur z-[200] flex items-center justify-center"
			>
				<span
					data-tauri-drag-region
					class="flex items-center gap-4 text-contrast font-semibold text-xl select-none cursor-default"
				>
					<RefreshCwIcon data-tauri-drag-region class="animate-spin w-6 h-6" />
					Restarting...
				</span>
			</div>
		</Transition>
		<Suspense>
			<AppSettingsModal ref="settingsModal" />
		</Suspense>
		<Suspense>
			<AuthGrantFlowWaitModal ref="modrinthLoginFlowWaitModal" @flow-cancel="cancelLogin" />
		</Suspense>

		<InstanceCreationFlowModal
			ref="installationModal"
			type="instance"
			show-snapshot-toggle
			:fetch-existing-instance-names="fetchExistingInstanceNames"
			:search-modpacks="searchModpacks"
			:get-project-versions="getProjectVersions"
			:get-loader-manifest="getLoaderManifest"
			@create="handleCreate"
			@browse-modpacks="handleBrowseModpacks"
		/>
		<UnknownPackWarningModal ref="unknownPackWarningModal" />
		<div
			class="app-grid-navbar bg-bg-raised flex flex-col p-[0.5rem] pt-0 gap-[0.5rem] w-[--left-bar-width]"
		>
			<NavButton v-tooltip.right="'Home'" to="/">
				<HomeIcon />
			</NavButton>
			<NavButton v-if="themeStore.featureFlags.worlds_tab" v-tooltip.right="'Worlds'" to="/worlds">
				<WorldIcon />
			</NavButton>
			<NavButton
				v-tooltip.right="'Discover content'"
				to="/browse/modpack"
				:is-primary="() => route.path.startsWith('/browse') && !route.query.i"
				:is-subpage="(route) => route.path.startsWith('/project') && !route.query.i"
			>
				<CompassIcon />
			</NavButton>
			<NavButton v-tooltip.right="'Skin selector'" to="/skins">
				<ChangeSkinIcon />
			</NavButton>
			<NavButton
				v-tooltip.right="'Library'"
				to="/library"
				:is-primary="(r) => r.path === '/library' || r.path === '/library'"
				:is-subpage="
					() =>
						route.path.startsWith('/instance') ||
						((route.path.startsWith('/browse') || route.path.startsWith('/project')) &&
							route.query.i)
				"
			>
				<LibraryIcon />
			</NavButton>
			<NavButton v-tooltip.right="'Core'" to="/core">
				<ServerStackIcon />
			</NavButton>
			<div class="h-px w-6 mx-auto my-2 bg-surface-5"></div>
			<suspense>
				<QuickInstanceSwitcher />
			</suspense>
			<NavButton
				v-tooltip.right="'Create new instance'"
				:to="() => installationModal?.show()"
				:disabled="offline"
			>
				<PlusIcon />
			</NavButton>
			<div class="flex flex-grow"></div>
			<NavButton
				v-tooltip.right="formatMessage(commonMessages.settingsLabel)"
				:to="() => $refs.settingsModal.show()"
			>
				<SettingsIcon />
			</NavButton>
			<OverflowMenu
				v-if="amberiteAuth.user"
				v-tooltip.right="`Amberite account`"
				class="w-12 h-12 text-primary rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-button-bg hover:text-contrast border-0 cursor-pointer"
				:options="[
					{
						id: 'sign-out',
						action: () => logOut(),
						color: 'danger',
					},
				]"
				placement="right-end"
			>
				<Avatar :src="amberiteAuth.user?.avatar_url" alt="" size="32px" circle />
				<template #sign-out>
					<UserIcon />
					<span class="inline-flex items-center gap-1">
						Signed in as
						<span class="inline-flex items-center gap-1 text-contrast font-semibold">
							<Avatar :src="amberiteAuth.user?.avatar_url" alt="" size="20px" circle />
							{{ amberiteAuth.user?.username }}
						</span>
					</span>
					<LogOutIcon />
				</template>
			</OverflowMenu>
		</div>
		<div data-tauri-drag-region class="app-grid-statusbar bg-bg-raised h-[--top-bar-height] flex">
			<div data-tauri-drag-region class="flex min-w-0 flex-1 overflow-hidden p-3">
				<ModrinthAppLogo class="h-full w-auto shrink-0 text-contrast pointer-events-none" />
				<div data-tauri-drag-region class="flex shrink-0 items-center gap-1 ml-3">
					<button
						class="cursor-pointer p-0 m-0 text-contrast border-none outline-none bg-button-bg rounded-full flex items-center justify-center w-6 h-6 hover:brightness-75 transition-all"
						@click="router.back()"
					>
						<LeftArrowIcon />
					</button>
					<button
						class="cursor-pointer p-0 m-0 text-contrast border-none outline-none bg-button-bg rounded-full flex items-center justify-center w-6 h-6 hover:brightness-75 transition-all"
						@click="router.forward()"
					>
						<RightArrowIcon />
					</button>
				</div>
				<Breadcrumbs class="pt-[2px]" />
			</div>
			<section data-tauri-drag-region class="flex shrink-0 ml-auto items-center">
				<ButtonStyled
					v-if="!forceSidebar && themeStore.toggleSidebar"
					:type="sidebarToggled ? 'standard' : 'transparent'"
					circular
				>
					<button
						class="mr-3 transition-transform"
						:class="{ 'rotate-180': !sidebarToggled }"
						@click="sidebarToggled = !sidebarToggled"
					>
						<RightArrowIcon />
					</button>
				</ButtonStyled>
				<div class="flex mr-3">
					<Suspense>
						<AppActionBar />
					</Suspense>
				</div>
				<WindowControls />
			</section>
		</div>
	</div>
	<div
		v-if="stateInitialized"
		class="app-contents"
		:class="{
			'sidebar-enabled': sidebarVisible,
			'disable-advanced-rendering': !themeStore.advancedRendering,
		}"
	>
		<div class="app-viewport relative flex-grow router-view">
			<transition name="popup-survey">
				<div
					v-if="availableSurvey"
					class="w-[400px] z-20 fixed -bottom-12 pb-16 right-[--right-bar-width] mr-4 rounded-t-2xl card-shadow bg-bg-raised border-surface-5 border-[1px] border-solid border-b-0 p-4"
				>
					<h2 class="text-lg font-extrabold mt-0 mb-2">Hey there Modrinth user!</h2>
					<p class="m-0 leading-tight">
						Would you mind answering a few questions about your experience with Modrinth App?
					</p>
					<p class="mt-3 mb-4 leading-tight">
						This feedback will go directly to the Modrinth team and help guide future updates!
					</p>
					<div class="flex gap-2">
						<ButtonStyled color="brand">
							<button @click="openSurvey"><NotepadTextIcon /> Take survey</button>
						</ButtonStyled>
						<ButtonStyled>
							<button @click="dismissSurvey"><XIcon /> No thanks</button>
						</ButtonStyled>
					</div>
				</div>
			</transition>
			<div
				class="loading-indicator-container h-8 fixed z-50 pointer-events-none"
				:style="{
					top: 'calc(var(--top-bar-height))',
					left: 'calc(var(--left-bar-width))',
					width: 'calc(100% - var(--left-bar-width) - var(--right-bar-width))',
				}"
			>
				<LoadingBar position="absolute" />
			</div>
			<div
				v-if="themeStore.featureFlags.page_path"
				class="absolute bottom-0 left-0 m-2 bg-tooltip-bg text-tooltip-text font-semibold rounded-full px-2 py-1 text-xs z-50"
			>
				{{ route.fullPath }}
			</div>
			<div
				id="background-teleport-target"
				class="absolute h-full -z-10 rounded-tl-[--radius-xl] overflow-hidden"
				:style="{
					width: 'calc(100% - var(--right-bar-width))',
				}"
			></div>
			<Admonition
				v-if="criticalErrorMessage"
				type="critical"
				:header="criticalErrorMessage.header"
				class="m-6 mb-0"
			>
				<div
					class="markdown-body text-primary"
					v-html="renderString(criticalErrorMessage.body ?? '')"
				></div>
			</Admonition>
			<Admonition
				v-if="authUnreachable"
				type="warning"
				:header="formatMessage(messages.authUnreachableHeader)"
				class="m-6 mb-0"
			>
				{{ formatMessage(messages.authUnreachableBody) }}
			</Admonition>
			<RouterView v-slot="{ Component }">
				<template v-if="Component">
					<Suspense @pending="onSuspensePending" @resolve="onSuspenseResolve">
						<component :is="Component"></component>
					</Suspense>
				</template>
			</RouterView>
		</div>
		<div
			class="app-sidebar mt-px shrink-0 flex flex-col border-0 border-l-[1px] border-[--brand-gradient-border] border-solid"
			:class="{ 'has-plus': hasPlus }"
		>
			<div
				v-overlay-scrollbars="sidebarOverlayScrollbarsOptions"
				class="app-sidebar-scrollable flex-grow shrink relative"
				:class="{ 'pb-12': !hasPlus }"
				data-overlayscrollbars-initialize
			>
				<div id="sidebar-teleport-target" class="sidebar-teleport-content"></div>
				<div class="sidebar-default-content" :class="{ 'sidebar-enabled': sidebarVisible }">
					<div class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid">
						<h3 class="text-base text-primary font-medium m-0">Playing as</h3>
						<suspense>
							<AccountsCard ref="accounts" @change="handleMinecraftAccountChange" />
						</suspense>
						<div
							v-if="!amberiteAuth.user && hasMinecraftAccounts"
							class="mt-2 text-sm text-secondary leading-tight"
						>
							<template v-if="amberiteAuth.signingIn">
								Connecting your Amberite account...
							</template>
							<template v-else>
								Amberite account is not connected yet.
								<button
									class="p-0 border-0 bg-transparent text-brand cursor-pointer"
									@click="signIn"
								>
									Retry
								</button>
							</template>
						</div>
					</div>
					<div class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid">
						<suspense>
							<FriendsList />
						</suspense>
					</div>
					<PrideFundraiserBanner
						v-if="prideFundraiserEnabled"
						class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid"
					/>
				</div>
			</div>
			<template v-if="showAd">
				<a
					href="https://modrinth.plus?app"
					class="absolute bottom-[250px] w-full flex justify-center items-center gap-1 px-4 py-3 text-purple font-medium hover:underline z-10"
					target="_blank"
				>
					<ArrowBigUpDashIcon class="text-2xl" /> Upgrade to Modrinth+
				</a>
				<PromotionWrapper />
			</template>
		</div>
	</div>
	<I18nDebugPanel />
	<NotificationPanel :has-sidebar="sidebarVisible" />
	<PopupNotificationPanel :has-sidebar="sidebarVisible" />
	<ErrorModal ref="errorModal" />
	<MinecraftAuthErrorModal ref="minecraftAuthErrorModal" />
	<NewModal
		ref="amberiteAccountModal"
		:header="
			amberiteAccountModalStep === 'confirm-dismiss'
				? formatMessage(messages.amberiteAccountModalDismissConfirmHeader)
				: formatMessage(messages.amberiteAccountModalHeader)
		"
		:closable="false"
		max-width="32rem"
	>
		<div v-if="amberiteAccountModalStep === 'confirm-dismiss'" class="flex flex-col gap-5">
			<div class="flex gap-4">
				<div class="grid size-12 shrink-0 place-content-center rounded-full bg-red-highlight">
					<ShieldAlertIcon class="size-6 text-red" />
				</div>
				<p class="m-0 text-base text-primary leading-snug">
					{{ formatMessage(messages.amberiteAccountModalDismissConfirmBody) }}
				</p>
			</div>
			<div class="flex justify-end gap-3">
				<ButtonStyled type="transparent">
					<button @click="cancelDismissAmberiteAccountModal">
						{{ formatMessage(messages.amberiteAccountModalDismissConfirmBack) }}
					</button>
				</ButtonStyled>
				<ButtonStyled color="red">
					<button @click="confirmDismissAmberiteAccountModal">
						{{ formatMessage(messages.amberiteAccountModalDismissConfirmConfirm) }}
					</button>
				</ButtonStyled>
			</div>
		</div>
		<div v-else class="flex flex-col gap-5">
			<div class="flex gap-4">
				<div class="grid size-12 shrink-0 place-content-center rounded-full bg-orange-highlight">
					<ShieldAlertIcon class="size-6 text-orange" />
				</div>
				<div class="flex flex-col gap-2">
					<p class="m-0 text-base text-primary leading-snug">
						{{ formatMessage(messages.amberiteAccountModalBody) }}
					</p>
					<p class="m-0 text-sm text-secondary leading-snug">
						{{ formatMessage(messages.amberiteAccountModalHint) }}
					</p>
				</div>
			</div>
			<div
				v-if="amberiteAuth.error.value"
				class="rounded-lg border border-solid border-surface-5 bg-surface-2 p-3 text-sm text-secondary"
			>
				{{ amberiteAuth.error.value.message }}
			</div>
			<div class="flex justify-end gap-3">
				<ButtonStyled color="red" type="outlined">
					<button
						:disabled="amberiteAuth.signingIn.value"
						@click="beginDismissAmberiteAccountModal"
					>
						{{ formatMessage(messages.amberiteAccountModalDismiss) }}
					</button>
				</ButtonStyled>
				<ButtonStyled color="brand">
					<button :disabled="amberiteAuth.signingIn.value" @click="signIn">
						{{
							amberiteAuth.signingIn.value
								? formatMessage(messages.amberiteAccountModalConnecting)
								: formatMessage(messages.amberiteAccountModalConnect)
						}}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</NewModal>
	<ContentInstallModal
		ref="modInstallModal"
		:instances="contentInstallInstances"
		:compatible-loaders="contentInstallLoaders"
		:game-versions="contentInstallGameVersions"
		:loading="contentInstallLoading"
		:default-tab="contentInstallDefaultTab"
		:preferred-loader="contentInstallPreferredLoader"
		:preferred-game-version="contentInstallPreferredGameVersion"
		:release-game-versions="contentInstallReleaseGameVersions"
		:project-info="contentInstallProjectInfo"
		@install="handleInstallToInstance"
		@create-and-install="handleCreateAndInstall"
		@navigate="handleContentInstallNavigate"
		@cancel="handleContentInstallCancel"
	/>
	<ModpackAlreadyInstalledModal
		ref="modpackAlreadyInstalledModal"
		@create-anyway="handleModpackDuplicateCreateAnyway"
		@go-to-instance="handleModpackDuplicateGoToInstance"
	/>
	<AddServerToInstanceModal ref="addServerToInstanceModal" />
	<ContentUpdaterModal
		ref="incompatibilityWarningModal"
		mode="incompatibility-warning"
		:versions="contentInstallIncompatibilityWarningVersions"
		:current-game-version="contentInstallIncompatibilityWarningCurrentGameVersion"
		:current-loader="contentInstallIncompatibilityWarningCurrentLoader"
		current-version-id=""
		:is-app="true"
		:project-type="contentInstallIncompatibilityWarningProjectType"
		:project-icon-url="contentInstallIncompatibilityWarningProjectIconUrl"
		:project-name="contentInstallIncompatibilityWarningProjectName"
		:warning="contentInstallIncompatibilityWarningMessage"
		:action-loading="contentInstallIncompatibilityWarningInstalling"
		@update="handleContentInstallIncompatibilityWarningInstall"
		@cancel="handleContentInstallIncompatibilityWarningCancel"
	/>
	<ModpackAlreadyInstalledModal
		ref="contentInstallModpackAlreadyInstalledModal"
		@create-anyway="handleContentInstallModpackDuplicateCreateAnyway"
		@go-to-instance="handleContentInstallModpackDuplicateGoToInstance"
	/>
	<InstallToPlayModal ref="installToPlayModal" />
	<UpdateToPlayModal ref="updateToPlayModal" />
</template>

<style lang="scss" scoped>
.app-grid-layout,
.app-contents {
	--top-bar-height: 3rem;
	--left-bar-width: 4rem;
	--right-bar-width: 300px;
}

.app-grid-layout {
	display: grid;
	grid-template: 'status status' 'nav dummy';
	grid-template-columns: auto 1fr;
	grid-template-rows: auto 1fr;
	position: relative;
	//z-index: 0;
	background-color: var(--color-raised-bg);
	height: 100vh;
}

.app-grid-navbar {
	grid-area: nav;
	position: relative;
	z-index: 2;
}

.app-grid-statusbar {
	grid-area: status;
	padding-right: var(--window-controls-width, 0px);
	position: relative;
	z-index: 2;
}

[data-tauri-drag-region-exclude] {
	-webkit-app-region: no-drag;
}

.app-contents {
	position: absolute;
	z-index: 1;
	left: var(--left-bar-width);
	top: var(--top-bar-height);
	right: 0;
	bottom: 0;
	height: calc(100vh - var(--top-bar-height));
	background-color: var(--color-bg);
	border-top-left-radius: var(--radius-xl);

	display: grid;
	grid-template-columns: 1fr 0px;
	// transition: grid-template-columns 0.4s ease-in-out;

	&.sidebar-enabled {
		grid-template-columns: 1fr 300px;
	}
}

.loading-indicator-container {
	border-top-left-radius: var(--radius-xl);
	overflow: hidden;
}

.app-sidebar {
	overflow: visible;
	width: 300px;
	position: relative;
	height: calc(100vh - var(--top-bar-height));
	background: var(--brand-gradient-bg);

	--color-button-bg: var(--brand-gradient-button);
	--color-button-bg-hover: var(--brand-gradient-border);
	--color-divider: var(--brand-gradient-border);
	--color-divider-dark: var(--brand-gradient-border);
}

.disable-advanced-rendering {
	.app-sidebar::before {
		box-shadow: none;
	}

	&.app-contents::before {
		box-shadow: none;
	}

	*,
	:deep(*) {
		box-shadow: none !important;
		--tw-drop-shadow:;
	}
}

.app-sidebar::before {
	content: '';
	box-shadow: -15px 0 15px -15px rgba(0, 0, 0, 0.1) inset;
	top: 0;
	bottom: 0;
	left: -2rem;
	width: 2rem;
	position: absolute;
	pointer-events: none;
}

.app-viewport {
	flex-grow: 1;
	height: 100%;
	overflow: auto;
	overflow-x: hidden;
	scrollbar-gutter: stable;
}

.app-contents::before {
	z-index: 30;
	content: '';
	position: fixed;
	left: var(--left-bar-width);
	top: var(--top-bar-height);
	right: calc(-1 * var(--left-bar-width));
	bottom: calc(-1 * var(--left-bar-width));
	border-radius: var(--radius-xl);
	box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1) inset;
	border-color: var(--surface-5);
	border-width: 1px;
	border-style: solid;
	pointer-events: none;
}

.sidebar-teleport-content {
	display: contents;
}

.sidebar-default-content {
	display: none;
}

.sidebar-teleport-content:empty + .sidebar-default-content.sidebar-enabled {
	display: contents;
}

.popup-survey-enter-active {
	transition:
		opacity 0.25s ease,
		transform 0.25s cubic-bezier(0.51, 1.08, 0.35, 1.15);
	transform-origin: top center;
}

.popup-survey-leave-active {
	transition:
		opacity 0.25s ease,
		transform 0.25s cubic-bezier(0.68, -0.17, 0.23, 0.11);
	transform-origin: top center;
}

.popup-survey-enter-from,
.popup-survey-leave-to {
	opacity: 0;
	transform: translateY(10rem) scale(0.8) scaleY(1.6);
}

@media (prefers-reduced-motion: no-preference) {
	.nav-button-animated-enter-active {
		transition: all 0.5s cubic-bezier(0.15, 1.4, 0.64, 0.96);
	}

	.nav-button-animated-leave-active {
		transition: all 0.25s ease;
	}

	.nav-button-animated-enter-active {
		position: relative;
	}

	.nav-button-animated-enter-active::before {
		content: '';
		inset: 0;
		border-radius: 100vw;
		background-color: var(--color-brand-highlight);
		position: absolute;
		animation: pop 0.5s ease-in forwards;
		opacity: 0;
	}

	@keyframes pop {
		0% {
			scale: 0.5;
		}
		50% {
			opacity: 0.5;
		}
		100% {
			scale: 1.5;
		}
	}

	.nav-button-animated-enter-from {
		scale: 0.5;
		translate: -2rem 0;
		opacity: 0;
	}

	.nav-button-animated-leave-to {
		scale: 0.75;
		opacity: 0;
	}

	.fade-enter-active {
		transition: 0.25s ease-in-out;
	}

	.fade-enter-from {
		opacity: 0;
	}
}
</style>
<style>
.os-theme-dark,
.os-theme-light {
	--os-handle-bg: var(--color-scrollbar) !important;
	--os-handle-bg-hover: var(--color-scrollbar) !important;
	--os-handle-bg-active: var(--color-scrollbar) !important;
}

.mac {
	.app-grid-statusbar {
		padding-left: 5rem;
	}
}

.windows {
	.fake-appbar {
		height: 2.5rem !important;
	}

	.info-card {
		right: 22rem;
	}

	.profile-card {
		right: 8rem;
	}
}

body.modrinth-console-fullscreen-active .app-contents {
	z-index: auto !important;
}

body.modrinth-console-fullscreen-active .app-grid-navbar,
body.modrinth-console-fullscreen-active .app-grid-statusbar,
body.modrinth-console-fullscreen-active .app-sidebar {
	visibility: hidden;
}
</style>
