<script setup lang="ts">
import { ExternalIcon, ModrinthIcon } from '@modrinth/assets'
import { Avatar, ButtonStyled, injectNotificationManager } from '@modrinth/ui'

import { useModrinthLink } from '@/composables/useModrinthLink'

const { handleError } = injectNotificationManager()
const modrinthLink = useModrinthLink()

async function linkModrinth() {
	await modrinthLink.link().catch(handleError)
}

async function unlinkModrinth() {
	await modrinthLink.unlink().catch(handleError)
}
</script>

<template>
	<div class="flex flex-col gap-4">
		<div>
			<h2 class="m-0 text-lg font-semibold text-contrast">Accounts</h2>
			<p class="m-0 mt-1 text-sm text-secondary">
				Manage accounts connected to Amberite.
			</p>
		</div>

		<div class="rounded-xl border border-solid border-surface-5 bg-surface-2 p-4">
			<div class="flex items-center gap-3">
				<div class="grid size-10 shrink-0 place-content-center rounded-lg bg-surface-3">
					<ModrinthIcon class="size-5" />
				</div>
				<div class="min-w-0 flex-1">
					<h3 class="m-0 text-base font-semibold text-contrast">Modrinth</h3>
					<p class="m-0 mt-1 text-sm text-secondary">
						Connect Modrinth for project, creator, and profile features.
					</p>
				</div>
				<div v-if="modrinthLink.linked.value" class="flex min-w-0 items-center gap-2">
					<Avatar :src="modrinthLink.user.value?.avatar_url" size="32px" circle />
					<span class="max-w-32 truncate text-sm font-semibold text-primary">
						{{ modrinthLink.user.value?.username ?? 'Linked' }}
					</span>
				</div>
			</div>

			<div class="mt-4 flex justify-end">
				<ButtonStyled v-if="modrinthLink.linked.value">
					<button :disabled="modrinthLink.loading.value" @click="unlinkModrinth">
						Unlink
					</button>
				</ButtonStyled>
				<ButtonStyled v-else color="brand">
					<button :disabled="modrinthLink.loading.value" @click="linkModrinth">
						<ExternalIcon />
						Link Modrinth account
					</button>
				</ButtonStyled>
			</div>
		</div>
	</div>
</template>
