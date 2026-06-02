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

type CoreRunMode = 'manual' | 'app_open' | 'startup'
const props = defineProps<{ mode: 'setup' | 'connect' }>()
const emit = defineEmits<{ finished: [] }>()
const modal = useTemplateRef<InstanceType<typeof MultiStageModal>>('modal')
const { currentUser, createGroup, updateGroup, inviteToGroup, refresh, friends } = useSocial()

const convexUrl = (import.meta.env.VITE_CONVEX_URL as string | undefined) ?? ''
const authJwksUrl =
	(import.meta.env.VITE_AUTH_JWKS_URL as string | undefined) ??
	(convexUrl ? `${convexUrl.replace('.convex.cloud', '.convex.site')}/.well-known/jwks.json` : '')
const code = ref('')
const step1Loading = ref(false)
const step1Error = ref<string | null>(null)
const alreadyPaired = ref(false)
const coreId = ref<string | null>(null)
const groupName = ref('Amberite Core')
const description = ref('')
const iconUrl = ref('')
const bannerUrl = ref('')
const subdomain = ref('')
const runOnStartup = ref(true)
const runInBackground = ref(true)
const installPath = ref('Default app data folder')
const backupRetention = ref(7)
const shareTelemetry = ref(false)
const inviteQuery = ref('')
const inviteSearchResults = ref<AmberiteUser[]>([])
const inviteSearchLoading = ref(false)
const invitedUsers = ref<InvitedUser[]>([])
const generatedInviteCode = ref<string | null>(null)
const runMode = ref<CoreRunMode>('startup')
const progress = ref(0)
const progressLabel = ref('Waiting to start')

async function pair() {
	step1Error.value = null
	if (!currentUser.value) return (step1Error.value = 'Sign in before pairing.')
	if (!/^\d{6}$/.test(code.value.trim())) return (step1Error.value = 'Enter the six-digit code.')
	step1Loading.value = true
	try {
		const result = await useCoreClient().completeSetup({
			code: code.value.trim(),
			convex_url: convexUrl,
			auth_jwks_url: authJwksUrl,
			owner_user_id: currentUser.value.userId,
		})
		await createGroup({ coreId: result.core_id, name: groupName.value, setupMode: 'remote' })
		coreId.value = result.core_id
		await refresh()
		modal.value?.nextStage()
	} catch (e) {
		step1Error.value = e instanceof Error ? e.message : String(e)
	} finally {
		step1Loading.value = false
	}
}

async function saveGeneral() {
	if (coreId.value) {
		await updateGroup({
			name: groupName.value.trim() || undefined,
			description: description.value.trim() || undefined,
			subdomain: subdomain.value.trim() || undefined,
		})
	}
	modal.value?.nextStage()
}

async function searchUsers() {
	const q = inviteQuery.value.trim()
	if (!q) return
	inviteSearchLoading.value = true
	try {
		inviteSearchResults.value = await useSocialClient().searchUsers(q)
	} finally {
		inviteSearchLoading.value = false
	}
}

async function inviteUser(userId: string, role: string) {
	await inviteToGroup({ inviteeUserId: userId, role: role as 'admin' | 'member' })
	const user =
		inviteSearchResults.value.find((x) => x.userId === userId) ??
		(friends.value?.friends ?? []).find((x) => x.user?.userId === userId)?.user
	invitedUsers.value.push({
		userId,
		username: user?.displayName ?? user?.username ?? userId,
		image: user?.image,
	})
}

async function generateInviteLink(role: string) {
	const result = await inviteToGroup({ role: role as 'admin' | 'member' })
	generatedInviteCode.value = result.code ?? result.inviteId
}

async function saveAdvanced() {
	if (coreId.value) {
		await useCoreClient().updateCoreMetadata({
			name: groupName.value,
			description: description.value || null,
			banner: bannerUrl.value || null,
			subdomain: subdomain.value || null,
			run_mode: runMode.value,
		})
	}
	modal.value?.nextStage()
}

async function runFinish() {
	const labels =
		props.mode === 'setup'
			? ['Downloading Core', 'Verifying binary', 'Starting service', 'Linking app', 'Final checks']
			: ['Checking Core', 'Linking account', 'Syncing group', 'Final checks']
	for (let i = 0; i < labels.length; i++) {
		progressLabel.value = labels[i]
		progress.value = Math.round(((i + 1) / labels.length) * 100)
		await new Promise((resolve) => setTimeout(resolve, props.mode === 'setup' ? 6000 : 1400))
	}
	if (props.mode === 'setup' && currentUser.value && !coreId.value) {
		const id = `local-preview-${Date.now()}`
		await createGroup({ coreId: id, name: groupName.value, setupMode: 'local' })
		coreId.value = id
		await refresh()
	}
}

function finish() {
	modal.value?.hide()
	emit('finished')
}

provideOnboarding({
	mode: props.mode,
	code, step1Loading, step1Error, alreadyPaired, coreId, groupName, description, iconUrl, bannerUrl,
	subdomain, runOnStartup, runInBackground, installPath, backupRetention, shareTelemetry,
	inviteQuery, inviteSearchResults, inviteSearchLoading, invitedUsers, generatedInviteCode,
	runMode, progress, progressLabel, pair, saveGeneral, searchUsers, inviteUser,
	generateInviteLink, saveAdvanced, runFinish, finish,
})

const isSetup = computed(() => props.mode === 'setup')
const back = { label: 'Back', icon: ArrowLeftIcon, onClick: () => modal.value?.prevStage() }
const stageConfigs = computed<StageConfigInput<unknown>[]>(() => [
	{ id: 'code', title: 'Pairing code', stageContent: CodeStage, skip: isSetup.value, leftButtonConfig: null, rightButtonConfig: {
		label: step1Loading.value ? 'Verifying...' : 'Verify connection', icon: LinkIcon,
		disabled: step1Loading.value || !/^\d{6}$/.test(code.value.trim()), loading: step1Loading.value, onClick: pair }, cannotNavigateForward: !coreId.value, maxWidth: '680px' },
	{ id: 'general', title: 'General', stageContent: GeneralStage, leftButtonConfig: isSetup.value ? null : back, rightButtonConfig: { label: 'Continue', icon: ChevronRightIcon, onClick: saveGeneral }, cannotNavigateForward: !groupName.value.trim(), maxWidth: '760px' },
	{ id: 'members', title: 'Members', stageContent: MembersStage, leftButtonConfig: back, rightButtonConfig: { label: 'Continue', icon: ChevronRightIcon, onClick: () => modal.value?.nextStage() }, maxWidth: '760px' },
	{ id: 'advanced', title: 'Advanced', stageContent: AdvancedStage, leftButtonConfig: back, rightButtonConfig: { label: 'Start', icon: ServerStackIcon, onClick: saveAdvanced }, maxWidth: '720px' },
	{ id: 'done', title: isSetup.value ? 'Install Core' : 'Connected', stageContent: DoneStage, nonProgressStage: true, leftButtonConfig: null, rightButtonConfig: { label: 'Finish', icon: CheckIcon, disabled: progress.value < 100, onClick: finish }, maxWidth: '680px' },
])

function show() {
	code.value = ''
	step1Error.value = null
	step1Loading.value = false
	coreId.value = null
	groupName.value = 'Amberite Core'
	description.value = ''
	iconUrl.value = ''
	bannerUrl.value = ''
	subdomain.value = ''
	invitedUsers.value = []
	generatedInviteCode.value = null
	progress.value = 0
	progressLabel.value = 'Waiting to start'
	modal.value?.setStage(props.mode === 'setup' ? 'general' : 'code'); modal.value?.show()
}

defineExpose({ show })
</script>

<template>
	<MultiStageModal ref="modal" :stages="stageConfigs" :context="{}" breadcrumbs fade="standard" />
</template>
