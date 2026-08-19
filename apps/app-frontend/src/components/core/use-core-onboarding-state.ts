import type { AmberiteUser } from '@amberite/amberite-api'
import { verifyCoreConnection } from '@modrinth/api-client'
import type {
	ServerAccessInviteSuggestion,
	ServerAccessRole,
	ServerAccessRoleOption,
} from '@modrinth/ui'
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'

import { useAmberiteAuth } from '@/composables/useAmberiteAuth'
import { useCoreClient } from '@/composables/useCoreClient'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'
import { config } from '@/config'
import { clearConnectedCore, getConnectedCore, setConnectedCore } from '@/core/connected-core'

import type { CoreAccessMember } from './core-access-types'
import type {
	CoreInviteLookupStatus,
	CoreOnboardingContext,
	CoreOnboardingFlow,
	CoreStartupRunMode,
} from './core-onboarding-context'
import {
	matchesInviteSuggestion,
	toAccessMember,
	toInviteCandidate,
	toInviteSuggestion,
	toPendingInvite,
} from './core-onboarding-members'

const LOCAL_CORE_URL = 'http://127.0.0.1:16662'

interface CoreOnboardingModalRef {
	hide: () => void
	show: () => void
	nextStage: () => void
	setStage: (stage: string) => void
}

interface ReservedPairingCore {
	code: string
	coreId: string
	coreUrl: string
	ownerUserId: string
	ownerDisplayName: string
	realtimeCredential: string
	setupSubmitted: boolean
}

export function useCoreOnboardingState(modal: { readonly value: CoreOnboardingModalRef | null }) {
	const auth = useAmberiteAuth()
	const social = useSocial()
	const socialClient = useSocialClient()
	const coreClient = useCoreClient()
	const connection = useCoreConnection()
	const flow = ref<CoreOnboardingFlow>('create')
	const connectCode = ref('')
	const connectValidated = ref(false)
	const inviteSearch = ref('')
	const inviteAsFriend = ref(true)
	const inviteLookupStatus = ref<CoreInviteLookupStatus>('idle')
	const remoteInviteSuggestions = ref<ServerAccessInviteSuggestion[]>([])
	const selectedInviteSuggestion = ref<ServerAccessInviteSuggestion | null>(null)
	const pendingInvites = ref<CoreAccessMember[]>([])
	const startupRunMode = ref<CoreStartupRunMode>('app_open')
	const error = ref('')
	const working = ref(false)
	const modalOpen = ref(false)
	const reservedPairingCore = ref<ReservedPairingCore | null>(null)
	let pairingReservationRequestId = 0
	let finalizingReservedPairing = false
	let inviteLookupRequestId = 0
	let inviteLookupTimer: ReturnType<typeof setTimeout> | undefined
	const canManageMembers = computed(
		() => flow.value === 'create' || !!reservedPairingCore.value || social.canManage.value,
	)
	const roles: ServerAccessRoleOption[] = [
		{ value: 'owner', label: 'Owner', description: 'Controls the Core and group.' },
		{ value: 'editor', label: 'Admin', description: 'Manages members and servers.' },
		{ value: 'viewer', label: 'Member', description: 'Uses shared Core access.' },
	]
	const members = computed<CoreAccessMember[]>(() => [
		...social.members.value.map(toAccessMember),
		...pendingInvites.value,
		...friendInviteCandidates.value,
	])
	const friendInviteCandidates = computed<CoreAccessMember[]>(() => {
		const existingIds = new Set([
			...social.members.value.map((x) => x.userId),
			...pendingInvites.value.map((x) => x.user.id),
		])
		const friends =
			social.friends.value?.friends
				.map((friend) => friend.user)
				.filter((user): user is NonNullable<typeof user> => !!user) ?? []
		return friends.filter((user) => !existingIds.has(user.userId)).map(toInviteCandidate)
	})
	const inviteSuggestions = computed<ServerAccessInviteSuggestion[]>(() => {
		const suggestions = new Map<string, ServerAccessInviteSuggestion>()
		if (selectedInviteSuggestion.value)
			suggestions.set(selectedInviteSuggestion.value.id, selectedInviteSuggestion.value)
		for (const suggestion of remoteInviteSuggestions.value)
			suggestions.set(suggestion.id, suggestion)
		return filterUnavailableInviteSuggestions([...suggestions.values()])
	})

	watch(connectCode, (value) => {
		const formatted = formatCoreCode(value)
		if (formatted !== value) connectCode.value = formatted
		if (reservedPairingCore.value) void releaseReservedPairingCore()
		connectValidated.value = false
		error.value = ''
	})

	watch(inviteSearch, (value) => {
		queueInviteLookup(value)
	})

	onUnmounted(() => {
		if (inviteLookupTimer) clearTimeout(inviteLookupTimer)
		modalOpen.value = false
		if (!finalizingReservedPairing) void releaseReservedPairingCore()
	})

	async function ensureAuth() {
		if (!auth.isLoggedIn.value) await auth.signIn()
	}

	async function finish(): Promise<boolean> {
		working.value = true
		error.value = ''
		try {
			await ensureAuth()
			if (flow.value === 'create') {
				if (!connectedCoreForCurrentAccount()?.coreId) await createCoreGroup()
				await applyCreateSettings()
			} else {
				finalizingReservedPairing = true
				await finalizeReservedPairingCore()
			}
			await sendPendingInvites()
			await social.refresh()
			working.value = false
			await nextTick()
			modal.value?.hide()
			return true
		} catch (reason) {
			error.value = reason instanceof Error ? reason.message : 'Core setup failed.'
			return false
		} finally {
			const shouldReleaseClosedPairing = !modalOpen.value && flow.value === 'connect'
			finalizingReservedPairing = false
			if (shouldReleaseClosedPairing) await releaseReservedPairingCore()
			working.value = false
		}
	}

	async function createCoreGroup(): Promise<string> {
		ensureNoConnectedCore('Cannot create a new Core: a Core is already connected.')
		const currentUser = await socialClient.currentUser()
		if (!currentUser?.userId) throw new Error('Sign in to Amberite to create a Core.')
		const current = await verifyCoreConnection(coreClient.adapter, { coreUrl: LOCAL_CORE_URL })
		if (current.state !== 'connected' || !current.coreId || !current.coreUrl) {
			throw new Error('Start Core before finishing setup.')
		}
		const localSetupSecret = await coreClient.adapter.getLocalSetupSecret?.()
		if (!localSetupSecret) {
			throw new Error('Local Core setup secret is not available. Restart Core and try again.')
		}
		const response = await coreClient.completeSetupAt(current.coreUrl, {
			local_setup_secret: localSetupSecret,
			convex_url: config.convexUrl,
			auth_jwks_url: convexJwksUrl(config.convexSiteUrl),
			auth_audience: 'convex',
			owner_user_id: currentUser.userId,
			owner_display_name:
				currentUser.username || currentUser.displayName || currentUser.name || currentUser.userId,
		})
		if (response.core_id !== current.coreId)
			throw new Error('The local Core answered with a different identity.')
		const group = await social.createGroup({
			coreId: current.coreId,
			name: 'Friend group',
			setupMode: 'local',
			connectionUrl: current.coreUrl,
		})
		if (social.error.value) throw social.error.value
		if (!group?.friendGroupId) throw new Error('Core group could not be created.')
		setConnectedCore({
			coreId: current.coreId,
			url: current.coreUrl,
			groupId: group.friendGroupId,
		})
		coreClient.clearCoreUrlCache()
		return group.friendGroupId
	}

	async function applyCreateSettings() {
		await coreClient.updateCoreMetadata({ run_mode: startupRunMode.value })
	}

	async function finalizeReservedPairingCore(): Promise<string> {
		ensureNoConnectedCore('Cannot connect: a Core is already connected.')
		const claim = reservedPairingCore.value
		if (!claim) throw new Error('Connect to a Core before finishing setup.')

		const previousCore = getConnectedCore()
		let linked = false
		try {
			if (!claim.setupSubmitted) {
				const response = await coreClient.completeSetupAt(claim.coreUrl, {
					code: claim.code,
					convex_url: config.convexUrl,
					auth_jwks_url: convexJwksUrl(config.convexSiteUrl),
					auth_audience: 'convex',
					owner_user_id: claim.ownerUserId,
					owner_display_name: claim.ownerDisplayName,
					realtime_credential: claim.realtimeCredential,
					...(config.realtimeUrl
						? {
								realtime_url: config.realtimeUrl,
							}
						: {}),
				})
				if (response.core_id !== claim.coreId)
					throw new Error('The Core answered with a different identity.')
				claim.setupSubmitted = true
			}

			setConnectedCore({ coreId: claim.coreId, url: claim.coreUrl })
			linked = true
			coreClient.clearCoreUrlCache()
			const connectionStatus = await connection.check()
			if (connectionStatus?.state !== 'connected') {
				throw new Error('The Core could not verify its identity after pairing.')
			}
			const finalized = await socialClient.finalizePairingCore({
				code: claim.code,
				coreId: claim.coreId,
				connectionUrl: claim.coreUrl,
			})
			reservedPairingCore.value = null
			return finalized.friendGroupId
		} catch (error) {
			if (linked) {
				if (previousCore) setConnectedCore(previousCore)
				else clearConnectedCore()
				coreClient.clearCoreUrlCache()
			}
			throw error
		}
	}

	async function sendPendingInvites() {
		for (const invite of pendingInvites.value) {
			await coreClient.createCoreInvitation({
				invitee_user_id: invite.user.id,
				invitee_display_name: invite.user.username,
				role_id: inviteRoleId(invite.role),
			})
			if (inviteAsFriend.value && !friendRequestUnavailableUserIds().has(invite.user.id)) {
				await social.sendFriendRequest({ targetUserId: invite.user.id })
				if (social.error.value) throw social.error.value
			}
		}
	}

	async function connectCore() {
		ensureNoConnectedCore('Cannot connect: a Core is already connected.')
		if (!modalOpen.value) throw new Error('Core connection was cancelled.')
		const code = normalizeCoreCode(connectCode.value)
		if (code.length !== 8) throw new Error('Enter all 8 characters of the Core code.')
		if (!/^[a-hj-np-z2-9]{8}$/.test(code))
			throw new Error('Code invalid. Please check your Core and try again.')
		const requestId = ++pairingReservationRequestId
		await reserveRemoteCore(code, requestId)
		if (requestId !== pairingReservationRequestId || !modalOpen.value)
			throw new Error('Core connection was cancelled.')
		connectValidated.value = true
	}

	async function reserveRemoteCore(code: string, requestId: number) {
		const currentUser = await socialClient.currentUser()
		if (!currentUser?.userId) throw new Error('Sign in to Amberite to connect a Core.')

		if (reservedPairingCore.value?.code === code) return
		if (reservedPairingCore.value?.code !== code)
			await releaseReservedPairingCore({ invalidatePending: false })

		const claim = await socialClient.claimPairingCore(code)
		if (!claim) throw new Error('No Core was found for that code.')
		if (
			requestId !== pairingReservationRequestId ||
			!modalOpen.value ||
			normalizeCoreCode(connectCode.value) !== code
		) {
			await socialClient.releasePairingCore({ code, coreId: claim.coreId }).catch(() => undefined)
			throw new Error('Core connection was cancelled.')
		}

		const coreUrl = resolvePairingCoreUrl(claim)
		if (!coreUrl) {
			await socialClient.releasePairingCore({ code, coreId: claim.coreId })
			throw new Error('That Core did not publish a reachable connection URL.')
		}

		reservedPairingCore.value = {
			code,
			coreId: claim.coreId,
			coreUrl,
			ownerUserId: currentUser.userId,
			ownerDisplayName:
				currentUser.username || currentUser.displayName || currentUser.name || currentUser.userId,
			realtimeCredential: claim.realtimeCredential,
			setupSubmitted: false,
		}
		if (requestId !== pairingReservationRequestId || !modalOpen.value) {
			await releaseReservedPairingCore()
			throw new Error('Core connection was cancelled.')
		}
	}

	async function releaseReservedPairingCore({
		invalidatePending = true,
	}: { invalidatePending?: boolean } = {}) {
		if (invalidatePending) pairingReservationRequestId += 1
		const claim = reservedPairingCore.value
		if (!claim) return
		reservedPairingCore.value = null
		connectValidated.value = false
		if (claim.setupSubmitted) return
		await socialClient
			.releasePairingCore({ code: claim.code, coreId: claim.coreId })
			.catch(() => undefined)
	}

	function queueInviteLookup(value: string) {
		const query = value.trim()
		const selected = selectedInviteSuggestion.value
		if (!selected || !matchesInviteSuggestion(selected, query.toLowerCase()))
			selectedInviteSuggestion.value = null
		inviteLookupRequestId += 1
		remoteInviteSuggestions.value = []
		if (inviteLookupTimer) clearTimeout(inviteLookupTimer)

		if (query.length < 2) {
			inviteLookupStatus.value = 'idle'
			return
		}

		inviteLookupStatus.value = 'loading'
		const requestId = inviteLookupRequestId
		inviteLookupTimer = setTimeout(() => {
			void searchInviteSuggestions(query, requestId)
		}, 250)
	}

	async function searchInviteSuggestions(query: string, requestId: number) {
		try {
			const users = await social.searchUsers(query)
			if (requestId !== inviteLookupRequestId || query !== inviteSearch.value.trim()) return
			remoteInviteSuggestions.value = filterUnavailableInviteUsers(users).map(toInviteSuggestion)
		} catch {
			if (requestId === inviteLookupRequestId && query === inviteSearch.value.trim())
				remoteInviteSuggestions.value = []
		} finally {
			if (requestId === inviteLookupRequestId && query === inviteSearch.value.trim())
				inviteLookupStatus.value = 'loaded'
		}
	}

	function filterUnavailableInviteUsers(users: AmberiteUser[]) {
		const unavailableIds = unavailableInviteUserIds()
		return users.filter((user) => !unavailableIds.has(user.userId))
	}

	function filterUnavailableInviteSuggestions(users: ServerAccessInviteSuggestion[]) {
		const unavailableIds = unavailableInviteUserIds()
		return users.filter((user) => !unavailableIds.has(user.id))
	}

	function unavailableInviteUserIds() {
		return new Set(
			[
				social.currentUser.value?.userId,
				...social.members.value.map((member) => member.userId),
				...pendingInvites.value.map((invite) => invite.user.id),
			].filter((id): id is string => !!id),
		)
	}

	function friendRequestUnavailableUserIds() {
		return new Set([
			...(social.friends.value?.friends ?? [])
				.map((friend) => friend.user?.userId)
				.filter((id): id is string => !!id),
			...(social.friends.value?.incoming ?? []).map((request) => request.request.fromUserId),
			...(social.friends.value?.outgoing ?? []).map((request) => request.request.toUserId),
		])
	}

	function selectInviteSuggestion(user: ServerAccessInviteSuggestion) {
		selectedInviteSuggestion.value = user
		inviteSearch.value = user.username
	}

	function createInvite() {
		const value = inviteSearch.value.trim().toLowerCase()
		const user = inviteSuggestions.value.find((suggestion) =>
			matchesInviteSuggestion(suggestion, value),
		)
		if (
			!user ||
			members.value.some((member) => !member.inviteCandidate && member.user.id === user.id)
		)
			return
		pendingInvites.value.push(toPendingInvite(user))
		selectedInviteSuggestion.value = null
		inviteSearch.value = ''
	}

	function quickInvite(member: CoreAccessMember) {
		if (!member.inviteCandidate) return
		pendingInvites.value.push({
			...member,
			id: member.user.id,
			inviteCandidate: false,
			pending: true,
		})
	}

	async function updateRole(member: CoreAccessMember, role: ServerAccessRole) {
		if (member.pending) {
			pendingInvites.value = pendingInvites.value.map((invite) =>
				invite.id === member.id ? { ...invite, role } : invite,
			)
			return
		}
		if (!member.isOwner)
			await social.setMemberRole(member.user.id, role === 'editor' ? 'admin' : 'member')
	}

	async function removeMember(member: CoreAccessMember) {
		if (member.pending) {
			pendingInvites.value = pendingInvites.value.filter((invite) => invite.id !== member.id)
			return
		}
		if (!member.isOwner) await social.kickMember(member.user.id)
	}

	async function validateConnectAndContinue() {
		const startedAt = performance.now()
		working.value = true
		error.value = ''
		try {
			ensureNoConnectedCore('Cannot connect: a Core is already connected.')
			await ensureAuth()
			await connectCore()
			await waitForMinimumDuration(startedAt)
			modal.value?.nextStage()
		} catch (reason) {
			error.value = reason instanceof Error ? reason.message : 'Core connection failed.'
		} finally {
			working.value = false
		}
	}

	function show(nextFlow: CoreOnboardingFlow) {
		void releaseReservedPairingCore()
		modalOpen.value = true
		flow.value = nextFlow
		error.value = ''
		connectCode.value = ''
		connectValidated.value = false
		inviteSearch.value = ''
		inviteAsFriend.value = true
		inviteLookupStatus.value = 'idle'
		remoteInviteSuggestions.value = []
		selectedInviteSuggestion.value = null
		pendingInvites.value = []
		startupRunMode.value = 'app_open'
		if (inviteLookupTimer) clearTimeout(inviteLookupTimer)
		inviteLookupRequestId += 1
		modal.value?.setStage(nextFlow === 'connect' ? 'connect' : 'start')
		modal.value?.show()
	}

	function handleModalHide() {
		modalOpen.value = false
		if (finalizingReservedPairing) return
		void releaseReservedPairingCore()
	}

	function connectedCoreForCurrentAccount() {
		const connected = getConnectedCore()
		if (!connected?.coreId) return null
		const link = social.coreLinks.value.find(
			(core) =>
				core.coreId === connected.coreId && core.linkState === 'linked' && !!core.connectionUrl,
		)
		if (link) return connected
		clearConnectedCore()
		coreClient.clearCoreUrlCache()
		return null
	}

	const ctx: CoreOnboardingContext = {
		flow,
		connectCode,
		connectValidated,
		inviteSearch,
		inviteAsFriend,
		inviteLookupStatus,
		startupRunMode,
		error,
		working,
		canManage: canManageMembers,
		members,
		roles,
		inviteSuggestions,
		selectedInviteSuggestion,
		selectInviteSuggestion,
		createInvite,
		quickInvite,
		updateRole,
		removeMember,
	}
	return { ctx, connectValidated, finish, handleModalHide, show, validateConnectAndContinue }
}

function ensureNoConnectedCore(message: string) {
	const connected = getConnectedCore()
	if (connected?.coreId) throw new Error(message)
}

function formatCoreCode(value: string) {
	const raw = value.toUpperCase().replace(/[^A-Z0-9]/g, '')
	const code = raw.slice(0, 8)
	return code.length > 4 ? `${code.slice(0, 4)}-${code.slice(4)}` : code
}

function normalizeCoreCode(value: string) {
	return value.replace(/[^A-Z0-9]/gi, '').toLowerCase()
}

function resolvePairingCoreUrl(claim: { connectionUrl?: string; metadata?: unknown }) {
	if (claim.connectionUrl) return claim.connectionUrl.replace(/\/$/, '')
	const metadata = claim.metadata as { bindHost?: unknown; port?: unknown } | undefined
	const port = typeof metadata?.port === 'number' ? metadata.port : undefined
	if (!port) return null
	const host = typeof metadata?.bindHost === 'string' ? metadata.bindHost : '127.0.0.1'
	if (host !== '127.0.0.1' && host !== 'localhost' && host !== '::1') return null
	return `http://127.0.0.1:${port}`
}

function convexJwksUrl(convexSiteUrl: string) {
	if (!convexSiteUrl) throw new Error('Convex site URL is not configured for Amberite auth.')
	return `${convexSiteUrl.replace(/\/$/, '')}/.well-known/jwks.json`
}

function inviteRoleId(role: ServerAccessRole) {
	return role === 'editor' || role === 'owner' ? 'role-admin' : 'role-member'
}

async function waitForMinimumDuration(startedAt: number) {
	const remaining = 1500 - (performance.now() - startedAt)
	if (remaining > 0) await new Promise((resolve) => setTimeout(resolve, remaining))
}
