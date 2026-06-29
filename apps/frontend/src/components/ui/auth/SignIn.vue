<template>
	<div v-if="subtleLauncherRedirectUri">
		<iframe
			:src="subtleLauncherRedirectUri"
			class="fixed left-0 top-0 z-[9999] m-0 h-full w-full border-0 p-0"
		></iframe>
	</div>
	<div
		v-else
		class="universal-card mx-auto flex w-full max-w-[27rem] flex-col gap-6 border border-solid border-surface-5 !p-6"
	>
		<div class="flex flex-col gap-5">
			<div class="text-center text-2xl font-semibold text-contrast">
				{{ formatMessage(messages.signInWithMinecraft) }}
			</div>
			<p class="m-0 text-center text-secondary">
				{{ formatMessage(messages.minecraftOnlyDescription) }}
			</p>

			<ButtonStyled color="brand">
				<a
					class="!w-full !justify-center !shadow-none"
					:href="getMinecraftAuthUrl('signin', redirectTarget)"
				>
					<BoxIcon />
					<span>{{ formatMessage(messages.continueWithMinecraft) }}</span>
					<RightArrowIcon />
				</a>
			</ButtonStyled>

			<div class="flex flex-wrap items-center justify-center gap-2.5 !text-base">
				<span>{{ formatMessage(messages.noAccountLabel) }}</span>
				<NuxtLink
					class="inline text-link"
					:to="{
						path: '/auth/sign-up',
						query: routeQuery,
					}"
				>
					{{ formatMessage(messages.createAccountLabel) }}
				</NuxtLink>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { BoxIcon, RightArrowIcon } from '@modrinth/assets'
import { ButtonStyled, defineMessages, useVIntl } from '@modrinth/ui'
import type { LocationQuery } from 'vue-router'

import { getMinecraftAuthUrl } from '@/composables/auth.ts'

interface Props {
	subtleLauncherRedirectUri?: string
	redirectTarget?: string
	routeQuery?: LocationQuery
}

const {
	subtleLauncherRedirectUri = '',
	redirectTarget = '',
	routeQuery = {},
} = defineProps<Props>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	signInWithMinecraft: {
		id: 'auth.sign-in.sign-in-with-minecraft',
		defaultMessage: 'Sign in with Minecraft',
	},
	minecraftOnlyDescription: {
		id: 'auth.sign-in.minecraft-only.description',
		defaultMessage: 'Amberite uses your Minecraft account as your sign-in identity.',
	},
	continueWithMinecraft: {
		id: 'auth.continue-with-minecraft',
		defaultMessage: 'Continue with Minecraft',
	},
	noAccountLabel: {
		id: 'auth.sign-in.no-account',
		defaultMessage: "Don't have an account?",
	},
	createAccountLabel: {
		id: 'auth.sign-in.create-account',
		defaultMessage: 'Sign up',
	},
})
</script>
