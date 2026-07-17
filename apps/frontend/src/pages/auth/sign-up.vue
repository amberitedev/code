<script setup lang="ts">
const route = useNativeRoute()
await navigateTo({ path: '/auth/sign-in', query: route.query }, { replace: true })
</script>

<template><div /></template>
