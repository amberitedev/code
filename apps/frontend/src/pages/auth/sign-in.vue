<template>
	<SignInView :redirect-target="redirectTarget" :route-query="route.query" />
</template>

<script setup lang="ts">
import { commonMessages, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import type { LocationQueryValue } from 'vue-router'

import SignInView from '@/components/ui/auth/SignIn.vue'

const getQueryString = (
	value: LocationQueryValue | LocationQueryValue[] | null | undefined,
): string => {
	const firstValue = Array.isArray(value) ? value[0] : value
	return typeof firstValue === 'string' ? firstValue : ''
}

const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()
const route = useNativeRoute()
const redirectTarget = normalizeRedirect(getQueryString(route.query.redirect))

const messages = defineMessages({
	signInTitle: { id: 'auth.sign-in.title', defaultMessage: 'Continue with Minecraft' },
})

useHead({ title: () => `${formatMessage(messages.signInTitle)} - Amberite` })

if (route.query.error) {
	addNotification({
		title: formatMessage(commonMessages.errorNotificationTitle),
		text: errorMessage(getQueryString(route.query.error)),
		type: 'error',
	})
}

const auth = await useAuth()
if (auth.value.user) await navigateTo(redirectTarget, { replace: true })

function normalizeRedirect(value: string): string {
	return value.startsWith('/') && !value.startsWith('//') ? value : '/dashboard'
}

function errorMessage(code: string): string {
	if (code === 'minecraft_auth_cancelled') return 'Minecraft sign-in was cancelled.'
	if (code === 'minecraft_uuid_mismatch')
		return 'That is not the remembered Minecraft account. Try again or use another account.'
	if (code === 'minecraft_java_profile_required')
		return 'This Microsoft account does not own a Minecraft: Java Edition profile.'
	if (code === 'minecraft_xbox_restricted')
		return 'Xbox account restrictions prevented Minecraft verification.'
	if (code === 'minecraft_provider_throttled')
		return 'Microsoft or Minecraft is temporarily limiting sign-in attempts. Try again shortly.'
	if (code === 'minecraft_provider_unreachable')
		return 'Microsoft or Minecraft could not be reached. Check your connection and try again.'
	return 'Minecraft sign-in failed. Please try again.'
}
</script>
