<script setup lang="ts">
definePageMeta({
	middleware: [
		'auth',
		() => navigateTo('/hosting/manage', { redirectCode: 302 }),
	],
})
</script>

<template>
	<div />
</template>
