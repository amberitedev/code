<template>
	<div
		class="universal-card flex w-full max-w-[27rem] flex-col gap-6 border border-solid border-surface-5 !p-6"
	>
		<div class="mx-auto text-center text-2xl font-semibold text-contrast">
			{{ formatMessage(messages.createAccountTitle) }}
		</div>
		<p class="m-0 text-center text-secondary">
			{{ formatMessage(messages.minecraftOnlyDescription) }}
		</p>

		<ButtonStyled color="brand">
			<a
				class="!w-full !justify-center !shadow-none"
				:href="getMinecraftAuthUrl('signup', redirectTarget)"
			>
				<BoxIcon />
				<span>{{ formatMessage(messages.continueWithMinecraft) }}</span>
				<RightArrowIcon />
			</a>
		</ButtonStyled>

		<p v-if="!routeQuery.launcher" class="m-0 text-center">
			<IntlFormatted :message-id="messages.legalDisclaimer">
				<template #terms-link="{ children }">
					<NuxtLink to="/legal/terms" class="text-link">
						<component :is="() => children" />
					</NuxtLink>
				</template>
				<template #privacy-policy-link="{ children }">
					<NuxtLink to="/legal/privacy" class="text-link">
						<component :is="() => children" />
					</NuxtLink>
				</template>
			</IntlFormatted>
		</p>

		<div class="mx-auto flex flex-wrap items-center justify-start gap-2 text-center">
			{{ formatMessage(messages.alreadyHaveAccountLabel) }}
			<NuxtLink
				class="mr-1 text-link"
				:to="{
					path: '/auth/sign-in',
					query: routeQuery,
				}"
			>
				{{ formatMessage(commonMessages.signInButton) }}
			</NuxtLink>
		</div>
	</div>
</template>

<script setup lang="ts">
import { BoxIcon, RightArrowIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	commonMessages,
	defineMessages,
	IntlFormatted,
	useVIntl,
} from '@modrinth/ui'
import type { LocationQuery } from 'vue-router'

import { getMinecraftAuthUrl } from '@/composables/auth.ts'

interface Props {
	redirectTarget?: string
	routeQuery?: LocationQuery
}

const { redirectTarget = '', routeQuery = {} } = defineProps<Props>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	createAccountTitle: {
		id: 'auth.sign-up.title.sign-up-with-minecraft',
		defaultMessage: 'Create an Amberite account',
	},
	minecraftOnlyDescription: {
		id: 'auth.sign-up.minecraft-only.description',
		defaultMessage: 'Amberite accounts are created with Minecraft sign-in.',
	},
	continueWithMinecraft: {
		id: 'auth.continue-with-minecraft',
		defaultMessage: 'Continue with Minecraft',
	},
	legalDisclaimer: {
		id: 'auth.sign-up.legal-dislaimer',
		defaultMessage:
			"By creating an account, you agree to Modrinth's <terms-link>Terms</terms-link> and <privacy-policy-link>Privacy Policy</privacy-policy-link>.",
	},
	alreadyHaveAccountLabel: {
		id: 'auth.sign-up.sign-in-option.title',
		defaultMessage: 'Already have an account?',
	},
})
</script>
