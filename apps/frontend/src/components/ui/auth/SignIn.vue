<template>
	<div
		class="universal-card mx-auto flex w-full max-w-[27rem] flex-col gap-6 border border-solid border-surface-5 !p-6"
	>
		<div class="flex flex-col gap-5">
			<div class="text-center text-2xl font-semibold text-contrast">
				{{ formatMessage(messages.continueWithMinecraft) }}
			</div>
			<p class="m-0 text-center text-secondary">
				{{ formatMessage(messages.minecraftOnlyDescription) }}
			</p>

			<div v-if="remembered" class="flex items-center gap-3 rounded-xl bg-surface-2 p-3">
				<Avatar :src="remembered.avatarUrl" :alt="remembered.verifiedMinecraftHandle" circle />
				<div class="min-w-0 text-left">
					<div class="truncate font-semibold text-contrast">{{ remembered.displayName }}</div>
					<div class="truncate text-sm text-secondary">
						@{{ remembered.verifiedMinecraftHandle }}
					</div>
				</div>
			</div>

			<ButtonStyled color="brand">
				<a
					class="!w-full !justify-center !shadow-none"
					:href="getMinecraftAuthUrl('continue', redirectTarget)"
					@click="setAuthVerifying"
				>
					<BoxIcon />
					<span>{{ continueLabel }}</span>
					<RightArrowIcon />
				</a>
			</ButtonStyled>

			<ButtonStyled>
				<a
					class="!w-full !justify-center"
					:href="getMinecraftAuthUrl('use_another_account', redirectTarget)"
					@click="setAuthVerifying"
				>
					{{ formatMessage(messages.useAnotherAccount) }}
				</a>
			</ButtonStyled>
		</div>
	</div>
</template>

<script setup lang="ts">
import { BoxIcon, RightArrowIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled, defineMessages, useVIntl } from '@modrinth/ui'
import type { LocationQuery } from 'vue-router'

import {
	getMinecraftAuthUrl,
	getRememberedAmberiteIdentity,
	setAuthVerifying,
} from '@/composables/auth.ts'

interface Props {
	redirectTarget?: string
	routeQuery?: LocationQuery
}

const { redirectTarget = '' } = defineProps<Props>()
const { formatMessage } = useVIntl()
const remembered = getRememberedAmberiteIdentity()
const continueLabel = computed(() =>
	remembered
		? `Continue as ${remembered.verifiedMinecraftHandle}`
		: formatMessage(messages.continueWithMinecraft),
)

const messages = defineMessages({
	continueWithMinecraft: {
		id: 'auth.continue-with-minecraft',
		defaultMessage: 'Continue with Minecraft',
	},
	minecraftOnlyDescription: {
		id: 'auth.sign-in.minecraft-only.description',
		defaultMessage:
			'Your verified Minecraft: Java Edition account creates or recovers one Amberite identity.',
	},
	useAnotherAccount: {
		id: 'auth.use-another-minecraft-account',
		defaultMessage: 'Use another account',
	},
})
</script>
