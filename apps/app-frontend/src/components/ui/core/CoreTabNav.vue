<script setup>
defineProps({
	tabs: { type: Array, required: true },
})

const selectedTab = defineModel({ type: String, required: true })
</script>

<template>
	<nav class="flex flex-wrap gap-2">
		<button
			v-for="tab in tabs"
			:key="tab"
			class="tab-button"
			:class="{ active: selectedTab === tab }"
			@click="selectedTab = tab"
		>
			{{ tab }}
		</button>
	</nav>
</template>

<style scoped>
.tab-button {
	@apply rounded-full border border-solid border-button-border bg-bg-raised px-4 py-2 text-sm font-bold capitalize text-primary;
}
.tab-button.active {
	@apply border-brand bg-brand-highlight text-contrast;
}
</style>
