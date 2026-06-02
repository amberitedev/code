<script setup lang="ts">
import type { AmberiteUser } from '@amberite/amberite-api'
import {
	ArrowLeftIcon,
	CheckIcon,
	ChevronRightIcon,
	LinkIcon,
	ServerStackIcon,
} from '@modrinth/assets'
import { MultiStageModal, type StageConfigInput } from '@modrinth/ui'
import { computed, ref, useTemplateRef } from 'vue'

import { useCoreCall } from '@/composables/useCoreCall'
import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'

import type { InvitedUser } from './core-onboarding-context'
import { provideOnboarding } from './core-onboarding-context'
import AdvancedStage from './stages/AdvancedStage.vue'
import CodeStage from './stages/CodeStage.vue'
import DoneStage from './stages/DoneStage.vue'
import GeneralStage from './stages/GeneralStage.vue'
import MembersStage from './stages/MembersStage.vue'

const props = defineProps<{ mode: 'setup' | 'connect' }>()
const emit = defineEmits<{ (e: 'finished'): void }>()

const modal = useTemplateRef<InstanceType<typeof MultiStageModal>>('modal')
const { currentUser, createGroup, updateGroup, inviteToGroup, acceptInvite, refresh } = useSocial()

const convexUrl = (import.meta.env.VITE_CONVEX_URL as string | undefined) ?? ''
const authJwksUrl =
	(import.meta.env.VITE_AUTH_JWKS_URL as string | undefined) ??
	(convexUrl ? `${convexUrl.replace('.convex.cloud', '.convex.site')}/.well-known/jwks.json` : '')

const { data: status, execute: loadStatus } = useCoreCall((c) => c.getSetupStatus())

const code = ref('')
const step1Loading = ref(false)
const step1Error = ref<string | null>(null)
const alreadyPaired = ref(false)
const coreId = ref<string | null>(null)

const groupName = ref('')
const description = ref('')
const bannerUrl = ref('')
const subdomain = ref('')
const runOnStartup = ref(true)
const runInBackground = ref(true)

const inviteQuery = ref('')
const inviteSearchResults = ref<AmberiteUser[]>([])
const inviteSearchLoading = ref(false)
const invitedUsers = ref<InvitedUser[]>([])
const generatedInviteCode = ref<string | null>(null)

const runMode = ref<'manual' | 'app_open' | 'startup'>('startup')

function computeRunMode(): 'manual' | 'app_open' | 'startup' {
	if (runOnStartup.value && runInBackground.value) return 'startup'
	if (runOnStartup.value) return 'app_open'
	return 'manual'
}

async function pair() {
	step1Error.value = null
	if (!currentUser.value) {
		step1Error.value = 'Sign in before pairing.'
		return
	}
	if (!code.value.trim()) {
		step1Error.value = 'Enter the pairing code.'
		return
	}
	step1Loading.value = true
	try {
		const client = useCoreClient()
		const result = await client.completeSetup({
			code: code.value.trim(),
			convex_url: convexUrl,
			auth_jwks_url: authJwksUrl,
			owner_user_id: currentUser.value.userId,
		})
		await createGroup({
			coreId: result.core_id,
			name: groupName.value.trim() || undefined,
			setupMode: 'remote',
		})
		coreId.value = result.core_id
		await refresh()
		modal.value?.nextStage()
	} catch (e) {
		step1Error.value = e instanceof Error ? e.message : String(e)
	} finally {
		step1Loading.value = false
	}
}

async function createOnly() {
	if (!status.value?.core_id) return
	try {
		await createGroup({
			coreId: status.value.core_id,
			name: groupName.value.trim() || undefined,
			setupMode: 'remote',
		})
		coreId.value = status.value.core_id
		await refresh()
		modal.value?.nextStage()
	} catch (e) {
		step1Error.value = e instanceof Error ? e.message : String(e)
	}
}

async function join() {
	step1Error.value = null
	if (!code.value.trim()) {
		step1Error.value = 'Enter an invite code.'
		return
	}
	step1Loading.value = true
	try {
		let inviteCode = code.value.trim().toUpperCase()
		if (!inviteCode.startsWith('AMB-') && /^\d{6}$/.test(inviteCode))
			inviteCode = `AMB-${inviteCode}`
		await acceptInvite({ code: inviteCode })
		await refresh()
		modal.value?.nextStage()
	} catch (e) {
		step1Error.value = e instanceof Error ? e.message : String(e)
	} finally {
		step1Loading.value = false
	}
}

async function saveGeneral() {
	try {
		await updateGroup({
			name: groupName.value.trim() || undefined,
			description: description.value.trim() || undefined,
			banner: bannerUrl.value.trim() || undefined,
			subdomain: subdomain.value.trim() || undefined,
		})
	} catch {
		/* ignored */
	}
	modal.value?.nextStage()
}

async function searchUsers() {
	const q = inviteQuery.value.trim()
	if (!q) return
	inviteSearchLoading.value = true
	try {
		const client = useSocialClient()
		inviteSearchResults.value = await client.searchUsers(q)
	} catch {
		inviteSearchResults.value = []
	} finally {
		inviteSearchLoading.value = false
	}
}

async function inviteUser(userId: string, role: string) {
	try {
		await inviteToGroup({ inviteeUserId: userId, role: role as 'owner' | 'admin' | 'member' })
		const u =
			inviteSearchResults.value.find((x) => x.userId === userId) ??
			(friends.value?.friends ?? []).find((f) => f.user?.userId === userId)?.user
		invitedUsers.value.push({
			userId,
			username: u?.displayName ?? u?.username ?? userId,
			image: u?.image,
		})
	} catch (e) {
		console.error('invite failed', e)
	}
}

async function generateInviteLink(role: string) {
	try {
		const r = await inviteToGroup({ role: role as 'owner' | 'admin' | 'member' })
		generatedInviteCode.value = r.code ?? r.inviteId
	} catch (e) {
		console.error('generate invite failed', e)
	}
}

async function saveAdvanced() {
	if (props.mode !== 'setup' || !coreId.value) {
		modal.value?.nextStage()
		return
	}
	try {
		const client = useCoreClient()
		await client.updateCoreMetadata({
			name: groupName.value.trim() || undefined,
			run_mode: computeRunMode(),
		})
	} catch (e) {
		console.error('save advanced failed', e)
	}
	modal.value?.nextStage()
}

function finish() {
	modal.value?.hide()
	emit('finished')
}

provideOnboarding({
	mode: props.mode,
	code,
	step1Loading,
	step1Error,
	alreadyPaired,
	coreId,
	groupName,
	description,
	bannerUrl,
	subdomain,
	runOnStartup,
	runInBackground,
	inviteQuery,
	inviteSearchResults,
	inviteSearchLoading,
	invitedUsers,
	generatedInviteCode,
	runMode,
	pair,
	join,
	searchUsers,
	inviteUser,
	generateInviteLink,
	saveAdvanced,
	finish,
})

const isSetup = computed(() => props.mode === 'setup')

const stageConfigs = computed<StageConfigInput<unknown>[]>(() => [
	{
		id: 'code',
		title: isSetup.value ? 'Pair' : 'Join',
		stageContent: CodeStage,
		leftButtonConfig: null,
		rightButtonConfig: {
			label: isSetup.value
				? alreadyPaired.value
					? 'Continue'
					: step1Loading.value
						? 'Pairing…'
						: 'Pair Core'
				: step1Loading.value
					? 'Joining…'
					: 'Join group',
			icon: isSetup.value ? ServerStackIcon : LinkIcon,
			disabled: step1Loading.value || (!alreadyPaired.value && !code.value.trim()),
			loading: step1Loading.value,
			onClick: async () => {
				if (isSetup.value) {
					if (alreadyPaired.value) await createOnly()
					else await pair()
				} else {
					await join()
				}
			},
		},
		cannotNavigateForward: step1Loading.value,
	},
	{
		id: 'general',
		title: 'General',
		stageContent: GeneralStage,
		leftButtonConfig: {
			label: 'Back',
			icon: ArrowLeftIcon,
			onClick: () => modal.value?.prevStage(),
		},
		rightButtonConfig: { label: 'Continue', icon: ChevronRightIcon, onClick: saveGeneral },
		cannotNavigateForward: !groupName.value.trim(),
	},
	{
		id: 'members',
		title: 'Members',
		stageContent: MembersStage,
		leftButtonConfig: {
			label: 'Back',
			icon: ArrowLeftIcon,
			onClick: () => modal.value?.prevStage(),
		},
		rightButtonConfig: {
			label: isSetup.value ? 'Next' : 'Finish',
			icon: isSetup.value ? ChevronRightIcon : CheckIcon,
			onClick: () => {
				if (isSetup.value) modal.value?.nextStage()
				else finish()
			},
		},
	},
	...(isSetup.value
		? [
				{
					id: 'advanced',
					title: 'Advanced',
					stageContent: AdvancedStage,
					leftButtonConfig: {
						label: 'Back',
						icon: ArrowLeftIcon,
						onClick: () => modal.value?.prevStage(),
					},
					rightButtonConfig: { label: 'Continue', icon: ChevronRightIcon, onClick: saveAdvanced },
				},
			]
		: []),
	{
		id: 'done',
		title: 'Done',
		stageContent: DoneStage,
		nonProgressStage: true,
		leftButtonConfig: null,
		rightButtonConfig: null,
		disableClose: false,
	},
])

function show() {
	code.value = ''
	step1Loading.value = false
	step1Error.value = null
	alreadyPaired.value = false
	coreId.value = null
	groupName.value = ''
	description.value = ''
	bannerUrl.value = ''
	subdomain.value = ''
	runOnStartup.value = true
	runInBackground.value = true
	inviteQuery.value = ''
	inviteSearchResults.value = []
	inviteSearchLoading.value = false
	invitedUsers.value = []
	generatedInviteCode.value = null
	runMode.value = 'startup'

	if (props.mode === 'setup') {
		void loadStatus().then(() => {
			alreadyPaired.value = status.value?.paired === true
			if (alreadyPaired.value) coreId.value = status.value?.core_id ?? null
		})
	}

	modal.value?.show()
}

defineExpose({ show })
</script>

<template>
	<MultiStageModal
		ref="modal"
		:stages="stageConfigs"
		:context="{}"
		breadcrumbs
		disable-progress
		fade="standard"
	/>
</template>
