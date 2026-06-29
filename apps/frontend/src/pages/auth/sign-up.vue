<template>
	<SignUpView :redirect-target="redirectTarget" :route-query="route.query" />
</template>

<script setup lang="ts">
import {
	commonMessages,
	defineMessages,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import type { LocationQueryValue } from 'vue-router'

import SignUpView from '@/components/ui/auth/SignUp.vue'

const getQueryString = (
	value: LocationQueryValue | LocationQueryValue[] | null | undefined,
): string => {
	const firstValue = Array.isArray(value) ? value[0] : value
	return typeof firstValue === 'string' ? firstValue : ''
}

const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'auth.sign-up.title',
		defaultMessage: 'Sign Up',
	},
	minecraftSignInFailed: {
		id: 'auth.sign-up.minecraft.failed',
		defaultMessage: 'Minecraft sign-up failed. Please try again with a Minecraft account.',
	},
})

useHead({
	title: () => `${formatMessage(messages.title)} - Modrinth`,
})

const auth = await useAuth()
const route = useNativeRoute()
const redirectTarget = getQueryString(route.query.redirect)

if (route.query.error) {
	addNotification({
		title: formatMessage(commonMessages.errorNotificationTitle),
		text: formatMessage(messages.minecraftSignInFailed),
		type: 'error',
	})
}

if (auth.value.user) {
	await navigateTo('/dashboard')
}
</script>
