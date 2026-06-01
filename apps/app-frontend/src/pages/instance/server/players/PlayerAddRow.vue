<template>
	<form class="mb-3 flex gap-2" @submit.prevent="submit">
		<input
			v-model="value"
			type="text"
			:placeholder="placeholder"
			:disabled="disabled"
			class="flex-1 rounded-xl bg-surface-2 px-3 py-2 text-contrast"
		/>
		<ButtonStyled color="brand">
			<button type="submit" :disabled="disabled || !value.trim()"><PlusIcon /> Add</button>
		</ButtonStyled>
	</form>
</template>

<script setup lang="ts">
import { PlusIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'
import { ref } from 'vue'

defineProps<{
	placeholder: string
	disabled?: boolean
}>()

const emit = defineEmits<{
	add: [name: string]
}>()

const value = ref('')

function submit() {
	const name = value.value.trim()
	if (!name) return
	emit('add', name)
	value.value = ''
}
</script>
