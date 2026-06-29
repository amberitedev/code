<template>
	<div />
</template>

<script setup lang="ts">
const auth = await useAuth()
await navigateTo(auth.value.user ? '/settings/account' : '/auth/sign-in', { replace: true })
</script>
