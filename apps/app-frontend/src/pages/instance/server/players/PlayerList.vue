<template>
	<div>
		<p v-if="!names.length" class="m-0 text-secondary">{{ emptyLabel }}</p>
		<div v-else class="flex flex-col gap-2">
			<div
				v-for="name in names"
				:key="name"
				class="flex items-center justify-between gap-3 rounded-xl bg-surface-2 px-3 py-2"
			>
				<span class="font-semibold text-contrast">{{ name }}</span>
				<button class="list-action" :disabled="disabled" @click="emit('action', name)">
					{{ actionLabel }}
				</button>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
withDefaults(
	defineProps<{
		names: string[]
		actionLabel: string
		emptyLabel?: string
		disabled?: boolean
	}>(),
	{
		emptyLabel: 'Empty.',
		disabled: false,
	},
)

const emit = defineEmits<{
	action: [name: string]
}>()
</script>

<style scoped>
.list-action {
	border-radius: 0.5rem;
	background: var(--color-button-bg);
	padding: 0.25rem 0.75rem;
	font-weight: 600;
	color: var(--color-contrast);
}
.list-action:disabled {
	opacity: 0.5;
	cursor: not-allowed;
}
.list-action:not(:disabled):hover {
	filter: brightness(1.1);
}
</style>
