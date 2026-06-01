<script setup lang="ts">
import { ButtonStyled } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import { useCoreCall } from '@/composables/useCoreCall'
import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CoreSetupPage' })

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

const { data: status, loading, execute: loadStatus } = useCoreCall((c) => c.getSetupStatus())

const alreadyPaired = computed(() => status.value?.paired === true)

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
</script>

<template>
	<div class="flex flex-col gap-4 w-full">
		<div v-if="loading" class="text-secondary">Checking Core status…</div>

		<div v-else-if="alreadyPaired" class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-2">
			<span class="font-bold text-green">This Core is paired</span>
			<span class="text-secondary text-sm">Core ID: {{ status?.core_id }}</span>
			<span v-if="status?.dev_mode" class="text-secondary text-sm">Running in dev mode.</span>
			<div v-if="!group" class="flex flex-col gap-2 mt-2">
				<span class="text-secondary text-sm">No friend group is linked yet.</span>
				<input
					v-model="groupName"
					class="rounded-lg bg-bg-input px-3 py-2"
					placeholder="Group name (optional)"
				/>
				<ButtonStyled color="brand">
					<button
						@click="
							createGroup({
								coreId: status!.core_id,
								name: groupName.trim() || undefined,
								setupMode: 'remote',
							})
						"
					>
						Create friend group
					</button>
				</ButtonStyled>
			</div>
		</div>

		<div v-else class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3">
			<span class="font-bold">Pair a Core</span>
			<p class="m-0 text-secondary text-sm">
				Start your Core server, then enter the pairing code it prints below.
			</p>
			<label class="flex flex-col gap-1">
				<span class="font-semibold">Pairing code</span>
				<input v-model="code" class="rounded-lg bg-bg-input px-3 py-2" placeholder="XXXX-XXXX" />
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
			<div class="flex items-center gap-3">
				<ButtonStyled color="brand">
					<button :disabled="pairing || !convexUrl" @click="pair">
						{{ pairing ? 'Pairing…' : 'Pair Core' }}
					</button>
				</ButtonStyled>
				<span v-if="done" class="text-green text-sm">Paired!</span>
			</div>
			<p v-if="pairError" class="text-red text-sm m-0">{{ pairError }}</p>
		</div>
	</div>
</template>
