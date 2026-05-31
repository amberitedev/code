<template>
	<Transition name="save-bar">
		<div
			v-if="visible"
			class="sticky bottom-0 z-10 mt-4 flex items-center justify-between gap-3 rounded-xl border border-solid border-divider bg-bg-raised px-4 py-3 shadow-lg"
		>
			<span class="text-sm font-bold text-contrast">You have unsaved changes.</span>
			<div class="flex gap-2">
				<ButtonStyled>
					<button type="button" :disabled="saving" @click="$emit('reset')">Reset</button>
				</ButtonStyled>
				<ButtonStyled color="brand">
					<button type="button" :disabled="saving" @click="$emit('save')">
						<SpinnerIcon v-if="saving" class="animate-spin" />
						<SaveIcon v-else />
						{{ saving ? 'Saving…' : 'Save changes' }}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</Transition>
</template>

<script setup lang="ts">
import { SaveIcon, SpinnerIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'

defineProps<{ visible: boolean; saving?: boolean }>()
defineEmits<{ save: []; reset: [] }>()
</script>

<style scoped>
.save-bar-enter-active,
.save-bar-leave-active {
	transition:
		opacity 0.15s ease,
		transform 0.15s ease;
}
.save-bar-enter-from,
.save-bar-leave-to {
	opacity: 0;
	transform: translateY(0.5rem);
}
</style>
