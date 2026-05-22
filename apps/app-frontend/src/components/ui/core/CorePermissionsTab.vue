<script setup>
defineProps({
	localMembers: { type: Array, required: true },
	presets: { type: Array, required: true },
})

defineEmits(['set-member-preset'])
</script>

<template>
	<section class="grid gap-4 md:grid-cols-2">
		<div
			v-for="preset in presets"
			:key="preset.id"
			class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5"
		>
			<h2 class="m-0 mb-3 text-xl font-black text-contrast">{{ preset.name }}</h2>
			<p>{{ preset.description }}</p>
			<code>{{ JSON.stringify(preset.permissions) }}</code>
		</div>

		<div
			class="rounded-[1.4rem] border border-solid border-button-border bg-bg-raised p-5 md:col-span-2"
		>
			<h2 class="m-0 mb-3 text-xl font-black text-contrast">Assign presets</h2>
			<div v-for="member in localMembers" :key="member.user_id" class="member-row">
				<span>{{ member.display_name ?? member.user_id }}</span>
				<select
					:value="member.permission_preset"
					@change="$emit('set-member-preset', member.user_id, $event.target.value)"
				>
					<option v-for="preset in presets" :key="preset.id" :value="preset.id">
						{{ preset.name }}
					</option>
				</select>
			</div>
		</div>
	</section>
</template>

<style scoped>
.member-row {
	@apply flex flex-wrap items-center justify-between gap-3 border-0 border-b border-solid border-button-border py-3;
}
</style>
