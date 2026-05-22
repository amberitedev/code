<script setup>
import { ButtonStyled, StyledInput, Toggle } from '@modrinth/ui'
import { ref } from 'vue'

defineProps({
	syncProfiles: { type: Array, required: true },
})

const emit = defineEmits(['register-profile', 'remove-profile'])
const name = ref('')
const gameVersion = ref('')
const loader = ref('')

function registerProfile() {
	if (!name.value.trim()) return
	emit('register-profile', {
		name: name.value.trim(),
		game_version: gameVersion.value.trim() || null,
		loader: loader.value.trim() || null,
	})
	name.value = ''
	gameVersion.value = ''
	loader.value = ''
}
</script>

<template>
	<section class="grid gap-4 md:grid-cols-2">
		<div
			class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5 md:col-span-2"
		>
			<h2 class="m-0 mb-3 text-xl font-black text-contrast">Register synced profile</h2>
			<div class="grid gap-3 md:grid-cols-3">
				<StyledInput v-model="name" placeholder="Profile name" wrapper-class="w-full" />
				<StyledInput v-model="gameVersion" placeholder="Game version" wrapper-class="w-full" />
				<StyledInput v-model="loader" placeholder="Loader" wrapper-class="w-full" />
			</div>
			<ButtonStyled>
				<button class="mt-3" @click="registerProfile">Register profile</button>
			</ButtonStyled>
		</div>

		<div
			v-for="profile in syncProfiles"
			:key="profile.id"
			class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5"
		>
			<h2 class="m-0 mb-3 text-xl font-black text-contrast">{{ profile.name }}</h2>
			<p>{{ profile.loader ?? 'unknown loader' }} {{ profile.game_version ?? '' }}</p>
			<div class="flex items-center justify-between gap-3">
				<Toggle :id="`sync-${profile.id}`" :model-value="profile.sync_enabled" />
				<button class="text-sm font-semibold text-red" @click="$emit('remove-profile', profile.id)">
					Remove
				</button>
			</div>
		</div>

		<div
			v-if="syncProfiles.length === 0"
			class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5"
		>
			<h2 class="m-0 mb-3 text-xl font-black text-contrast">No synced profiles yet</h2>
			<p>
				Client-to-Core sync endpoints are ready. The instance page can register profiles when its
				state work lands.
			</p>
		</div>
	</section>
</template>
