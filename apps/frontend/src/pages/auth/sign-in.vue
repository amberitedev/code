<template>
	<SignInView
		:subtle-launcher-redirect-uri="subtleLauncherRedirectUri"
		:redirect-target="redirectTarget"
		:route-query="route.query"
	/>
</template>

<script setup lang="ts">
import {
	commonMessages,
	defineMessages,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import type { LocationQueryValue } from 'vue-router'

import SignInView from '@/components/ui/auth/SignIn.vue'
import { getLauncherRedirectUrl } from '@/composables/auth.ts'

const getQueryString = (
	value: LocationQueryValue | LocationQueryValue[] | null | undefined,
): string => {
	const firstValue = Array.isArray(value) ? value[0] : value
	return typeof firstValue === 'string' ? firstValue : ''
}

const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	signInTitle: {
		id: 'auth.sign-in.title',
		defaultMessage: 'Sign In',
	},
	minecraftSignInFailed: {
		id: 'auth.sign-in.minecraft.failed',
		defaultMessage: 'Minecraft sign-in failed. Please try again with a Minecraft account.',
	},
})

useHead({
	title() {
		return `${formatMessage(messages.signInTitle)} - Modrinth`
	},
})

const auth = await useAuth()
const route = useNativeRoute()
const redirectTarget = getQueryString(route.query.redirect)
const subtleLauncherRedirectUri = ref<string>()

if (route.query.error) {
	addNotification({
		title: formatMessage(commonMessages.errorNotificationTitle),
		text: formatMessage(messages.minecraftSignInFailed),
		type: 'error',
	})
}

if (auth.value.user) {
	await finishSignIn()
}

async function finishSignIn() {
	if (route.query.launcher) {
		const token = auth.value.token
		const redirectUrl = `${getLauncherRedirectUrl(route)}/?code=${token}`

		if (redirectUrl.startsWith('https://launcher-files.modrinth.com/')) {
			await navigateTo(redirectUrl, {
				external: true,
			})
		} else {
			// Preserve the existing launcher redirect behavior without exposing the token in the visible URL.
			subtleLauncherRedirectUri.value = redirectUrl
		}

		return
	}

	if (route.query.redirect) {
		const redirect = decodeURIComponent(getQueryString(route.query.redirect))
		await navigateTo(redirect, {
			replace: true,
		})
	} else {
		await navigateTo('/dashboard')
	}
}
</script>
