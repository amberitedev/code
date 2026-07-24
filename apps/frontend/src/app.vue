<template>
	<div
		v-if="auth.status === 'restoring' || auth.status === 'verifying'"
		class="grid min-h-screen place-items-center bg-surface-1 px-6 text-center"
	>
		<div class="flex max-w-md flex-col items-center gap-3">
			<SpinnerIcon class="size-8 animate-spin text-brand" />
			<h1 class="m-0 text-2xl font-semibold text-contrast">
				{{
					auth.status === 'verifying'
						? 'Verifying your Minecraft account'
						: 'Restoring your session'
				}}
			</h1>
			<p class="m-0 text-secondary">This should only take a moment.</p>
		</div>
	</div>
	<div
		v-else-if="auth.status === 'retryableOffline'"
		class="grid min-h-screen place-items-center bg-surface-1 px-6 text-center"
	>
		<div class="universal-card flex max-w-md flex-col items-center gap-4 !p-6">
			<h1 class="m-0 text-2xl font-semibold text-contrast">Amberite is unreachable</h1>
			<p class="m-0 text-secondary">
				Your remembered identity is safe. Check your connection and try again.
			</p>
			<ButtonStyled color="brand"><button @click="retry">Try again</button></ButtonStyled>
		</div>
	</div>
	<NuxtLayout v-else>
		<NuxtRouteAnnouncer />
		<ClientOnly><LoadingBar /></ClientOnly>
		<NotificationPanel />
		<AdsConsentNotification />
		<I18nDebugPanel />
		<NuxtPage />
	</NuxtLayout>
</template>

<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import { ButtonStyled, I18nDebugPanel, LoadingBar, NotificationPanel } from '@modrinth/ui'

import AdsConsentNotification from '~/components/ui/AdsConsentNotification.vue'
import { setupProviders } from '~/providers/setup.ts'

import { getSignInRouteObj, retryAuthRestore, useAuth } from './composables/auth'

const authState = await useAuth()
const auth = computed(() => authState.value)
const route = useRoute()
setupProviders(authState)

async function retry() {
	const restored = await retryAuthRestore()
	if (restored.value.status === 'signedOut') await navigateTo(getSignInRouteObj(route))
}
</script>
