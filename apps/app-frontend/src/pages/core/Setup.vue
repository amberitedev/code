<script setup lang="ts">
import { ArrowLeftIcon, ServerStackIcon, XIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useCoreCall } from '@/composables/useCoreCall'
import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CoreSetupPage' })

const route = useRoute()
const router = useRouter()
const { currentUser, group, createGroup, refresh } = useSocial()

const code = ref('')
const groupName = ref('')
const pairError = ref<string | null>(null)
const pairing = ref(false)
const done = ref(false)

const convexUrl = (import.meta.env.VITE_CONVEX_URL as string | undefined) ?? ''
const authJwksUrl =
	(import.meta.env.VITE_AUTH_JWKS_URL as string | undefined) ??
	(convexUrl ? `${convexUrl.replace('.convex.cloud', '.convex.site')}/.well-known/jwks.json` : '')

const {
	data: status,
	loading,
	error: statusError,
	execute: loadStatus,
} = useCoreCall((c) => c.getSetupStatus())

const alreadyPaired = computed(() => status.value?.paired === true)
const hasGroup = computed(() => group.value !== null)

onMounted(() => {
	void loadStatus()
})

async function pair() {
	pairError.value = null
	done.value = false
	if (!currentUser.value) {
		pairError.value = 'Sign in (or pick a dev identity) before pairing.'
		return
	}
	if (!code.value.trim()) {
		pairError.value = 'Enter the pairing code shown by your Core.'
		return
	}
	pairing.value = true
	try {
		const client = (await import('@/composables/useCoreClient')).useCoreClient()
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
		await Promise.all([loadStatus(), refresh()])
		done.value = true
	} catch (e) {
		pairError.value = e instanceof Error ? e.message : String(e)
	} finally {
		pairing.value = false
	}
}

async function createOnly() {
	if (!status.value?.core_id) return
	await createGroup({
		coreId: status.value.core_id,
		name: groupName.value.trim() || undefined,
		setupMode: 'remote',
	})
	await Promise.all([loadStatus(), refresh()])
}

function back() {
	if (route.query.redirect) {
		router.push(String(route.query.redirect))
	} else {
		router.push('/core')
	}
}
</script>

<template>
	<div class="flex flex-col gap-6 w-full max-w-3xl mx-auto">
		<button
			type="button"
			class="flex items-center gap-2 text-secondary hover:text-contrast transition-colors self-start bg-transparent border-0 p-0 cursor-pointer"
			@click="back"
		>
			<ArrowLeftIcon /> Back
		</button>

		<header class="flex flex-col gap-2">
			<div class="flex items-center gap-3 text-sm text-secondary">
				<span>Core</span>
				<span aria-hidden>›</span>
				<span class="text-contrast font-semibold">Set up a new Core</span>
			</div>
			<h1 class="m-0">Set up a new Core</h1>
			<p class="m-0 text-secondary">
				Start the Core server, then enter the pairing code it prints below to link it to your
				Amberite account.
			</p>
		</header>

		<div v-if="loading" class="rounded-2xl bg-bg-raised p-6 text-secondary">
			Checking Core status…
		</div>

		<div v-else-if="statusError" class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-2">
			<h3 class="m-0 text-red">Core unreachable</h3>
			<p class="m-0 text-secondary text-sm">
				Couldn't reach a paired Core on this machine. Start the Core server first; it prints a
				pairing code in the console.
			</p>
		</div>

		<div
			v-else-if="alreadyPaired && !hasGroup"
			class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-3"
		>
			<div class="flex items-center gap-2">
				<ServerStackIcon class="w-5 h-5 text-brand" />
				<span class="font-bold text-green">This Core is paired</span>
			</div>
			<span class="text-secondary text-sm">Core ID: {{ status?.core_id }}</span>
			<span v-if="status?.dev_mode" class="text-secondary text-sm">Running in dev mode.</span>
			<label class="flex flex-col gap-1 mt-2">
				<span class="font-semibold">Group name</span>
				<input
					v-model="groupName"
					class="rounded-lg bg-bg-input px-3 py-2"
					placeholder="My Core (optional)"
				/>
			</label>
			<div class="flex items-center gap-2 justify-end pt-2">
				<ButtonStyled @click="back"><XIcon /> Cancel</ButtonStyled>
				<ButtonStyled color="brand" @click="createOnly">Create friend group</ButtonStyled>
			</div>
		</div>

		<div v-else class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-3">
			<label class="flex flex-col gap-1">
				<span class="font-semibold">Pairing code</span>
				<input
					v-model="code"
					class="rounded-lg bg-bg-input px-3 py-2 font-mono"
					placeholder="XXXX-XXXX"
					@keyup.enter="pair"
				/>
				<span class="text-secondary text-xs">
					Shown in the Core server's console output on first start.
				</span>
			</label>
			<label class="flex flex-col gap-1">
				<span class="font-semibold">Group name</span>
				<input
					v-model="groupName"
					class="rounded-lg bg-bg-input px-3 py-2"
					placeholder="My Core (optional)"
				/>
			</label>
			<p v-if="!convexUrl" class="text-red text-sm m-0">
				VITE_CONVEX_URL is not set — pairing can't continue.
			</p>
			<div class="flex items-center gap-3 justify-end pt-2">
				<ButtonStyled @click="back"><XIcon /> Cancel</ButtonStyled>
				<ButtonStyled color="brand" :disabled="pairing || !convexUrl" @click="pair">
					{{ pairing ? 'Pairing…' : 'Pair Core' }}
				</ButtonStyled>
			</div>
			<p v-if="done" class="text-green text-sm m-0">Paired!</p>
			<p v-if="pairError" class="text-red text-sm m-0">{{ pairError }}</p>
		</div>
	</div>
</template>
