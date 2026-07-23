<script setup lang="ts">
const route = useNativeRoute()
const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : '/dashboard'
await navigateTo(redirect.startsWith('/') && !redirect.startsWith('//') ? redirect : '/dashboard', {
	replace: true,
})
</script>

<template><div /></template>
