<script setup>
import { ServerStackIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'

defineProps({
	coreName: { type: String, default: null },
})

defineEmits(['setup'])
</script>

<template>
	<section class="hero-card">
		<div>
			<p class="m-0 text-xs font-black uppercase tracking-[0.35em] text-brand">Amberite Core</p>
			<h1 class="m-0 mt-3 text-4xl font-black text-contrast">
				{{ coreName ?? 'No Core connected' }}
			</h1>
			<p class="m-0 mt-2 max-w-2xl text-secondary">
				Manage the permanent Core and friend group relationship. Instance/profile hooks are
				intentionally left for the active instance-page work.
			</p>
		</div>
		<ButtonStyled color="brand">
			<button @click="$emit('setup')"><ServerStackIcon /> Setup Core</button>
		</ButtonStyled>
	</section>
</template>

<style scoped>
.hero-card {
	@apply flex flex-col justify-between gap-5 rounded-[2rem] border border-solid border-highlight bg-bg-raised p-6 shadow-2xl md:flex-row md:items-center;
}
</style>
