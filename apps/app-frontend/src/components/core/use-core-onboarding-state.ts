import type {
	ServerAccessInviteSuggestion,
	ServerAccessRole,
	ServerAccessRoleOption,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { useAmberiteAuth } from '@/composables/useAmberiteAuth'
import { useCoreClient } from '@/composables/useCoreClient'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'
import { config } from '@/config'
import { clearConnectedCore, getConnectedCore, setConnectedCore } from '@/core/connected-core'

import type { CoreOnboardingContext, CoreOnboardingFlow } from './core-onboarding-context'
import {
	matchesInviteSuggestion,
	mockInviteUsers,
	toAccessMember,
	toInviteCandidate,
	toPendingInvite,
} from './core-onboarding-members'
import type { CoreAccessMember } from './core-access-types'

interface CoreOnboardingModalRef {
	hide: () => void
	show: () => void
	nextStage: () => void
	setStage: (stage: string) => void
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
	const pendingInvites = ref<CoreAccessMember[]>([])
	const error = ref('')
	const working = ref(false)
	const canManageMembers = computed(() => flow.value === 'create' || social.canManage.value)
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
				.filter((user): user is NonNullable<typeof user> => !!user) ?? mockInviteUsers.slice(0, 2)
		return friends.filter((user) => !existingIds.has(user.userId)).map(toInviteCandidate)
	})
	const inviteSuggestions = computed<ServerAccessInviteSuggestion[]>(() => {
		const query = inviteSearch.value.trim().toLowerCase()
		return mockInviteUsers.filter((user) => !query || user.username.toLowerCase().includes(query))
	})

	watch(connectCode, (value) => {
		const formatted = formatCoreCode(value)
		if (formatted !== value) connectCode.value = formatted
		connectValidated.value = false
		error.value = ''
	})

	async function ensureAuth() {
		if (!auth.isLoggedIn.value) await auth.signIn()
	}

	async function finish() {
		working.value = true
		error.value = ''
		try {
			if (flow.value === 'create')
				ensureNoConnectedCore('Cannot create a new Core: a Core is already connected.')
			await ensureAuth()
			if (flow.value === 'create') await createCoreGroup()
			await social.refresh()
			modal.value?.hide()
		} catch (reason) {
			error.value = reason instanceof Error ? reason.message : 'Core setup failed.'
		} finally {
			working.value = false
		}
	}

	async function createCoreGroup() {
		ensureNoConnectedCore('Cannot create a new Core: a Core is already connected.')
		const current = await connection.check()
		if (!current?.coreId || !current.coreUrl) {
			throw new Error('Start Core before finishing setup.')
		}
		setConnectedCore({ coreId: current.coreId, url: current.coreUrl })
		coreClient.clearCoreUrlCache()
		await social.createGroup({
			coreId: current.coreId,
			name: 'Friend group',
			setupMode: 'local',
			connectionUrl: current.coreUrl,
		})
	}

	async function connectCore() {
		ensureNoConnectedCore('Cannot connect: a Core is already connected.')
		const code = normalizeCoreCode(connectCode.value)
		if (code.length !== 8) throw new Error('Enter all 8 characters of the Core code.')
		if (!/^[a-hj-np-z2-9]{8}$/.test(code))
			throw new Error('Code invalid. Please check your Core and try again.')
		await pairRemoteCore(code)
		connectValidated.value = true
	}

	async function pairRemoteCore(code: string) {
		const currentUser = await socialClient.currentUser()
		if (!currentUser?.userId) throw new Error('Sign in to Amberite to connect a Core.')

		const claim = await socialClient.claimPairingCore(code)
		if (!claim) throw new Error('No Core was found for that code.')

		const coreUrl = resolvePairingCoreUrl(claim)
		if (!coreUrl) {
			await socialClient.releasePairingCore({ code, coreId: claim.coreId })
			throw new Error('That Core did not publish a reachable connection URL.')
		}

		const previousCore = getConnectedCore()
		let linked = false
		try {
			const response = await coreClient.completeSetupAt(coreUrl, {
				code,
				convex_url: config.convexUrl,
				auth_jwks_url: convexJwksUrl(config.convexUrl),
				owner_user_id: currentUser.userId,
				...(config.realtimeUrl
					? { realtime_credential: claim.realtimeCredential, realtime_url: config.realtimeUrl }
					: {}),
			})
			if (response.core_id !== claim.coreId)
				throw new Error('The Core answered with a different identity.')
			setConnectedCore({ coreId: response.core_id, url: coreUrl })
			linked = true
			coreClient.clearCoreUrlCache()
			const connectionStatus = await connection.check()
			if (connectionStatus?.state !== 'connected') {
				throw new Error('The Core could not verify its identity after pairing.')
			}
			await socialClient.finalizePairingCore({
				code,
				coreId: claim.coreId,
				connectionUrl: coreUrl,
			})
			await social.refresh()
		} catch (error) {
			if (linked) {
				if (previousCore) setConnectedCore(previousCore)
				else clearConnectedCore()
				coreClient.clearCoreUrlCache()
			}
			await socialClient.releasePairingCore({ code, coreId: claim.coreId }).catch(() => undefined)
			throw error
		}
	}

	function selectInviteSuggestion(user: ServerAccessInviteSuggestion) {
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
		flow.value = nextFlow
		error.value = ''
		connectCode.value = ''
		connectValidated.value = false
		inviteSearch.value = ''
		inviteAsFriend.value = true
		pendingInvites.value = []
		modal.value?.setStage(nextFlow === 'connect' ? 'connect' : 'members')
		modal.value?.show()
	}

	const ctx: CoreOnboardingContext = {
		flow,
		connectCode,
		connectValidated,
		inviteSearch,
		inviteAsFriend,
		error,
		working,
		canManage: canManageMembers,
		members,
		roles,
		inviteSuggestions,
		selectInviteSuggestion,
		createInvite,
		quickInvite,
		updateRole,
		removeMember,
	}
	return { ctx, connectValidated, finish, show, validateConnectAndContinue }
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

function convexJwksUrl(convexUrl: string) {
	if (!convexUrl) throw new Error('Convex is not configured for Amberite account linking.')
	return `${convexUrl.replace(/\/$/, '')}/.well-known/jwks.json`
}

async function waitForMinimumDuration(startedAt: number) {
	const remaining = 1500 - (performance.now() - startedAt)
	if (remaining > 0) await new Promise((resolve) => setTimeout(resolve, remaining))
}
