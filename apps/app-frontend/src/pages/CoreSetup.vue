<script setup lang="ts">
import type { CoreApiClient } from '@amberite/amberite-api'
import { CheckIcon, ServerIcon, ServerPlusIcon } from '@modrinth/assets'
import { ButtonStyled, StyledInput } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { ensureAmberiteSession } from '@/helpers/amberite-auth'
import {
	coreClient,
	getCoreAppSettings,
	installCore,
	isCoreInstalled,
	setCoreAppSettings,
	startCore,
} from '@/helpers/amberite-core'
import {
	claimPairingCore,
	currentUser,
	ensureCoreFriendGroup,
	ensureSocialProfile,
} from '@/helpers/friend-groups'

defineOptions({
	name: 'CoreSetupPage',
})

const router = useRouter()
const mode = ref('remote')
const coreName = ref('Amberite Core')
const code = ref('')
const coreDownloadUrl = ref(import.meta.env.VITE_CORE_DOWNLOAD_URL ?? '')
const coreUrl = ref('http://localhost:16662')
const jwksUrl = ref(
	import.meta.env.VITE_AUTH_JWKS_URL ?? `${coreClient().adapter.convexUrl}/.well-known/jwks.json`,
)
const busy = ref(false)
const error = ref(null)

onMounted(async () => {
	coreUrl.value =
		(await getCoreAppSettings().catch(() => null))?.core_url ?? 'http://localhost:16662'
})

const canSubmit = computed(() => coreName.value.trim() && jwksUrl.value.trim())

function pick(nextMode) {
	mode.value = nextMode
}

async function finishSetup() {
	if (!canSubmit.value) return
	busy.value = true
	error.value = null
	try {
		await ensureAmberiteSession({ interactive: true })
		const owner = await currentUser()
		if (!owner?.userId) throw new Error('Sign in to Amberite before pairing Core')

		if (mode.value === 'local') {
			if (!(await isCoreInstalled())) {
				if (!coreDownloadUrl.value.trim()) throw new Error('Core download URL is not configured')
				await installCore(coreDownloadUrl.value.trim())
			}
			coreUrl.value = await startCore()
		}

		await setCoreAppSettings({
			core_url: coreUrl.value || null,
			display_name: null,
			auto_launch_core: mode.value === 'local',
		})
		const client = coreClient()
		const response = await client.completeSetup({
			...(mode.value === 'local'
				? { local_setup_secret: await getLocalSetupSecret(client) }
				: { code: code.value.trim() }),
			convex_url: client.adapter.convexUrl,
			auth_jwks_url: jwksUrl.value,
			owner_user_id: owner.userId,
		})
		if (mode.value === 'remote') {
			await claimPairingCore({ code: code.value.trim() }).catch(() => null)
		}
		await ensureSocialProfile().catch(() => null)
		await ensureCoreFriendGroup({
			coreId: response.core_id,
			name: coreName.value,
			setupMode: mode.value,
			connectionUrl: coreUrl.value,
		})
		await router.push('/core')
	} catch (err) {
		error.value = err instanceof Error ? err.message : String(err)
	} finally {
		busy.value = false
	}
}

async function getLocalSetupSecret(client: CoreApiClient) {
	const secret = await client.adapter.getLocalSetupSecret?.()
	if (!secret) throw new Error('Local Core setup secret is not available')
	return secret
}
</script>

<template>
	<div class="min-h-full bg-gradient-to-br from-bg via-bg to-brand-highlight/20 p-6">
		<div class="mx-auto flex max-w-5xl flex-col gap-6">
			<div class="rounded-[2rem] border border-solid border-highlight bg-bg-raised p-6 shadow-2xl">
				<p class="m-0 text-sm font-bold uppercase tracking-[0.3em] text-brand">Core setup</p>
				<h1 class="m-0 mt-3 text-4xl font-black text-contrast">
					Claim the Core that anchors your friend group.
				</h1>
				<p class="m-0 mt-3 max-w-2xl text-lg text-secondary">
					One Core, one group. This page stores the connection and creates the friend-group shell
					the instances page can hook into later.
				</p>
			</div>

			<div class="grid gap-4 md:grid-cols-2">
				<button class="core-option" :class="{ active: mode === 'remote' }" @click="pick('remote')">
					<ServerIcon /><span>Connect running Core</span><small>Paste the CLI pairing code.</small>
				</button>
				<button class="core-option" :class="{ active: mode === 'local' }" @click="pick('local')">
					<ServerPlusIcon /><span>Install locally</span
					><small>Uses the local setup secret when available.</small>
				</button>
			</div>

			<div class="rounded-[1.5rem] border border-solid border-button-border bg-bg-raised p-5">
				<div class="grid gap-4 md:grid-cols-2">
					<StyledInput v-model="coreName" placeholder="Core name" wrapper-class="w-full" />
					<StyledInput v-model="coreUrl" placeholder="Core URL" wrapper-class="w-full" />
					<StyledInput
						v-if="mode === 'local'"
						v-model="coreDownloadUrl"
						placeholder="Core download URL"
						wrapper-class="w-full"
					/>
					<StyledInput v-model="jwksUrl" placeholder="JWKS URL" wrapper-class="w-full" />
					<StyledInput
						v-if="mode === 'remote'"
						v-model="code"
						placeholder="Pairing code"
						wrapper-class="w-full"
					/>
				</div>
				<p v-if="error" class="mb-0 mt-4 text-sm font-semibold text-red">{{ error }}</p>
				<div class="mt-5 flex justify-end">
					<ButtonStyled color="brand">
						<button :disabled="busy || !canSubmit" @click="finishSetup">
							<CheckIcon /> Finish setup
						</button>
					</ButtonStyled>
				</div>
			</div>
		</div>
	</div>
</template>

<style scoped>
.core-option {
	@apply flex min-h-36 flex-col items-start gap-2 rounded-3xl border border-solid border-button-border bg-bg-raised p-5 text-left text-primary transition-all;
}
.core-option svg {
	@apply h-7 w-7 text-brand;
}
.core-option span {
	@apply text-xl font-black text-contrast;
}
.core-option small {
	@apply text-sm text-secondary;
}
.core-option.active {
	@apply border-brand bg-brand-highlight text-contrast shadow-xl;
}
</style>
