<script setup lang="ts">
import { LinkIcon, RefreshCwIcon, ServerStackIcon, UnlinkIcon } from '@modrinth/assets'
import type { Amberite } from '@modrinth/api-client'
import {
	Button,
	EmptyState,
	injectModrinthClient,
	injectNotificationManager,
	StyledInput,
} from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { config } from '@/config'
import {
	clearConnectedCore,
	getConnectedCore,
	setConnectedCore,
	useConnectedCore,
} from '@/core/connected-core'
import { amberite } from '@/services/amberite'

const notifications = injectNotificationManager()
const client = injectModrinthClient()
const connectedCore = useConnectedCore()
const cores = ref<Amberite.Cores.v1.CoreListEntry[]>([])
const code = ref('')
const pairing = ref(false)
const loading = ref(false)
const normalizedCode = computed(() => code.value.replace(/[^a-z0-9]/gi, '').toLowerCase())
const canPair = computed(() => normalizedCode.value.length === 8 && !pairing.value)

onMounted(loadCores)

async function loadCores() {
	loading.value = true
	try {
		cores.value = await amberite.auth
			.currentUser()
			.then(async (user) => (user ? await client.amberite.cores_v1.list() : []))
	} catch (error) {
		notifyError('Could not load paired Cores', error)
	} finally {
		loading.value = false
	}
}

async function pairCore() {
	if (!canPair.value) return
	pairing.value = true
	let claim: Amberite.Cores.v1.PairingClaim | null = null
	let setupSubmitted = false
	try {
		const user = await amberite.auth.currentUser()
		if (!user) throw new Error('Sign in before pairing a Core.')
		claim = await client.amberite.cores_v1.claimPairing(normalizedCode.value)
		if (!claim) throw new Error('No Core is waiting with that pairing code.')
		const coreUrl = resolveCoreUrl(claim)
		if (!coreUrl) throw new Error('That Core did not provide a reachable connection URL.')
		const setup = await amberite.core.completeSetupAt(coreUrl, {
			code: normalizedCode.value,
			convex_url: config.convexUrl,
			auth_jwks_url: `${config.convexSiteUrl.replace(/\/$/, '')}/.well-known/jwks.json`,
			auth_audience: 'convex',
			owner_user_id: user.userId,
			owner_display_name: user.name,
			sync_credential: claim.syncCredential,
		})
		setupSubmitted = true
		if (setup.core_id !== claim.coreId) throw new Error('The Core answered with another identity.')
		setConnectedCore({ coreId: claim.coreId, url: coreUrl })
		amberite.core.clearCoreUrlCache()
		await client.amberite.cores_v1.finalizePairing({
			code: normalizedCode.value,
			coreId: claim.coreId,
			connectionUrl: coreUrl,
		})
		code.value = ''
		await loadCores()
		notifications.addNotification({ type: 'success', title: 'Core paired' })
	} catch (error) {
		if (claim && !setupSubmitted)
			await client.amberite.cores_v1
				.releasePairing(normalizedCode.value, claim.coreId)
				.catch(() => undefined)
		// TODO: If Core accepted setup but Convex finalization failed, expose an explicit retry state.
		// Account ownership transfers and paid/group-owned Core deletion are intentionally unsupported.
		notifyError('Could not pair Core', error)
	} finally {
		pairing.value = false
	}
}

function connectLocally(core: { coreId: string; connectionUrl?: string }) {
	if (!core.connectionUrl) return
	setConnectedCore({ coreId: core.coreId, url: core.connectionUrl })
	amberite.core.clearCoreUrlCache()
}

function disconnectLocally() {
	clearConnectedCore()
	amberite.core.clearCoreUrlCache()
}

function resolveCoreUrl(claim: {
	connectionUrl?: string
	metadata?: { bindHost?: string; port?: number }
}) {
	if (claim.connectionUrl) return claim.connectionUrl.replace(/\/$/, '')
	if (!claim.metadata?.port) return null
	const host = claim.metadata.bindHost ?? '127.0.0.1'
	if (!['127.0.0.1', 'localhost', '::1'].includes(host)) return null
	return `http://127.0.0.1:${claim.metadata.port}`
}

function notifyError(title: string, error: unknown) {
	notifications.addNotification({
		type: 'error',
		title,
		text: error instanceof Error ? error.message : String(error),
	})
}
</script>

<template>
	<div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-6">
		<header>
			<h1 class="m-0 text-2xl font-semibold text-contrast">Core</h1>
			<p class="m-0 mt-2 text-secondary">
				Pair the self-hosted Core that manages servers for your group.
			</p>
		</header>

		<section class="universal-card flex flex-col gap-4">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">Pair a Core</h2>
				<p class="m-0 mt-1 text-secondary">Enter the eight-character code shown by Core.</p>
			</div>
			<div class="flex items-center gap-3">
				<StyledInput
					v-model="code"
					class="max-w-sm font-mono uppercase"
					:maxlength="9"
					placeholder="ABCD-EFGH"
					@keydown.enter="pairCore"
				/>
				<Button type="colored" color="brand" :disabled="!canPair" @click="pairCore">
					<LinkIcon />
					{{ pairing ? 'Pairing…' : 'Pair' }}
				</Button>
			</div>
		</section>

		<section class="universal-card flex flex-col gap-4">
			<div class="flex items-center justify-between gap-3">
				<h2 class="m-0 text-lg font-semibold text-contrast">Your Cores</h2>
				<Button :disabled="loading" @click="loadCores">
					<RefreshCwIcon :class="{ 'animate-spin': loading }" /> Refresh
				</Button>
			</div>
			<EmptyState
				v-if="!loading && cores.length === 0"
				type="empty"
				heading="No Core paired"
				description="Start Core and enter its pairing code above."
			>
				<template #icon><ServerStackIcon /></template>
			</EmptyState>
			<div
				v-for="core in cores"
				v-else
				:key="core.coreId"
				class="flex items-center gap-3 rounded-xl bg-surface-2 p-4"
			>
				<ServerStackIcon class="size-6" />
				<div class="min-w-0 flex-1">
					<div class="truncate font-semibold text-contrast">{{ core.coreId }}</div>
					<div class="truncate text-sm text-secondary">
						{{ core.connectionUrl ?? 'No direct URL' }}
					</div>
				</div>
				<Button
					v-if="connectedCore?.coreId !== core.coreId"
					:disabled="!core.connectionUrl"
					@click="connectLocally(core)"
				>
					Connect
				</Button>
				<Button v-else @click="disconnectLocally"><UnlinkIcon /> Disconnect app</Button>
			</div>
		</section>
	</div>
</template>
