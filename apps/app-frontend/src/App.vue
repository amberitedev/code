<script setup>
/**
 * App shell.
 * - Left nav slider state (227) measures and animates the app rail selected background.
 * - setupApp (671) initializes settings, auth, state, update, and provider wiring.
 * - clearMinimizedInstancePullState (892) keeps the pull-down snapshot cleanup centralized.
 * - Route guards (1018) manage the global loading bar, route motion, and page analytics.
 * - Suspense handlers (1078) bridge route component loading into the shared loading state.
 * - RouterView template (2258) renders route content inside the app shell.
 */
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
	SpinnerIcon,
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
	CoreHostingBackend,
	defineMessages,
	I18nDebugPanel,
	LoadingBar,
	NewModal,
	NotificationPanel,
	OverflowMenu,
	PopupNotificationPanel,
	provideHostingBackend,
	provideModalBehavior,
	provideModrinthClient,
	provideNotificationManager,
	providePageContext,
	providePopupNotificationManager,
	UI_MOTION_NAV_TABS_FADE_DELAY_MS,
	UI_MOTION_NAV_TABS_FADE_EASING,
	UI_MOTION_NAV_TABS_FADE_MS,
	UI_MOTION_NAV_TABS_SLIDER_EASING,
	UI_MOTION_NAV_TABS_SLIDER_MS,
	UiMotionTransition,
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
import { computed, nextTick, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'

import ModrinthAppLogo from '@/assets/modrinth_app.svg?component'
import AccountsCard from '@/components/ui/AccountsCard.vue'
import AppActionBar from '@/components/ui/AppActionBar.vue'
import Breadcrumbs from '@/components/ui/Breadcrumbs.vue'
import InstanceCreationFlowModal from '@/components/ui/creation-flow/InstanceCreationFlowModal.vue'
import ErrorModal from '@/components/ui/ErrorModal.vue'
import FriendsList from '@/components/ui/friends/FriendsList.vue'
import AddServerToInstanceModal from '@/components/ui/install_flow/AddServerToInstanceModal.vue'
import UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import LibraryInstancePullSurface from '@/components/ui/LibraryInstancePullSurface.vue'
import MinecraftAuthErrorModal from '@/components/ui/minecraft-auth-error-modal/MinecraftAuthErrorModal.vue'
import MinecraftRequiredModal from '@/components/ui/minecraft-required-modal/MinecraftRequiredModal.vue'
import AppSettingsModal from '@/components/ui/modal/AppSettingsModal.vue'
import InstallToPlayModal from '@/components/ui/modal/InstallToPlayModal.vue'
import ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import ModrinthAccountRequiredModal from '@/components/ui/modal/ModrinthAccountRequiredModal.vue'
import UpdateToPlayModal from '@/components/ui/modal/UpdateToPlayModal.vue'
import NavButton from '@/components/ui/NavButton.vue'
import PrideFundraiserBanner from '@/components/ui/PrideFundraiserBanner.vue'
import PromotionWrapper from '@/components/ui/PromotionWrapper.vue'
import QuickInstanceSwitcher from '@/components/ui/QuickInstanceSwitcher.vue'
import SharedInstanceInviteHandler from '@/components/ui/shared-instances/shared-instance-invite-handler/index.vue'
import SplashScreen from '@/components/ui/SplashScreen.vue'
import WindowControls from '@/components/ui/WindowControls.vue'
import { useCheckDisableMouseover } from '@/composables/macCssFix.js'
import { useAmberiteAuth } from '@/composables/useAmberiteAuth'
import { startCoreMonitor, useCoreClient } from '@/composables/useCoreClient'
import { preloadDiscoverContentQueries } from '@/composables/useDiscoverContentPreload'
import { useModrinthLink } from '@/composables/useModrinthLink'
import { useSocialClient } from '@/composables/useSocialClient'
import { config } from '@/config'
import {
	ads_consent_listener,
	hide_ads_window,
	init_ads_window,
	perform_ads_consent_action,
	should_show_ads_consent_popup,
	show_ads_window,
} from '@/helpers/ads.js'
import { debugAnalytics, initAnalytics, trackEvent } from '@/helpers/analytics'
import { check_reachable } from '@/helpers/auth.js'
import { get_user, get_version } from '@/helpers/cache.js'
import { command_listener, notification_listener, warning_listener } from '@/helpers/events.js'
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
import { start_join_server, start_join_singleplayer_world } from '@/helpers/worlds.ts'
import i18n from '@/i18n.config'
import LibraryPage from '@/pages/library/Index.vue'
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
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useError } from '@/store/error.js'
import { useTheming } from '@/store/state'

import { generateSkinPreviews } from './helpers/rendering/batch-skin-renderer'
import { get_available_capes, get_available_skins } from './helpers/skins'
import { AppNotificationManager } from './providers/app-notifications'
import { AppPopupNotificationManager } from './providers/app-popup-notifications'

const themeStore = useTheming()
const router = useRouter()
const route = useRoute()
const breadcrumbData = useBreadcrumbs()
const APP_LEFT_NAV_WIDTH = '4rem'
const APP_SIDEBAR_WIDTH = 300
const INTERCOM_BUBBLE_DEFAULT_PADDING = 20
const LEFT_NAV_SLIDER_STUTTER_MS = 15
const amberiteAuth = useAmberiteAuth()
const coreClient = useCoreClient()
const socialClient = useSocialClient()
const modrinthLink = useModrinthLink()
const PRIDE_FUNDRAISER_END_DATE = new Date('2026-07-01T00:00:00Z').getTime()
const hasMinecraftAccounts = ref(false)
const canUseAmberiteFeatures = computed(() => amberiteAuth.status.value === 'authenticated')
const AMBERITE_ACCOUNT_DISMISS_KEY = 'amberite:account-modal:dismissed'
const amberiteAccountDismissedForSession = ref(false)
try {
	amberiteAccountDismissedForSession.value =
		window.sessionStorage.getItem(AMBERITE_ACCOUNT_DISMISS_KEY) === 'true'
} catch {
	// sessionStorage can be unavailable in tests.
}
const amberiteAccountModalStep = ref('main')
const showAmberiteAccountModal = computed(() => false)
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

const leftNav = ref(null)
const leftNavSliderReady = ref(false)
const leftNavSliderTransitionsEnabled = ref(false)
const leftNavSliderSubpageSelected = ref(false)
const leftNavSliderPosition = ref({ left: 0, top: 0, right: 0, bottom: 0 })
const leftNavSliderDelays = ref({ left: '0ms', top: '0ms', right: '0ms', bottom: '0ms' })
const LEFT_NAV_OPTIMISTIC_ACTIVE_CLASS = 'left-nav-optimistic-active'
const leftNavSliderStyle = computed(() => ({
	left: `${leftNavSliderPosition.value.left}px`,
	top: `${leftNavSliderPosition.value.top}px`,
	right: `${leftNavSliderPosition.value.right}px`,
	bottom: `${leftNavSliderPosition.value.bottom}px`,
	opacity: leftNavSliderReady.value ? 1 : 0,
	'--left-nav-fade-delay-ms': `${UI_MOTION_NAV_TABS_FADE_DELAY_MS}ms`,
	'--left-nav-fade-easing': UI_MOTION_NAV_TABS_FADE_EASING,
	'--left-nav-fade-ms': `${UI_MOTION_NAV_TABS_FADE_MS}ms`,
	'--left-nav-left-delay': leftNavSliderDelays.value.left,
	'--left-nav-right-delay': leftNavSliderDelays.value.right,
	'--left-nav-top-delay': leftNavSliderDelays.value.top,
	'--left-nav-bottom-delay': leftNavSliderDelays.value.bottom,
	'--left-nav-slider-easing': UI_MOTION_NAV_TABS_SLIDER_EASING,
	'--left-nav-slider-ms': `${UI_MOTION_NAV_TABS_SLIDER_MS}ms`,
}))

let leftNavResizeObserver = null
let leftNavMutationObserver = null
let leftNavRemeasureFrame = null

function cancelLeftNavRemeasureFrame() {
	if (leftNavRemeasureFrame === null) return

	cancelAnimationFrame(leftNavRemeasureFrame)
	leftNavRemeasureFrame = null
}

function scheduleLeftNavSliderRemeasure() {
	if (typeof window === 'undefined') return

	cancelLeftNavRemeasureFrame()
	leftNavRemeasureFrame = requestAnimationFrame(() => {
		leftNavRemeasureFrame = null
		updateLeftNavSlider()
	})
}

function disconnectLeftNavLayoutObservers() {
	leftNavResizeObserver?.disconnect()
	leftNavResizeObserver = null
	leftNavMutationObserver?.disconnect()
	leftNavMutationObserver = null
}

function getLeftNavButtons() {
	const nav = leftNav.value
	if (!nav) return []

	return Array.from(nav.querySelectorAll('.app-nav-button'))
}

function clearOptimisticLeftNavSelection() {
	getLeftNavButtons().forEach((button) => button.classList.remove(LEFT_NAV_OPTIMISTIC_ACTIVE_CLASS))
}

function setOptimisticLeftNavSelection(button) {
	clearOptimisticLeftNavSelection()
	button.classList.add(LEFT_NAV_OPTIMISTIC_ACTIVE_CLASS)
}

function setupLeftNavLayoutObservers() {
	if (typeof window === 'undefined') return

	disconnectLeftNavLayoutObservers()

	const nav = leftNav.value
	if (!nav) return

	if (typeof ResizeObserver !== 'undefined') {
		leftNavResizeObserver = new ResizeObserver(scheduleLeftNavSliderRemeasure)
		leftNavResizeObserver.observe(nav)
		getLeftNavButtons().forEach((button) => leftNavResizeObserver?.observe(button))
	}

	if (typeof MutationObserver !== 'undefined') {
		leftNavMutationObserver = new MutationObserver(() => {
			setupLeftNavLayoutObservers()
			scheduleLeftNavSliderRemeasure()
		})
		leftNavMutationObserver.observe(nav, {
			childList: true,
			subtree: true,
		})
	}
}

function getLeftNavLibraryButton() {
	const nav = leftNav.value
	if (!nav) return null

	return nav.querySelector('[data-left-nav-library]')
}

function getRouteActiveLeftNavButton() {
	const nav = leftNav.value
	if (!nav) return null

	return (
		nav.querySelector(`.app-nav-button.${LEFT_NAV_OPTIMISTIC_ACTIVE_CLASS}:not(.disabled)`) ??
		nav.querySelector('.app-nav-button.router-link-active:not(.disabled)') ??
		nav.querySelector('.app-nav-button.subpage-active:not(.disabled)')
	)
}

function getLeftNavInstanceButtonForRoutePath(path) {
	if (!path) return null

	let targetPath = ''
	try {
		targetPath = router.resolve(path).path
	} catch {
		return null
	}

	return (
		getLeftNavButtons().find(
			(button) => button.getAttribute('data-left-nav-instance-route') === targetPath,
		) ?? null
	)
}

function getLibraryInstanceLeftNavMotionButton() {
	if (libraryInstancePullBreadcrumbMode.value === 'restore') {
		return (
			getLeftNavInstanceButtonForRoutePath(minimizedInstanceRoutePath.value) ??
			getRouteActiveLeftNavButton()
		)
	}

	return getRouteActiveLeftNavButton()
}

function getLibraryInstanceLeftNavScrubProgress() {
	if (!['dismiss', 'restore', 'open', 'close'].includes(libraryInstancePullBreadcrumbMode.value)) {
		return null
	}

	return Math.min(1, Math.max(0, libraryInstancePullBreadcrumbProgress.value))
}

function getActiveLeftNavButton() {
	if (libraryInstanceLeftNavSelectionFrozen.value) {
		const libraryButton = getLeftNavLibraryButton()
		if (libraryButton) return libraryButton
	}

	return getRouteActiveLeftNavButton()
}

function getLeftNavSliderPosition(button) {
	const nav = leftNav.value
	if (!nav) return null

	return {
		left: button.offsetLeft,
		top: button.offsetTop,
		right: nav.offsetWidth - button.offsetLeft - button.offsetWidth,
		bottom: nav.offsetHeight - button.offsetTop - button.offsetHeight,
	}
}

function isSameLeftNavSliderPosition(position) {
	return (
		position.left === leftNavSliderPosition.value.left &&
		position.right === leftNavSliderPosition.value.right &&
		position.top === leftNavSliderPosition.value.top &&
		position.bottom === leftNavSliderPosition.value.bottom
	)
}

function applyLeftNavSliderPosition(position) {
	leftNavSliderPosition.value = position
}

function setLeftNavSliderDelaysToZero() {
	leftNavSliderDelays.value = { left: '0ms', top: '0ms', right: '0ms', bottom: '0ms' }
}

function restoreLeftNavSliderTransitionsAfterScrub() {
	requestAnimationFrame(() => {
		if (getLibraryInstanceLeftNavScrubProgress() === null) {
			leftNavSliderTransitionsEnabled.value = true
		}
	})
}

function animateLeftNavSliderTo(position) {
	const staggerDelay = `${LEFT_NAV_SLIDER_STUTTER_MS}ms`
	const currentPosition = leftNavSliderPosition.value

	leftNavSliderDelays.value = {
		left: position.left < currentPosition.left ? '0ms' : staggerDelay,
		right: position.left < currentPosition.left ? staggerDelay : '0ms',
		top: position.top < currentPosition.top ? '0ms' : staggerDelay,
		bottom: position.top < currentPosition.top ? staggerDelay : '0ms',
	}

	applyLeftNavSliderPosition(position)
}

function interpolateLeftNavSliderPosition(from, to, progress) {
	const interpolate = (start, end) => start + (end - start) * progress

	return {
		left: interpolate(from.left, to.left),
		top: interpolate(from.top, to.top),
		right: interpolate(from.right, to.right),
		bottom: interpolate(from.bottom, to.bottom),
	}
}

function positionLeftNavSliderAt(button, subpageSelected) {
	const position = getLeftNavSliderPosition(button)
	if (!position) return

	leftNavSliderSubpageSelected.value = subpageSelected

	if (!leftNavSliderReady.value) {
		applyLeftNavSliderPosition(position)
		leftNavSliderReady.value = true

		restoreLeftNavSliderTransitionsAfterScrub()
	} else if (!isSameLeftNavSliderPosition(position)) {
		if (!leftNavSliderTransitionsEnabled.value) {
			restoreLeftNavSliderTransitionsAfterScrub()
		}
		animateLeftNavSliderTo(position)
	} else if (!leftNavSliderTransitionsEnabled.value) {
		restoreLeftNavSliderTransitionsAfterScrub()
	}
}

function scrubLeftNavSliderForLibraryInstanceMotion() {
	const progress = getLibraryInstanceLeftNavScrubProgress()
	if (progress === null) return false

	const instanceButton = getLibraryInstanceLeftNavMotionButton()
	const libraryButton = getLeftNavLibraryButton()
	if (!instanceButton || !libraryButton) return false

	const instancePosition = getLeftNavSliderPosition(instanceButton)
	const libraryPosition = getLeftNavSliderPosition(libraryButton)
	if (!instancePosition || !libraryPosition) return false

	leftNavSliderSubpageSelected.value =
		progress < 0.98 &&
		instanceButton.classList.contains('subpage-active') &&
		!instanceButton.classList.contains('router-link-active')
	leftNavSliderTransitionsEnabled.value = false
	setLeftNavSliderDelaysToZero()
	applyLeftNavSliderPosition(
		interpolateLeftNavSliderPosition(instancePosition, libraryPosition, progress),
	)
	leftNavSliderReady.value = true
	return true
}

function updateLeftNavSlider() {
	if (scrubLeftNavSliderForLibraryInstanceMotion()) return

	const activeButton = getActiveLeftNavButton()
	if (!activeButton) {
		leftNavSliderReady.value = false
		leftNavSliderTransitionsEnabled.value = false
		return
	}

	positionLeftNavSliderAt(
		activeButton,
		activeButton.classList.contains('subpage-active') &&
			!activeButton.classList.contains('router-link-active'),
	)
}

function isPrimaryLeftNavPointerIntent(event) {
	return !(
		event.defaultPrevented ||
		event.button !== 0 ||
		event.metaKey ||
		event.altKey ||
		event.ctrlKey ||
		event.shiftKey
	)
}

function handleLeftNavPointerIntent(event) {
	if (!isPrimaryLeftNavPointerIntent(event)) return

	const target = event.target instanceof Element ? event.target : null
	const button = target?.closest('.app-nav-button')
	if (!button || !leftNav.value?.contains(button)) return
	if (button.tagName !== 'A' || button.classList.contains('disabled')) return

	setOptimisticLeftNavSelection(button)
	if (
		button.hasAttribute('data-left-nav-instance-route') &&
		!isInstanceRoute(route.path) &&
		canShowLibraryInstanceMotion.value
	) {
		leftNavSliderTransitionsEnabled.value = false
		setLeftNavSliderDelaysToZero()
		return
	}

	positionLeftNavSliderAt(button, false)
}

async function setupLeftNavSlider() {
	await nextTick()
	setupLeftNavLayoutObservers()
	scheduleLeftNavSliderRemeasure()
}

watch(
	() => route.fullPath,
	async () => {
		await nextTick()
		clearOptimisticLeftNavSelection()
		scheduleLeftNavSliderRemeasure()
	},
)

const notificationManager = new AppNotificationManager()
provideNotificationManager(notificationManager)
const { handleError, addNotification } = notificationManager

const popupNotificationManager = new AppPopupNotificationManager()
providePopupNotificationManager(popupNotificationManager)
const { addPopupNotification } = popupNotificationManager
let adsConsentPopupId = null
let unlistenAdsConsent

const appVersion = getVersion()
const tauriApiClient = new TauriModrinthClient({
	userAgent: async () => `modrinth/theseus/${await appVersion} (support@modrinth.com)`,
	labrinthBaseUrl: config.labrinthBaseUrl,
	archonBaseUrl: config.archonBaseUrl,
	sharedInstancesBaseUrl: config.sharedInstancesBaseUrl,
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
provideHostingBackend(
	new CoreHostingBackend(coreClient, {
		searchUsers: async (query) => {
			const users = await socialClient.searchUsers(query).catch(() => [])
			return users.map((user) => ({
				id: user.userId,
				username: user.username ?? user.displayName ?? user.userId,
				avatarUrl: user.image,
			}))
		},
	}),
)
useQuery({
	queryKey: computed(() => ['authenticated-user', 'campaigns', modrinthLink.user.value?.id]),
	queryFn: () => tauriApiClient.labrinth.users_v3.getAuthenticated(),
	enabled: () => modrinthLink.linked.value,
	retry: false,
})
useQuery({
	queryKey: computed(() => ['shared-instance-eligibility', credentials.value?.user?.id]),
	queryFn: can_current_user_use_shared_instances,
	enabled: () => !!credentials.value?.session && !!credentials.value?.user?.id,
	retry: false,
	staleTime: Infinity,
	refetchOnMount: false,
	refetchOnWindowFocus: false,
	refetchOnReconnect: false,
})
const hasPlus = computed(
	() =>
		!!credentials.value?.user &&
		(hasMidasBadge(credentials.value.user) ||
			hasActivePride26Midas(authenticatedModrinthUser.value?.campaigns?.pride_26)),
)
const showAd = computed(
	() => sidebarVisible.value && !hasPlus.value && credentials.value !== undefined,
)
const adConsentAvailable = computed(() => credentials.value !== undefined && !hasPlus.value)
providePageContext({
	hierarchicalSidebarAvailable: ref(true),
	showAds: showAd,
	adConsentAvailable,
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
	initializeProviders,
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
const serverInvitePopupNotificationIds = new Set()
let liveNotificationGeneration = 0
let liveNotificationsEnabled = true

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
watch(stateInitialized, async (ready) => {
	if (!ready) {
		leftNavSliderReady.value = false
		leftNavSliderTransitionsEnabled.value = false
		disconnectLeftNavLayoutObservers()
		return
	}

	await setupLeftNavSlider()
})

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
	enabled: () => stateInitialized.value,
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
	try {
		unlistenAdsConsent = await ads_consent_listener(handleAdsConsentRequired)
		handleAdsConsentRequired(await should_show_ads_consent_popup())
	} catch (error) {
		handleError(error)
	}

	document.querySelector('body').addEventListener('click', handleClick)
	document.querySelector('body').addEventListener('auxclick', handleAuxClick)

	if (stateInitialized.value) {
		await setupLeftNavSlider()
	}

	window.addEventListener('resize', scheduleLeftNavSliderRemeasure)

	if (document.fonts) {
		void document.fonts.ready.then(scheduleLeftNavSliderRemeasure).catch(() => undefined)
	}
})

onUnmounted(async () => {
	document.querySelector('body').removeEventListener('click', handleClick)
	document.querySelector('body').removeEventListener('auxclick', handleAuxClick)
	unsubscribeSidebarToggle()
	clearDelayedUpdatePopup()
	cancelLeftNavRemeasureFrame()
	cancelLibraryInstanceRestoreOverlayFrame()
	disconnectLibraryInstanceRouteContentObserver()
	disconnectLeftNavLayoutObservers()
	window.removeEventListener('resize', scheduleLeftNavSliderRemeasure)

	await unlistenAdsConsent?.()
	await unlistenUpdateDownload?.()
})

let updatesCheckStarted = false
watch(stateInitialized, (ready) => {
	if (!ready || updatesCheckStarted) return

	updatesCheckStarted = true
	void checkUpdates()
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

function handleAdsConsentRequired(required) {
	if (!required) {
		if (adsConsentPopupId !== null) {
			popupNotificationManager.removeNotification(adsConsentPopupId)
			adsConsentPopupId = null
		}
		return
	}

	if (
		adsConsentPopupId !== null &&
		popupNotificationManager.getNotifications().some((item) => item.id === adsConsentPopupId)
	) {
		return
	}

	const notification = addPopupNotification({
		title: formatMessage(messages.adsConsentTitle),
		text: formatMessage(messages.adsConsentBody),
		type: 'info',
		hideIcon: true,
		autoCloseMs: null,
		dismissible: false,
		buttons: [
			{
				label: formatMessage(messages.adsConsentManage),
				action: () => perform_ads_consent_action('manage').catch(handleError),
				color: 'standard',
				keepOpen: true,
			},
			{
				label: formatMessage(messages.adsConsentReject),
				action: () => perform_ads_consent_action('reject').catch(handleError),
				color: 'brand',
				keepOpen: true,
			},
			{
				label: formatMessage(messages.adsConsentAccept),
				action: () => perform_ads_consent_action('accept').catch(handleError),
				color: 'brand',
				keepOpen: true,
			},
		],
	})

	adsConsentPopupId = notification.id
}

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
	await amberiteAuth.initialize()
	startCoreMonitor()
	hasMinecraftAccounts.value = amberiteAuth.hasMinecraftAccess.value
	stateInitialized.value = true
	void initializeProviders()

	isMaximized.value = await getCurrentWindow().isMaximized()

	await getCurrentWindow().onResized(async () => {
		isMaximized.value = await getCurrentWindow().isMaximized()
	})

	if (telemetry) {
		initAnalytics()
		debugAnalytics()
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
			type: 'warning',
		}),
	)

	void fetchCriticalAnnouncement(version, dev)

	get_opening_command().then(handleCommand)
	void modrinthLink.refresh()

	if (amberiteAuth.hasMinecraftAccess.value) {
		try {
			const skins = (await get_available_skins()) ?? []
			const capes = (await get_available_capes()) ?? []
			generateSkinPreviews(skins, capes)
		} catch {
			console.warn('Minecraft skin previews are unavailable.')
		}
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
let pendingRouteMotion = null
let forcedNextRouteMotion = null

let suspensePending = false
const routeMotion = ref({
	enabled: false,
	key: 'initial-route',
	direction: 'down',
	type: 'slide',
})
const suppressLocalRouteLoadingBar = ref(false)
provide('suppressLocalRouteLoadingBar', suppressLocalRouteLoadingBar)
const suppressNextLibraryInstanceRouteMotion = ref(false)
const lastLibraryRoutePath = ref('/library')
const minimizedInstanceRoutePath = ref('')
const minimizedInstanceSnapshotHtml = ref('')
const minimizedInstanceBreadcrumbs = ref(null)
const libraryInstancePullBreadcrumbProgress = ref(0)
const libraryInstancePullBreadcrumbMode = ref('none')
const restoringLibraryInstanceRoute = ref(false)
const libraryInstanceProgrammaticClose = ref(false)
const libraryInstanceOpening = ref(false)
const libraryInstanceOpeningFromMinimized = ref(false)
const libraryInstanceRouteReady = ref(false)
const libraryInstanceDragDisabled = ref(false)
const libraryInstanceOpenAnimationKey = ref(0)
const libraryInstanceCloseAnimationKey = ref(0)
const libraryInstanceOpenStartedForPath = ref('')
const libraryInstanceOpenAnimationComplete = ref(false)
const libraryInstanceCloseTarget = ref('')
let libraryInstanceRestoreOverlayFrame = null
let libraryInstanceRouteContentObserver = null
let libraryInstanceRouteContentObserverTarget = null
let libraryInstanceRouteContentObserverFrame = null
const canShowLibraryInstanceMotion = computed(
	() => stateInitialized.value && !stateFailed.value && !criticalErrorMessage.value,
)
const libraryInstanceBackToLastLibraryRoute = async () => {
	const target = lastLibraryRoutePath.value || '/library'
	forcedNextRouteMotion = createRouteSlideMotion(target, 'right')
	await router.push(target)
}
provide('libraryInstanceBackToLastLibraryRoute', libraryInstanceBackToLastLibraryRoute)
provide('beginLibraryInstanceOpenNavigation', beginLibraryInstanceOpenNavigation)
const isLibraryInstancePullActive = computed(
	() =>
		canShowLibraryInstanceMotion.value &&
		isInstanceRoute(route.path) &&
		libraryInstanceRouteReady.value &&
		!libraryInstanceDragDisabled.value,
)
const showLibraryInstanceRestoreNotch = computed(
	() =>
		canShowLibraryInstanceMotion.value &&
		!!minimizedInstanceRoutePath.value &&
		((isLibraryRoute(route.path) && !libraryInstanceOpenStartedForPath.value) ||
			restoringLibraryInstanceRoute.value ||
			libraryInstanceOpeningFromMinimized.value),
)
const libraryBreadcrumbPreview = Object.freeze([{ name: 'Library' }])
const libraryInstancePullPreviewBreadcrumbs = computed(() =>
	libraryInstancePullBreadcrumbMode.value === 'restore'
		? (minimizedInstanceBreadcrumbs.value ?? undefined)
		: isInstanceRoute(route.path) ||
			  libraryInstancePullBreadcrumbMode.value === 'open' ||
			  libraryInstancePullBreadcrumbMode.value === 'close' ||
			  libraryInstancePullBreadcrumbProgress.value > 0
			? libraryBreadcrumbPreview
			: undefined,
)
const libraryInstancePullBreadcrumbPreviewProgress = computed(() =>
	libraryInstancePullBreadcrumbMode.value === 'restore'
		? 1 - libraryInstancePullBreadcrumbProgress.value
		: libraryInstancePullBreadcrumbProgress.value,
)
const libraryInstanceLeftNavSelectionFrozen = computed(
	() =>
		libraryInstanceProgrammaticClose.value ||
		libraryInstancePullBreadcrumbMode.value === 'dismiss' ||
		libraryInstancePullBreadcrumbMode.value === 'open' ||
		libraryInstancePullBreadcrumbMode.value === 'restore' ||
		libraryInstancePullBreadcrumbMode.value === 'close',
)

watch([libraryInstanceLeftNavSelectionFrozen, libraryInstancePullBreadcrumbProgress], () => {
	scheduleLeftNavSliderRemeasure()
})

const sidebarOverlayScrollbarsOptions = Object.freeze({
	overflow: {
		x: 'hidden',
		y: 'scroll',
	},
})

function isBrowseRoute(path) {
	return path.startsWith('/browse')
}

function isLibraryRoute(path) {
	return path === '/library' || path.startsWith('/library/')
}

function isInstanceRoute(path) {
	return path.startsWith('/instance/')
}

function cloneBreadcrumbs(breadcrumbs) {
	return breadcrumbs.map((breadcrumb) => ({
		...breadcrumb,
		query: breadcrumb.query ? { ...breadcrumb.query } : breadcrumb.query,
	}))
}

function getCurrentRouteBreadcrumbs() {
	const additionalContext =
		route.meta.useContext === true
			? breadcrumbData.context
			: route.meta.useRootContext === true
				? breadcrumbData.rootContext
				: null
	const crumbs = route.meta.breadcrumb ?? []
	return cloneBreadcrumbs(additionalContext ? [additionalContext, ...crumbs] : crumbs)
}

function createRouteSlideMotion(target, direction = 'left') {
	const resolved = router.resolve(target)
	return {
		enabled: true,
		key: `route-slide:${resolved.fullPath || resolved.path}:${Date.now()}`,
		direction,
		type: 'slide',
	}
}

function beginLibraryInstanceOpenNavigation(target) {
	if (!canShowLibraryInstanceMotion.value || isInstanceRoute(route.path)) return

	const resolved = router.resolve(target)
	if (!isInstanceRoute(resolved.path)) return

	const opensInstanceFromLibrary = isLibraryRoute(route.path)
	const opensInstanceFromMinimized = opensInstanceFromLibrary && !!minimizedInstanceRoutePath.value

	if (opensInstanceFromLibrary) {
		lastLibraryRoutePath.value = route.fullPath || route.path
	}

	if (opensInstanceFromMinimized) {
		restoringLibraryInstanceRoute.value = false
		libraryInstanceProgrammaticClose.value = false
		libraryInstanceOpeningFromMinimized.value = true
	} else {
		clearMinimizedInstancePullState()
		libraryInstanceOpeningFromMinimized.value = false
	}

	libraryInstanceOpening.value = true
	libraryInstanceRouteReady.value = false
	libraryInstanceDragDisabled.value = false
	libraryInstanceOpenAnimationComplete.value = false
	libraryInstancePullBreadcrumbProgress.value = 1
	libraryInstancePullBreadcrumbMode.value = 'open'
}

function captureInstancePullSnapshot() {
	const routeContent = document.querySelector('[data-library-instance-route-content]')
	if (routeContent instanceof HTMLElement) {
		minimizedInstanceSnapshotHtml.value = routeContent.innerHTML
	}
	minimizedInstanceBreadcrumbs.value = getCurrentRouteBreadcrumbs()
}

function clearMinimizedInstanceSnapshot() {
	minimizedInstanceRoutePath.value = ''
	minimizedInstanceSnapshotHtml.value = ''
	minimizedInstanceBreadcrumbs.value = null
}

function clearMinimizedInstancePullState() {
	cancelLibraryInstanceRestoreOverlayFrame()
	clearMinimizedInstanceSnapshot()
	libraryInstancePullBreadcrumbProgress.value = 0
	libraryInstancePullBreadcrumbMode.value = 'none'
	restoringLibraryInstanceRoute.value = false
	libraryInstanceProgrammaticClose.value = false
	libraryInstanceOpening.value = false
	libraryInstanceOpeningFromMinimized.value = false
	libraryInstanceDragDisabled.value = false
	libraryInstanceOpenStartedForPath.value = ''
	libraryInstanceOpenAnimationComplete.value = false
	libraryInstanceCloseTarget.value = ''
}

function cancelLibraryInstanceRestoreOverlayFrame() {
	if (libraryInstanceRestoreOverlayFrame === null) return

	cancelAnimationFrame(libraryInstanceRestoreOverlayFrame)
	libraryInstanceRestoreOverlayFrame = null
}

function cancelLibraryInstanceRouteContentObserverFrame() {
	if (libraryInstanceRouteContentObserverFrame === null) return

	cancelAnimationFrame(libraryInstanceRouteContentObserverFrame)
	libraryInstanceRouteContentObserverFrame = null
}

function disconnectLibraryInstanceRouteContentObserver() {
	cancelLibraryInstanceRouteContentObserverFrame()
	libraryInstanceRouteContentObserver?.disconnect()
	libraryInstanceRouteContentObserver = null
	libraryInstanceRouteContentObserverTarget = null
}

function scheduleLibraryInstanceRouteContentRefresh() {
	if (libraryInstanceRouteContentObserverFrame !== null) return

	libraryInstanceRouteContentObserverFrame = requestAnimationFrame(() => {
		libraryInstanceRouteContentObserverFrame = null
		void refreshLibraryInstanceRouteReady()
	})
}

function syncLibraryInstanceRouteContentObserver() {
	if (
		typeof window === 'undefined' ||
		typeof MutationObserver === 'undefined' ||
		!isInstanceRoute(route.path)
	) {
		disconnectLibraryInstanceRouteContentObserver()
		return
	}

	const routeContent = document.querySelector('[data-library-instance-route-content]')
	if (!routeContent) {
		disconnectLibraryInstanceRouteContentObserver()
		return
	}

	if (libraryInstanceRouteContentObserverTarget === routeContent) return

	disconnectLibraryInstanceRouteContentObserver()
	libraryInstanceRouteContentObserverTarget = routeContent
	libraryInstanceRouteContentObserver = new MutationObserver(
		scheduleLibraryInstanceRouteContentRefresh,
	)
	libraryInstanceRouteContentObserver.observe(routeContent, {
		childList: true,
		subtree: true,
		attributes: true,
		attributeFilter: [
			'data-library-instance-page-ready',
			'data-library-instance-drag-disabled',
			'data-library-instance-error-state',
		],
	})
}

function queueLibraryInstanceRestoreOverlayClear() {
	if (!restoringLibraryInstanceRoute.value) return
	if (typeof window === 'undefined') {
		clearMinimizedInstancePullState()
		return
	}

	cancelLibraryInstanceRestoreOverlayFrame()
	libraryInstanceRestoreOverlayFrame = requestAnimationFrame(() => {
		libraryInstanceRestoreOverlayFrame = requestAnimationFrame(() => {
			libraryInstanceRestoreOverlayFrame = null
			if (restoringLibraryInstanceRoute.value && isInstanceRoute(route.path)) {
				clearMinimizedInstancePullState()
			}
		})
	})
}

function hasReadyLibraryInstanceRouteContent() {
	if (!isInstanceRoute(route.path) || !canShowLibraryInstanceMotion.value) return false

	const routeContent = document.querySelector('[data-library-instance-route-content]')
	return !!routeContent?.querySelector('[data-library-instance-page-ready]')
}

function hasDragDisabledLibraryInstanceRouteContent() {
	if (!isInstanceRoute(route.path) || !canShowLibraryInstanceMotion.value) return false

	const routeContent = document.querySelector('[data-library-instance-route-content]')
	return !!routeContent?.querySelector('[data-library-instance-drag-disabled]')
}

async function refreshLibraryInstanceRouteReady() {
	await nextTick()
	syncLibraryInstanceRouteContentObserver()
	libraryInstanceRouteReady.value = hasReadyLibraryInstanceRouteContent()
	libraryInstanceDragDisabled.value = hasDragDisabledLibraryInstanceRouteContent()
	if (finishLibraryInstanceOpenAnimationIfReady()) return
	maybeStartLibraryInstanceOpenAnimation()
}

function finishLibraryInstanceOpenAnimationIfReady() {
	if (
		!libraryInstanceOpening.value ||
		!libraryInstanceOpenAnimationComplete.value ||
		!libraryInstanceRouteReady.value ||
		!isInstanceRoute(route.path)
	) {
		return false
	}

	libraryInstancePullBreadcrumbProgress.value = 0
	libraryInstancePullBreadcrumbMode.value = 'none'
	libraryInstanceProgrammaticClose.value = false
	libraryInstanceOpening.value = false
	libraryInstanceOpeningFromMinimized.value = false
	clearMinimizedInstanceSnapshot()
	return true
}

function maybeStartLibraryInstanceOpenAnimation(path = route.fullPath, requireReady = true) {
	if (
		!libraryInstanceOpening.value ||
		(requireReady && !libraryInstanceRouteReady.value) ||
		restoringLibraryInstanceRoute.value ||
		libraryInstanceOpenStartedForPath.value === path
	) {
		return
	}

	libraryInstanceOpenStartedForPath.value = path
	libraryInstanceOpenAnimationComplete.value = false
	libraryInstanceOpenAnimationKey.value += 1
}

function getProjectRouteRoot(path) {
	const match = path.match(/^\/project\/[^/]+/)
	return match?.[0] ?? null
}

function getInstanceRouteRoot(path) {
	const match = path.match(/^\/instance\/[^/]+/)
	return match?.[0] ?? null
}

function getHostingManageRouteRoot(path) {
	const match = path.match(/^\/hosting\/manage\/[^/]+/)
	return match?.[0] ?? null
}

function getLocalTabRouteGroup(path) {
	if (isBrowseRoute(path)) return 'browse'
	if (path.startsWith('/library')) return 'library'

	return getProjectRouteRoot(path) ?? getInstanceRouteRoot(path) ?? getHostingManageRouteRoot(path)
}

function shouldSuppressLocalTabRouteLoading(to, from) {
	const toGroup = getLocalTabRouteGroup(to.path)
	const opensInstanceFromLibrary = isLibraryRoute(from.path) && isInstanceRoute(to.path)
	const returnsToLibraryFromInstance = isInstanceRoute(from.path) && isLibraryRoute(to.path)

	return (
		(!!toGroup && toGroup === getLocalTabRouteGroup(from.path)) ||
		opensInstanceFromLibrary ||
		returnsToLibraryFromInstance
	)
}

function shouldSuppressCurrentRouteLoading() {
	return suppressLocalRouteLoadingBar.value || !!getLocalTabRouteGroup(route.path)
}

function getStaticRouteMotion() {
	return {
		enabled: false,
		key: 'static-route',
		direction: 'middle',
		type: 'none',
	}
}

function getRouteMotion(to, from) {
	if (forcedNextRouteMotion) {
		const motion = forcedNextRouteMotion
		forcedNextRouteMotion = null
		return motion
	}

	if (suppressNextLibraryInstanceRouteMotion.value) {
		return getStaticRouteMotion()
	}

	const opensInstanceFromLibrary = isLibraryRoute(from.path) && isInstanceRoute(to.path)
	const returnsToLibraryFromInstance = isInstanceRoute(from.path) && isLibraryRoute(to.path)

	if (returnsToLibraryFromInstance) {
		return createRouteSlideMotion(to.fullPath || to.path, 'right')
	}

	if (opensInstanceFromLibrary) {
		return getStaticRouteMotion()
	}

	return getStaticRouteMotion()
}

router.beforeEach((to, from) => {
	suspensePending = false
	if (routerToken) loading.end(routerToken)
	const initialNavigation = from.matched.length === 0
	const opensInstanceRoute =
		!initialNavigation && !isInstanceRoute(from.path) && isInstanceRoute(to.path)
	const opensInstanceFromLibrary = isLibraryRoute(from.path) && isInstanceRoute(to.path)
	if (opensInstanceFromLibrary && !suppressNextLibraryInstanceRouteMotion.value) {
		lastLibraryRoutePath.value = from.fullPath || from.path
	}
	if (opensInstanceRoute && !suppressNextLibraryInstanceRouteMotion.value) {
		const targetFullPath = to.fullPath || to.path
		const openAlreadyStartedForTarget =
			libraryInstanceOpening.value && libraryInstanceOpenStartedForPath.value === targetFullPath
		if (!openAlreadyStartedForTarget) {
			beginLibraryInstanceOpenNavigation(targetFullPath)
		} else {
			libraryInstanceRouteReady.value = false
			libraryInstanceDragDisabled.value = false
			libraryInstancePullBreadcrumbProgress.value = 1
			libraryInstancePullBreadcrumbMode.value = 'open'
		}
	}
	if (!isLibraryRoute(to.path) && !isInstanceRoute(to.path)) {
		clearMinimizedInstancePullState()
	}
	pendingRouteMotion = getRouteMotion(to, from)
	suppressLocalRouteLoadingBar.value = shouldSuppressLocalTabRouteLoading(to, from)
	routerToken = suppressLocalRouteLoadingBar.value ? null : loading.begin()
})
router.afterEach((to, from, failure) => {
	routeMotion.value = failure
		? getStaticRouteMotion()
		: (pendingRouteMotion ?? getStaticRouteMotion())
	pendingRouteMotion = null
	if (failure) {
		clearOptimisticLeftNavSelection()
		scheduleLeftNavSliderRemeasure()
		suppressLocalRouteLoadingBar.value = false
		suppressNextLibraryInstanceRouteMotion.value = false
		if (libraryInstancePullBreadcrumbMode.value === 'open') {
			libraryInstancePullBreadcrumbProgress.value = 0
			libraryInstancePullBreadcrumbMode.value = 'none'
			libraryInstanceOpening.value = false
			libraryInstanceRouteReady.value = false
			libraryInstanceDragDisabled.value = false
			libraryInstanceOpenStartedForPath.value = ''
			libraryInstanceOpenAnimationComplete.value = false
		}
		libraryInstanceOpeningFromMinimized.value = false
		if (
			!libraryInstanceProgrammaticClose.value ||
			libraryInstancePullBreadcrumbMode.value !== 'close'
		) {
			libraryInstanceProgrammaticClose.value = false
		}
	} else {
		if (isLibraryRoute(to.path)) {
			disconnectLibraryInstanceRouteContentObserver()
			lastLibraryRoutePath.value = to.fullPath || to.path
			libraryInstancePullBreadcrumbProgress.value = 0
			libraryInstancePullBreadcrumbMode.value = 'none'
			libraryInstanceProgrammaticClose.value = false
			libraryInstanceOpening.value = false
			libraryInstanceOpeningFromMinimized.value = false
			libraryInstanceRouteReady.value = false
			libraryInstanceDragDisabled.value = false
			libraryInstanceOpenStartedForPath.value = ''
			libraryInstanceOpenAnimationComplete.value = false
			libraryInstanceCloseTarget.value = ''
		}
		if (isInstanceRoute(to.path) && !restoringLibraryInstanceRoute.value) {
			const targetFullPath = to.fullPath || to.path
			const openStartedForTarget = libraryInstanceOpenStartedForPath.value === targetFullPath
			const preserveOpenMotion = libraryInstancePullBreadcrumbMode.value === 'open'
			const preserveOpening = libraryInstanceOpening.value
			const preserveMinimizedCard =
				preserveOpenMotion &&
				libraryInstanceOpeningFromMinimized.value &&
				!!minimizedInstanceRoutePath.value
			libraryInstanceRouteReady.value = false
			libraryInstanceDragDisabled.value = false
			if (preserveOpenMotion) {
				libraryInstanceProgrammaticClose.value = false
				libraryInstanceCloseTarget.value = ''
				if (!preserveMinimizedCard) {
					clearMinimizedInstanceSnapshot()
				}
				if (!openStartedForTarget) {
					libraryInstanceOpenStartedForPath.value = ''
				}
			} else {
				clearMinimizedInstancePullState()
			}
			if (preserveOpenMotion) {
				libraryInstanceOpening.value = preserveOpening
				libraryInstanceRouteReady.value = false
				if (!openStartedForTarget) {
					libraryInstanceOpenStartedForPath.value = ''
				}
				libraryInstancePullBreadcrumbProgress.value = libraryInstanceOpenAnimationComplete.value
					? 0
					: 1
				libraryInstancePullBreadcrumbMode.value = 'open'
				void refreshLibraryInstanceRouteReady()
			} else {
				void refreshLibraryInstanceRouteReady()
			}
		}
		if (!isLibraryRoute(to.path) && !isInstanceRoute(to.path)) {
			disconnectLibraryInstanceRouteContentObserver()
			clearMinimizedInstancePullState()
			libraryInstanceRouteReady.value = false
			libraryInstanceDragDisabled.value = false
		}
		suppressNextLibraryInstanceRouteMotion.value = false
	}
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
	suspenseToken = shouldSuppressCurrentRouteLoading() ? null : loading.begin()
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
	suppressLocalRouteLoadingBar.value = false
	if (isInstanceRoute(route.path)) {
		void refreshLibraryInstanceRouteReady()
	}
	if (restoringLibraryInstanceRoute.value && isInstanceRoute(route.path)) {
		queueLibraryInstanceRestoreOverlayClear()
	}
}

async function dismissInstanceToLibrary() {
	if (!isInstanceRoute(route.path)) return

	captureInstancePullSnapshot()
	minimizedInstanceRoutePath.value = route.fullPath
	libraryInstanceProgrammaticClose.value = false
	suppressNextLibraryInstanceRouteMotion.value = true
	try {
		await router.push(lastLibraryRoutePath.value || '/library')
	} catch {
		suppressNextLibraryInstanceRouteMotion.value = false
	}
}

async function restoreInstanceFromLibraryNotch() {
	const target = minimizedInstanceRoutePath.value
	if (!target) return

	restoringLibraryInstanceRoute.value = true
	suppressNextLibraryInstanceRouteMotion.value = true
	try {
		await router.push(target)
		await nextTick()
		if (!suspensePending) {
			queueLibraryInstanceRestoreOverlayClear()
		}
	} catch {
		minimizedInstanceRoutePath.value = target
		restoringLibraryInstanceRoute.value = false
		suppressNextLibraryInstanceRouteMotion.value = false
	}
}

async function finishProgrammaticLibraryInstanceClose() {
	const target = libraryInstanceCloseTarget.value || lastLibraryRoutePath.value || '/library'
	const targetPath = router.resolve(target).path
	const shouldKeepMinimizedCard = isInstanceRoute(route.path) && isLibraryRoute(targetPath)
	if (shouldKeepMinimizedCard) {
		captureInstancePullSnapshot()
		minimizedInstanceRoutePath.value = route.fullPath
	}
	suppressNextLibraryInstanceRouteMotion.value = true
	try {
		await router.push(target)
	} catch {
		if (shouldKeepMinimizedCard) {
			minimizedInstanceRoutePath.value = ''
			minimizedInstanceSnapshotHtml.value = ''
			minimizedInstanceBreadcrumbs.value = null
		}
		suppressNextLibraryInstanceRouteMotion.value = false
		libraryInstanceProgrammaticClose.value = false
		libraryInstanceCloseTarget.value = ''
		libraryInstancePullBreadcrumbProgress.value = 0
		libraryInstancePullBreadcrumbMode.value = 'none'
	}
}

function clearMinimizedInstanceFromLibraryPull() {
	clearMinimizedInstancePullState()
}

function handleLibraryInstancePullDragging(dragging) {
	if (!dragging && !isLibraryRoute(route.path) && !minimizedInstanceRoutePath.value) {
		libraryInstancePullBreadcrumbProgress.value = 0
		libraryInstancePullBreadcrumbMode.value = 'none'
	}
}

function handleLibraryInstancePullProgress(progress, mode) {
	if (mode === 'dismiss' || mode === 'restore') {
		libraryInstancePullBreadcrumbProgress.value = Math.min(1, Math.max(0, progress))
		libraryInstancePullBreadcrumbMode.value = mode
	} else if (mode === 'open' || mode === 'close') {
		libraryInstancePullBreadcrumbProgress.value = Math.min(1, Math.max(0, progress))
		libraryInstancePullBreadcrumbMode.value = mode
	} else if (mode === 'none') {
		if (libraryInstanceOpening.value) {
			libraryInstanceOpenAnimationComplete.value = true
			libraryInstancePullBreadcrumbProgress.value = 0
			if (!finishLibraryInstanceOpenAnimationIfReady()) {
				libraryInstancePullBreadcrumbMode.value = 'open'
				libraryInstanceProgrammaticClose.value = false
			}

			if (leftNav.value) {
				updateLeftNavSlider()
			}
			return
		}

		libraryInstancePullBreadcrumbProgress.value = 0
		libraryInstancePullBreadcrumbMode.value = 'none'
		libraryInstanceProgrammaticClose.value = false
		libraryInstanceOpening.value = false
		libraryInstanceOpeningFromMinimized.value = false
	}

	if (leftNav.value) {
		updateLeftNavSlider()
	}
}

const queryClient = useQueryClient()

function preloadDiscoverContent() {
	preloadDiscoverContentQueries(queryClient)
}

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

		preloadDiscoverContent()
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
const minecraftRequiredModal = ref()
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
const sharedInstanceInviteHandler = ref()
const updateToPlayModal = ref()

watch(incompatibilityWarningModal, (modal) => {
	if (modal) {
		setContentIncompatibilityWarningModal(modal)
	}
})

const AMBERITE_DESTINATION_KEY = 'amberite:pending-destination'
let amberiteDestinationResumePromise = null

setupAuthProvider(amberiteAuth.user, async (redirectPath) => {
	await signIn('continue', redirectPath)
}, amberiteAuth.isReady)

async function signIn(mode = 'continue', redirectPath = route.fullPath) {
	try {
		window.sessionStorage.setItem(
			AMBERITE_DESTINATION_KEY,
			normalizeAmberiteDestination(redirectPath),
		)
	} catch {
		// sessionStorage can be unavailable in tests.
	}
	await amberiteAuth.signIn(mode)
	if (amberiteAuth.isLoggedIn.value) await resumeAmberiteDestination()
}

async function retryAmberiteRestore() {
	await amberiteAuth.retryRestore()
	if (amberiteAuth.isLoggedIn.value) await resumeAmberiteDestination()
}

async function resumeAmberiteDestination() {
	if (amberiteDestinationResumePromise) return await amberiteDestinationResumePromise
	amberiteDestinationResumePromise = performAmberiteDestinationResume().finally(() => {
		amberiteDestinationResumePromise = null
	})
	await amberiteDestinationResumePromise
}

async function performAmberiteDestinationResume() {
	let destination = null
	try {
		const pendingDestination = window.sessionStorage.getItem(AMBERITE_DESTINATION_KEY)
		window.sessionStorage.removeItem(AMBERITE_DESTINATION_KEY)
		if (pendingDestination) destination = normalizeAmberiteDestination(pendingDestination)
	} catch {
		// Keep the current Desktop route when transient storage is unavailable.
	}
	if (destination && destination !== route.fullPath) await router.replace(destination)
}

function normalizeAmberiteDestination(destination) {
	return typeof destination === 'string' &&
		destination.startsWith('/') &&
		!destination.startsWith('//')
		? destination
		: '/'
}

function isCloudOnlyRoute() {
	return route.matched.some((record) => record.meta.requiresCloud)
}

watch(
	[() => amberiteAuth.status.value, () => route.path],
	([nextStatus], [previousStatus]) => {
		if (previousStatus === 'offlineRetrying' && nextStatus === 'authenticated') {
			addNotification({
				title: 'Amberite connected',
				text: 'Cloud features are available again.',
				type: 'success',
			})
		}
		if (nextStatus === 'authenticated') void resumeAmberiteDestination()
		else if (isCloudOnlyRoute()) {
			void router.replace('/library')
		}
	},
)

function handleMinecraftAccountChange(hasAccounts) {
	// Launcher account selection is independent from the signed-in Amberite identity.
	hasMinecraftAccounts.value = hasAccounts
	amberiteAuth.hasMinecraftAccess.value = hasAccounts
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

async function requestModrinthAuth(flow = 'sign-in') {
	await signIn(flow)
	return !!credentials.value?.session
}

async function logOut() {
	clearLiveNotifications()
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

watch(
	[showAd, adConsentAvailable],
	async ([showAds, canManageConsent]) => {
		if (showAds) {
			await init_ads_window(true)
			return
		}

		await hide_ads_window(true)
		if (canManageConsent) {
			await init_ads_window()
		}
	},
	{ immediate: true },
)

onMounted(() => {
	invoke('show_window')

	error.setErrorModal(errorModal.value)
	error.setMinecraftAuthErrorModal(minecraftAuthErrorModal.value)
	error.setMinecraftRequiredModal(minecraftRequiredModal.value)

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
	if (!liveNotificationsEnabled || !notification?.body || notification.read) return
	if (await sharedInstanceInviteHandler.value?.handleNotification(notification)) return

	if (notification.body.type === 'server_invite') {
		if (displayedServerInviteNotifications.has(notification.id)) return

		const generation = liveNotificationGeneration
		displayedServerInviteNotifications.add(notification.id)

		const serverName =
			typeof notification.body.server_name === 'string' ? notification.body.server_name : 'a server'
		const inviterId = notification.body.invited_by
		const invitedBy =
			typeof inviterId === 'string' ? await get_user(inviterId, 'bypass').catch(() => null) : null
		if (generation !== liveNotificationGeneration) return

		const popupNotification = addPopupNotification({
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
		serverInvitePopupNotificationIds.add(popupNotification.id)
	}
}

function clearLiveNotifications() {
	liveNotificationGeneration++
	liveNotificationsEnabled = false
	for (const id of serverInvitePopupNotificationIds) {
		popupNotificationManager.removeNotification(id)
	}
	displayedServerInviteNotifications.clear()
	serverInvitePopupNotificationIds.clear()
	sharedInstanceInviteHandler.value?.clearNotifications()
}

async function handleCommand(e) {
	if (!e) return

	if (e.event === 'RunMRPack') {
		// RunMRPack should directly install a local mrpack given a path
		if (e.path.endsWith('.mrpack')) {
			const location = { type: 'fromFile', path: e.path }
			const preview = await install_get_modpack_preview(location).catch(handleError)
			if (preview?.unknownFile || preview?.externalFilesInModpack.length > 0) {
				const splitPath = e.path.split(/[\\/]/)
				const fileName = splitPath ? splitPath[splitPath.length - 1] : e.path
				unknownPackWarningModal.value?.show(
					() => install_create_modpack_instance(location).then(() => undefined),
					fileName,
					preview.externalFilesInModpack,
				)
			} else {
				await install_create_modpack_instance(location).catch(handleError)
			}
			trackEvent('InstanceCreate', {
				source: 'CreationModalFileDrop',
			})
		}
	} else if (e.event === 'LaunchInstance') {
		const instance = await getInstance(e.id).catch(handleError)
		if (!instance || instance.quarantined) return

		if (e.server) {
			await start_join_server(e.id, e.server).catch(handleError)
		} else if (e.singleplayer_world) {
			await start_join_singleplayer_world(e.id, e.singleplayer_world).catch(handleError)
		} else {
			await run(e.id).catch(handleError)
		}
	} else if (e.event === 'InstallSharedInstanceInvite') {
		await sharedInstanceInviteHandler.value?.installFromInviteId(e.invite_id)
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
		console.debug('Skipping update check as updates are disabled in this build or environment')
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

async function fetchCriticalAnnouncement(version, dev) {
	if (dev || version.endsWith('-local')) return

	const announcementUrl = `${config.labrinthBaseUrl}/appCriticalAnnouncement.json?version=${encodeURIComponent(version)}`
	try {
		const response = await fetch(announcementUrl)
		if (response.status === 404) return
		if (!response.ok) throw new Error(`Failed to fetch critical announcement: ${response.status}`)

		const res = await response.json()
		if (res && res.header && res.body) {
			criticalErrorMessage.value = res
		}
	} catch (error) {
		console.debug('Failed to fetch critical announcement', error)
	}
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

	const instances = (await list().catch(handleError)) ?? []
	const isActivePlayer = instances.some(
		(instance) =>
			isWithinLastTwoWeeks(instance.last_played) && !isWithinLastTwoWeeks(instance.created),
	)

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
		v-if="stateInitialized && !amberiteAuth.canUseLauncher.value"
		data-tauri-drag-region
		class="fixed inset-0 z-[190] grid place-items-center bg-bg px-6 text-center"
	>
		<div class="fixed right-0 top-0 z-[191] pt-1">
			<WindowControls />
		</div>
		<div class="universal-card flex w-full max-w-md flex-col items-center gap-4 !p-6">
			<SpinnerIcon
				v-if="['restoring', 'connecting'].includes(amberiteAuth.status.value)"
				class="size-8 animate-spin text-brand"
			/>
			<template v-if="amberiteAuth.status.value === 'restoring'">
				<h1 class="m-0 text-2xl font-semibold text-contrast">Restoring your session</h1>
				<p class="m-0 text-secondary">Checking the secure session stored on this device.</p>
			</template>
			<template v-else-if="amberiteAuth.status.value === 'connecting'">
				<h1 class="m-0 text-2xl font-semibold text-contrast">Verifying Minecraft</h1>
				<p class="m-0 text-secondary">Complete the Microsoft window to continue.</p>
			</template>
			<template v-else-if="amberiteAuth.status.value === 'reauthRequired'">
				<h1 class="m-0 text-2xl font-semibold text-contrast">Your Amberite session expired</h1>
				<p class="m-0 text-secondary">
					Reconnect to Amberite to continue using this account.
				</p>
				<ButtonStyled color="brand" class="w-full">
					<button class="!w-full !justify-center" @click="signIn('continue')">
						Continue with Minecraft
					</button>
				</ButtonStyled>
			</template>
			<template v-else-if="amberiteAuth.status.value === 'offlineRetrying'">
				<h1 class="m-0 text-2xl font-semibold text-contrast">Amberite is unreachable</h1>
				<p class="m-0 text-secondary">
					Your secure local session is preserved. Check your connection and retry.
				</p>
				<ButtonStyled color="brand"
					><button @click="retryAmberiteRestore">Try again</button></ButtonStyled
				>
			</template>
			<template v-else>
				<Avatar
					v-if="amberiteAuth.rememberedIdentity.value"
					:src="amberiteAuth.rememberedIdentity.value.avatarUrl"
					:alt="amberiteAuth.rememberedIdentity.value.verifiedMinecraftHandle"
					size="48px"
					circle
				/>
				<h1 class="m-0 text-2xl font-semibold text-contrast">
					{{
						amberiteAuth.rememberedIdentity.value
							? 'Signed out — Continue as ' +
								amberiteAuth.rememberedIdentity.value.verifiedMinecraftHandle
							: 'Login to Amberite'
					}}
				</h1>
				<div
					v-if="amberiteAuth.error.value"
					class="w-full rounded-lg border border-solid border-orange bg-orange-highlight p-3 text-left text-sm text-primary"
				>
					<strong class="block text-contrast">
						{{
							amberiteAuth.serverUnavailable.value
								? 'Amberite servers are unavailable'
								: 'Sign-in failed'
						}}
					</strong>
					{{ amberiteAuth.error.value.message }}
					<span v-if="amberiteAuth.serverUnavailable.value">
						Sign-in requires a connection to Amberite.
					</span>
				</div>
				<ButtonStyled color="brand" class="w-full">
					<button
						class="relative !min-h-12 !w-full !justify-center"
						:disabled="amberiteAuth.signingIn.value"
						@click="signIn('continue')"
					>
						<SpinnerIcon
							v-if="amberiteAuth.signingIn.value"
							class="absolute size-5 animate-spin"
						/>
						<span :class="{ 'opacity-0': amberiteAuth.signingIn.value }">
							Continue with Minecraft
						</span>
					</button>
				</ButtonStyled>
			</template>
		</div>
	</div>
	<div
		v-if="stateInitialized && amberiteAuth.canUseLauncher.value"
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
			<ModrinthAccountRequiredModal ref="modrinthLoginModal" :request-auth="requestModrinthAuth" />
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
			ref="leftNav"
			class="app-grid-navbar bg-bg-raised flex flex-col p-[0.5rem] pt-0 gap-[0.5rem] w-[--left-bar-width]"
			:class="{ 'left-nav-slider-ready': leftNavSliderReady }"
			@pointerdown="handleLeftNavPointerIntent"
		>
			<div
				v-if="leftNavSliderReady"
				class="left-nav-slider"
				:class="{
					'left-nav-slider--subpage': leftNavSliderSubpageSelected,
					'left-nav-slider-transition': leftNavSliderTransitionsEnabled,
				}"
				:style="leftNavSliderStyle"
				aria-hidden="true"
			/>
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
				@mouseenter="preloadDiscoverContent"
				@focus="preloadDiscoverContent"
			>
				<CompassIcon />
			</NavButton>
			<NavButton v-tooltip.right="'Skin selector'" to="/skins">
				<ChangeSkinIcon />
			</NavButton>
			<NavButton
				v-tooltip.right="'Library'"
				to="/library"
				data-left-nav-library
				:is-primary="(r) => isLibraryRoute(r.path)"
				:is-subpage="
					() =>
						route.path.startsWith('/instance') ||
						((route.path.startsWith('/browse') || route.path.startsWith('/project')) &&
							route.query.i)
				"
			>
				<LibraryIcon />
			</NavButton>
			<NavButton
				v-tooltip.right="
					canUseAmberiteFeatures ? 'Core' : 'Core is unavailable while Amberite is offline'
				"
				to="/core"
				:disabled="!canUseAmberiteFeatures"
			>
				<ServerStackIcon />
			</NavButton>
			<div class="h-px w-6 mx-auto my-2 bg-surface-5"></div>
			<suspense>
				<QuickInstanceSwitcher />
			</suspense>
			<NavButton
				v-tooltip.right="'Create new instance'"
				:to="() => installationModal?.show()"
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
						data-tauri-drag-region-exclude
						class="cursor-pointer p-0 m-0 text-contrast border-none outline-none bg-button-bg rounded-full flex items-center justify-center w-6 h-6 hover:brightness-75 transition-all"
						@click="router.back()"
					>
						<LeftArrowIcon />
					</button>
					<button
						data-tauri-drag-region-exclude
						class="cursor-pointer p-0 m-0 text-contrast border-none outline-none bg-button-bg rounded-full flex items-center justify-center w-6 h-6 hover:brightness-75 transition-all"
						@click="router.forward()"
					>
						<RightArrowIcon />
					</button>
				</div>
				<Breadcrumbs
					class="pt-[2px]"
					:preview-breadcrumbs="libraryInstancePullPreviewBreadcrumbs"
					:preview-progress="libraryInstancePullBreadcrumbPreviewProgress"
				/>
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
		v-if="stateInitialized && amberiteAuth.canUseLauncher.value"
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
				v-if="
					amberiteAuth.isOffline.value && amberiteAuth.status.value !== 'connectionError'
				"
				type="warning"
				header="Amberite is offline"
				class="m-6 mb-0"
			>
				Minecraft features still work. Core, friends, and sync are unavailable.
			</Admonition>
			<Admonition
				v-if="amberiteAuth.status.value === 'connectionError'"
				type="critical"
				header="Amberite could not connect this identity"
				class="m-6 mb-0"
			>
				Local Minecraft features remain available. Retry sign-in or open Help before using cloud features.
			</Admonition>
			<Admonition
				v-if="authUnreachable"
				type="warning"
				:header="formatMessage(messages.authUnreachableHeader)"
				class="m-6 mb-0"
			>
				{{ formatMessage(messages.authUnreachableBody) }}
			</Admonition>
			<LibraryInstancePullSurface
				:active="isLibraryInstancePullActive"
				:minimized="showLibraryInstanceRestoreNotch"
				:opening="libraryInstanceOpening && isInstanceRoute(route.path)"
				:closing="libraryInstanceProgrammaticClose"
				:drag-disabled="libraryInstanceDragDisabled"
				:open-key="libraryInstanceOpenAnimationKey"
				:close-key="libraryInstanceCloseAnimationKey"
				:restore-snapshot-html="minimizedInstanceSnapshotHtml"
				:sidebar-visible="sidebarVisible"
				@dismiss="dismissInstanceToLibrary"
				@restore="restoreInstanceFromLibraryNotch"
				@clear="clearMinimizedInstanceFromLibraryPull"
				@close-complete="finishProgrammaticLibraryInstanceClose"
				@dragging="handleLibraryInstancePullDragging"
				@progress="handleLibraryInstancePullProgress"
			>
				<template #underlay>
					<LibraryPage inert-underlay :underlay-path="lastLibraryRoutePath" />
				</template>
				<template #content>
					<div data-library-instance-route-content class="library-instance-route-content">
						<div class="library-instance-router-content">
							<RouterView v-slot="{ Component }">
								<UiMotionTransition
									v-if="routeMotion.enabled"
									:content-key="routeMotion.key"
									enabled
									:direction="routeMotion.direction"
									:type="routeMotion.type"
									:enter-ms="120"
									:leave-ms="120"
									easing="content-slide"
									mode="out-in"
									distance="2rem"
									:lock-height="false"
								>
									<template v-if="Component">
										<Suspense @pending="onSuspensePending" @resolve="onSuspenseResolve">
											<component :is="Component"></component>
										</Suspense>
									</template>
								</UiMotionTransition>
								<template v-else-if="Component">
									<Suspense @pending="onSuspensePending" @resolve="onSuspenseResolve">
										<component :is="Component"></component>
									</Suspense>
								</template>
							</RouterView>
						</div>
					</div>
				</template>
			</LibraryInstancePullSurface>
		</div>
		<div
			class="app-sidebar mt-px shrink-0 flex flex-col border-0 border-l-[1px] border-[--color-sidebar-accent-border] border-solid"
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
							v-if="!amberiteAuth.user.value && hasMinecraftAccounts"
							class="mt-2 text-sm text-secondary leading-tight"
						>
							<template v-if="amberiteAuth.signingIn">
								Connecting your Amberite account...
							</template>
							<template v-else-if="amberiteAuth.isOffline.value">
								Amberite is offline.
								<button
									class="p-0 border-0 bg-transparent text-brand cursor-pointer"
									@click="retryAmberiteRestore"
								>
									Retry
								</button>
							</template>
							<template v-else>Amberite account is not connected yet.</template>
						</div>
					</div>
					<div class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid">
						<suspense>
							<FriendsList v-if="canUseAmberiteFeatures" />
							<p v-else class="m-0 text-sm text-secondary">Friends are unavailable offline.</p>
						</suspense>
					</div>
					<PrideFundraiserBanner
						v-if="prideFundraiserEnabled"
						class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid"
					/>
				</div>
			</div>
			<div id="sidebar-bottom-teleport-target" class="sidebar-bottom-teleport-content"></div>
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
	<MinecraftRequiredModal ref="minecraftRequiredModal" />
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
			<Transition name="fade">
				<div
					v-if="amberiteAuth.error.value"
					class="rounded-lg border border-solid border-surface-5 bg-surface-2 p-3 text-sm text-secondary"
				>
					{{ amberiteAuth.error.value.message }}
				</div>
			</Transition>
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
	<SharedInstanceInviteHandler ref="sharedInstanceInviteHandler" />
	<InstallToPlayModal ref="installToPlayModal" :show-external-warnings="false" />
	<UpdateToPlayModal ref="updateToPlayModal" :show-external-warnings="false" />
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
	--color-button-bg-hover: var(--color-left-nav-button-bg-hover);

	&.left-nav-slider-ready {
		:deep(.app-nav-button.router-link-active),
		:deep(.app-nav-button.left-nav-optimistic-active) {
			--nav-button-active-bg: transparent;
			--nav-button-active-shadow: none;
		}

		:deep(.app-nav-button.subpage-active) {
			--nav-button-subpage-bg: transparent;
		}
	}

	:deep(.app-nav-button.left-nav-optimistic-active) {
		@apply text-[--color-button-text-selected];
		background: var(--nav-button-active-bg, var(--color-button-bg-selected));
		box-shadow: var(
			--nav-button-active-shadow,
			0 0 0 1px color-mix(in srgb, var(--color-brand) 38%, transparent),
			0 0 18px color-mix(in srgb, var(--color-brand) 24%, transparent)
		);

		svg {
			filter: drop-shadow(0 0 0.5rem black);
		}
	}

	:deep(.app-nav-button.left-nav-optimistic-active:hover) {
		@apply text-[--color-button-text-selected];
		background: var(--nav-button-active-bg, var(--color-button-bg-selected));
	}
}

.left-nav-slider {
	position: absolute;
	z-index: 0;
	pointer-events: none;
	overflow: hidden;
	border-radius: 100vw;
	background-color: var(--color-button-bg-selected);
	box-shadow:
		0 0 0 1px color-mix(in srgb, var(--color-brand) 38%, transparent),
		0 0 18px color-mix(in srgb, var(--color-brand) 24%, transparent);
}

.left-nav-slider--subpage {
	background-color: var(--color-button-bg);
	box-shadow: none;
}

.left-nav-slider-transition {
	transition:
		left var(--left-nav-slider-ms) var(--left-nav-slider-easing) var(--left-nav-left-delay),
		right var(--left-nav-slider-ms) var(--left-nav-slider-easing) var(--left-nav-right-delay),
		top var(--left-nav-slider-ms) var(--left-nav-slider-easing) var(--left-nav-top-delay),
		bottom var(--left-nav-slider-ms) var(--left-nav-slider-easing) var(--left-nav-bottom-delay),
		opacity var(--left-nav-fade-ms) var(--left-nav-fade-easing) var(--left-nav-fade-delay-ms),
		background-color var(--left-nav-fade-ms) var(--left-nav-fade-easing),
		box-shadow var(--left-nav-fade-ms) var(--left-nav-fade-easing);
}

.app-grid-statusbar {
	grid-area: status;
	padding-right: var(--window-controls-width, 0px);
	position: relative;
	z-index: 2;
}

:global([data-tauri-drag-region-exclude]) {
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
	--color-button-bg-hover: var(--color-sidebar-button-bg-hover);
	--color-clear-button-bg-hover: var(--color-sidebar-button-bg-hover);
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
	box-shadow: -15px 0 15px -15px rgba(0, 0, 0, 0.16) inset;
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
	scrollbar-gutter: stable both-edges;
}

.library-instance-route-content {
	position: relative;
	min-height: 100%;
	height: 100%;
}

.library-instance-router-content {
	min-height: 100%;
	height: 100%;
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

.sidebar-bottom-teleport-content {
	flex: none;
	padding: 1rem;
}

.sidebar-bottom-teleport-content:empty {
	display: none;
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
