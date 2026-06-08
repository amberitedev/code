import type {
	ServerAccessInviteSuggestion,
	ServerAccessMember,
	ServerAccessRole,
	ServerAccessRoleOption,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import { useAmberiteAuth } from '@/composables/useAmberiteAuth'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'

import type { CoreOnboardingContext, CoreOnboardingFlow } from './core-onboarding-context'
import {
	matchesInviteSuggestion,
	mockInviteUsers,
	toAccessMember,
	toInviteCandidate,
	toPendingInvite,
} from './core-onboarding-members'

interface CoreOnboardingModalRef { hide: () => void; show: () => void; nextStage: () => void; setStage: (stage: string) => void }

export function useCoreOnboardingState(modal: { readonly value: CoreOnboardingModalRef | null }) {
	const auth = useAmberiteAuth()
	const social = useSocial()
	const socialClient = useSocialClient()
	const connection = useCoreConnection()
	const flow = ref<CoreOnboardingFlow>('create')
	const coreName = ref('Friend group')
	const coreDescription = ref('')
	const coreIcon = ref<string>()
	const connectCode = ref('')
	const connectValidated = ref(false)
	const inviteSearch = ref('')
	const inviteAsFriend = ref(true)
	const pendingInvites = ref<ServerAccessMember[]>([])
	const error = ref('')
	const working = ref(false)
	const canManageMembers = computed(() => flow.value === 'create' || social.canManage.value)
	const roles: ServerAccessRoleOption[] = [
		{ value: 'owner', label: 'Owner', description: 'Controls the Core and group.' },
		{ value: 'editor', label: 'Admin', description: 'Manages members and servers.' },
		{ value: 'viewer', label: 'Member', description: 'Uses shared Core access.' },
	]
	const members = computed<ServerAccessMember[]>(() => [
		...social.members.value.map(toAccessMember),
		...pendingInvites.value,
		...friendInviteCandidates.value,
	])
	const friendInviteCandidates = computed<ServerAccessMember[]>(() => {
		const existingIds = new Set([...social.members.value.map((x) => x.userId), ...pendingInvites.value.map((x) => x.user.id)])
		const friends = social.friends.value?.friends
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
	})

	async function ensureAuth() {
		if (!auth.isLoggedIn.value) await auth.signIn()
	}

	async function finish() {
		working.value = true
		error.value = ''
		try {
			await ensureAuth()
			if (flow.value === 'create') await createCoreGroup()
			modal.value?.hide()
		} catch (reason) {
			error.value = reason instanceof Error ? reason.message : 'Core setup failed.'
		} finally {
			working.value = false
		}
	}

	async function createCoreGroup() {
		const current = connection.status.value?.coreId ? connection.status.value : await connection.check()
		if (!current?.coreId) throw new Error('Core is not reachable yet.')
		await social.createGroup({
			coreId: current.coreId,
			name: coreName.value.trim() || 'Friend group',
			description: coreDescription.value.trim() || undefined,
			setupMode: 'local',
			connectionUrl: current.coreUrl,
		})
	}

	async function connectCore() {
		const code = connectCode.value.trim()
		if (!code) throw new Error('Enter a Core code.')
		if (code.startsWith('AMB-')) await social.acceptInvite({ code })
		else if (!(await socialClient.claimPairingCore(code))) throw new Error('No Core was found for that code.')
		connectValidated.value = true
		await social.refresh()
	}

	async function selectIcon() {
		const value = await open({
			multiple: false,
			filters: [{ name: 'Image', extensions: ['png', 'jpeg', 'svg', 'webp', 'gif', 'jpg'] }],
		})
		if (typeof value === 'string') coreIcon.value = convertFileSrc(value)
	}

	function selectInviteSuggestion(user: ServerAccessInviteSuggestion) {
		inviteSearch.value = user.username
	}

	function createInvite() {
		const value = inviteSearch.value.trim().toLowerCase()
		const user = inviteSuggestions.value.find((suggestion) => matchesInviteSuggestion(suggestion, value))
		if (!user || members.value.some((member) => !member.inviteCandidate && member.user.id === user.id)) return
		pendingInvites.value.push(toPendingInvite(user))
		inviteSearch.value = ''
	}

	function quickInvite(member: ServerAccessMember) {
		if (!member.inviteCandidate) return
		pendingInvites.value.push({ ...member, id: member.user.id, inviteCandidate: false, pending: true })
	}

	async function updateRole(member: ServerAccessMember, role: ServerAccessRole) {
		if (member.pending) {
			pendingInvites.value = pendingInvites.value.map((invite) =>
				invite.id === member.id ? { ...invite, role } : invite,
			)
			return
		}
		if (!member.isOwner) await social.setMemberRole(member.user.id, role === 'editor' ? 'admin' : 'member')
	}

	async function removeMember(member: ServerAccessMember) {
		if (member.pending) {
			pendingInvites.value = pendingInvites.value.filter((invite) => invite.id !== member.id)
			return
		}
		if (!member.isOwner) await social.kickMember(member.user.id)
	}

	async function validateConnectAndContinue() {
		working.value = true
		error.value = ''
		try {
			await ensureAuth()
			await connectCore()
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
		modal.value?.setStage(nextFlow === 'connect' ? 'connect' : 'general')
		modal.value?.show()
	}

	const ctx: CoreOnboardingContext = {
		flow, coreName, coreDescription, coreIcon, connectCode, inviteSearch, inviteAsFriend,
		error, working, canManage: canManageMembers, members, roles, inviteSuggestions,
		selectIcon, selectInviteSuggestion, createInvite, quickInvite, updateRole, removeMember,
	}
	return { ctx, connectValidated, finish, show, validateConnectAndContinue }
}

function formatCoreCode(value: string) {
	const raw = value.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 9)
	return raw.length > 3 ? `${raw.slice(0, 3)}-${raw.slice(3)}` : raw
}
