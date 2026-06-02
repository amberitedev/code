<script setup lang="ts">
import { ArrowLeftIcon, LinkIcon, XIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CoreConnectPage' })

const route = useRoute()
const router = useRouter()
const { currentUser, acceptInvite } = useSocial()

const code = ref('')
const accepting = ref(false)
const success = ref(false)
const error = ref<string | null>(null)

const isInviteCode = computed(() => code.value.trim().toUpperCase().startsWith('AMB-'))
const inviteCode = computed(() => code.value.trim().toUpperCase())

async function connect() {
	error.value = null
	success.value = false
	if (!code.value.trim()) {
		error.value = 'Enter a pairing or invite code.'
		return
	}
	accepting.value = true
	try {
		if (isInviteCode.value) {
			await acceptInvite({ code: inviteCode.value })
		} else {
			error.value =
				'Core pairing codes are not yet supported in the desktop app. ' +
				'Ask the group owner for an invite code (AMB-XXXXXX) instead.'
			accepting.value = false
			return
		}
		success.value = true
	} catch (e) {
		error.value = e instanceof Error ? e.message : String(e)
	} finally {
		accepting.value = false
	}
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
				<span class="text-contrast font-semibold">Connect to a Core</span>
			</div>
			<h1 class="m-0">Connect to a Core</h1>
			<p class="m-0 text-secondary">
				Got a Core running already? Enter an invite code below to join its friend group.
			</p>
		</header>

		<div class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-4">
			<label class="flex flex-col gap-2">
				<span class="font-semibold">Invite code</span>
				<input
					v-model="code"
					class="rounded-lg bg-bg-input px-3 py-3 font-mono text-lg tracking-widest"
					placeholder="AMB-ABC123"
					spellcheck="false"
					autocapitalize="characters"
					@keyup.enter="connect"
				/>
				<span class="text-secondary text-xs">
					Invite codes look like <code class="font-mono">AMB-XXXXXX</code>. Ask the group owner to
					generate one for you.
				</span>
			</label>

			<p v-if="error" class="m-0 text-red text-sm">{{ error }}</p>
			<p v-if="success" class="m-0 text-green text-sm">Joined! Loading your group…</p>

			<div class="flex items-center gap-2 justify-end pt-2">
				<ButtonStyled @click="back"><XIcon /> Cancel</ButtonStyled>
				<ButtonStyled color="brand" :disabled="accepting" @click="connect">
					<LinkIcon />
					{{ accepting ? 'Joining…' : 'Join friend group' }}
				</ButtonStyled>
			</div>
		</div>

		<div v-if="!currentUser" class="rounded-2xl bg-bg-raised p-6 flex flex-col gap-2 text-center">
			<h3 class="m-0">Not signed in</h3>
			<p class="m-0 text-secondary text-sm">Sign in to Amberite before joining a friend group.</p>
		</div>
	</div>
</template>
