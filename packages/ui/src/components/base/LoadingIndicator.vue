<template>
	<div class="w-full flex items-center justify-center text-secondary">
		<SpinnerIcon class="size-6 animate-spin" aria-hidden="true" />
		<span class="sr-only">Loading...</span>
	</div>
</template>

<script setup>
import { SpinnerIcon } from '@modrinth/assets'
</script>
