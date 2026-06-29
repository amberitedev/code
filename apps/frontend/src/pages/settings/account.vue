<template>
	<div>
		<ConfirmModal
			ref="modalConfirm"
			:title="formatMessage(messages.deleteAccountConfirmTitle)"
			:description="formatMessage(messages.deleteAccountConfirmDescription)"
			:proceed-label="formatMessage(messages.deleteAccountConfirmProceed)"
			:confirmation-text="auth.user.username"
			:has-to-type="true"
			@proceed="deleteAccount"
		/>

		<section class="universal-card">
			<h2 class="text-2xl">{{ formatMessage(messages.signInMethodTitle) }}</h2>
			<div class="adjacent-input">
				<label>
					<span class="label__title">{{ formatMessage(messages.minecraftFieldTitle) }}</span>
					<span class="label__description">
						{{ formatMessage(messages.minecraftFieldDescription) }}
					</span>
				</label>
				<div class="inline-flex items-center gap-2 text-primary">
					<BoxIcon />
					{{ formatMessage(messages.minecraftConnectedLabel) }}
				</div>
			</div>
		</section>

		<section class="universal-card">
			<h2 class="text-2xl">{{ formatMessage(messages.modrinthLinkTitle) }}</h2>
			<p>{{ formatMessage(messages.modrinthLinkDescription) }}</p>
			<div v-if="linkedModrinthAccount" class="adjacent-input">
				<label>
					<span class="label__title">{{ linkedModrinthAccount.username }}</span>
					<span class="label__description">
						{{ formatMessage(messages.modrinthLinkedDescription) }}
					</span>
				</label>
				<ButtonStyled color="red">
					<button @click="disconnectModrinthAccount">
						<TrashIcon />
						{{ formatMessage(commonMessages.removeButton) }}
					</button>
				</ButtonStyled>
			</div>
			<div v-else class="flex flex-col gap-2.5">
				<label for="modrinth-token">
					<span class="label__title">{{ formatMessage(messages.modrinthTokenLabel) }}</span>
					<span class="label__description">
						{{ formatMessage(messages.modrinthTokenDescription) }}
					</span>
				</label>
				<StyledInput
					id="modrinth-token"
					v-model="modrinthToken"
					type="text"
					autocomplete="off"
					wrapper-class="w-full"
					:placeholder="formatMessage(messages.modrinthTokenPlaceholder)"
				/>
				<div>
					<ButtonStyled color="brand">
						<button :disabled="!modrinthToken.trim() || linkingModrinth" @click="linkModrinthAccount">
							<PlusIcon />
							{{ formatMessage(messages.modrinthLinkButton) }}
						</button>
					</ButtonStyled>
				</div>
			</div>
		</section>

		<section id="data-export" class="universal-card">
			<h2>{{ formatMessage(messages.dataExportTitle) }}</h2>
			<p>{{ formatMessage(messages.dataExportDescription) }}</p>
			<ButtonStyled v-if="generated">
				<a :href="generated" download="export.json">
					<DownloadIcon />
					{{ formatMessage(messages.downloadExportButton) }}
				</a>
			</ButtonStyled>
			<ButtonStyled v-else>
				<button :disabled="generatingExport" @click="exportData">
					<template v-if="generatingExport">
						<UpdatedIcon /> {{ formatMessage(messages.generatingExportButton) }}
					</template>
					<template v-else>
						<UpdatedIcon /> {{ formatMessage(messages.generateExportButton) }}
					</template>
				</button>
			</ButtonStyled>
		</section>

		<section id="delete-account" class="universal-card">
			<h2>{{ formatMessage(messages.deleteAccountSectionTitle) }}</h2>
			<p>{{ formatMessage(messages.deleteAccountSectionDescription) }}</p>
			<ButtonStyled color="red">
				<button type="button" @click="modalConfirm.show()">
					<TrashIcon />
					{{ formatMessage(messages.deleteAccountButton) }}
				</button>
			</ButtonStyled>
		</section>
	</div>
</template>

<script setup lang="ts">
import type { LinkedModrinthAccount } from '@amberite/amberite-api'
import { BoxIcon, DownloadIcon, PlusIcon, TrashIcon, UpdatedIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	commonMessages,
	ConfirmModal,
	defineMessages,
	injectNotificationManager,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'

import {
	useAmberiteAuthClient,
	useAmberiteSocialClient,
} from '~/composables/amberite-client.ts'

definePageMeta({
	middleware: 'auth',
})

const { addNotification } = injectNotificationManager()
const auth = await useAuth()
const amberiteAuthClient = useAmberiteAuthClient()
const amberiteSocialClient = useAmberiteSocialClient()
const { formatMessage } = useVIntl()

const linkedModrinthAccount = ref<LinkedModrinthAccount | null>(null)
const modrinthToken = ref('')
const linkingModrinth = ref(false)
const modalConfirm = ref()

const messages = defineMessages({
	deleteAccountConfirmTitle: {
		id: 'settings.account.delete.confirm.title',
		defaultMessage: 'Are you sure you want to delete your account?',
	},
	deleteAccountConfirmDescription: {
		id: 'settings.account.delete.confirm.description',
		defaultMessage:
			'This will **immediately delete all of your user data and follows**. This will not delete your projects. Deleting your account cannot be reversed.',
	},
	deleteAccountConfirmProceed: {
		id: 'settings.account.delete.confirm.proceed',
		defaultMessage: 'Delete this account',
	},
	signInMethodTitle: {
		id: 'settings.account.sign-in-method.title',
		defaultMessage: 'Sign-in method',
	},
	minecraftFieldTitle: {
		id: 'settings.account.sign-in-method.minecraft.title',
		defaultMessage: 'Minecraft',
	},
	minecraftFieldDescription: {
		id: 'settings.account.sign-in-method.minecraft.description',
		defaultMessage: 'Amberite uses your Minecraft account as your only sign-in method.',
	},
	minecraftConnectedLabel: {
		id: 'settings.account.sign-in-method.minecraft.connected',
		defaultMessage: 'Connected',
	},
	modrinthLinkTitle: {
		id: 'settings.account.modrinth-link.title',
		defaultMessage: 'Linked Modrinth account',
	},
	modrinthLinkDescription: {
		id: 'settings.account.modrinth-link.description',
		defaultMessage:
			'Link Modrinth for content management. This is separate from Amberite sign-in.',
	},
	modrinthLinkedDescription: {
		id: 'settings.account.modrinth-link.linked.description',
		defaultMessage: 'This Modrinth account is linked for content features.',
	},
	modrinthTokenLabel: {
		id: 'settings.account.modrinth-link.token.label',
		defaultMessage: 'Modrinth access token',
	},
	modrinthTokenDescription: {
		id: 'settings.account.modrinth-link.token.description',
		defaultMessage: 'This token links your Modrinth account; it is not used to sign in to Amberite.',
	},
	modrinthTokenPlaceholder: {
		id: 'settings.account.modrinth-link.token.placeholder',
		defaultMessage: 'Paste a Modrinth token',
	},
	modrinthLinkButton: {
		id: 'settings.account.modrinth-link.action.link',
		defaultMessage: 'Link Modrinth',
	},
	modrinthLinkSuccess: {
		id: 'settings.account.modrinth-link.notification.linked',
		defaultMessage: 'Modrinth account linked.',
	},
	modrinthDisconnectSuccess: {
		id: 'settings.account.modrinth-link.notification.disconnected',
		defaultMessage: 'Modrinth account disconnected.',
	},
	dataExportTitle: {
		id: 'settings.account.data-export.title',
		defaultMessage: 'Data export',
	},
	dataExportDescription: {
		id: 'settings.account.data-export.description',
		defaultMessage: 'Request a copy of the personal data associated with your account.',
	},
	downloadExportButton: {
		id: 'settings.account.data-export.action.download',
		defaultMessage: 'Download export',
	},
	generatingExportButton: {
		id: 'settings.account.data-export.action.generating',
		defaultMessage: 'Generating export...',
	},
	generateExportButton: {
		id: 'settings.account.data-export.action.generate',
		defaultMessage: 'Generate export',
	},
	deleteAccountSectionTitle: {
		id: 'settings.account.delete.section.title',
		defaultMessage: 'Delete account',
	},
	deleteAccountSectionDescription: {
		id: 'settings.account.delete.section.description',
		defaultMessage:
			'Once you delete your account, there is no going back. Deleting your account will remove attached Amberite account data.',
	},
	deleteAccountButton: {
		id: 'settings.account.delete.section.action',
		defaultMessage: 'Delete account',
	},
})

await refreshLinkedModrinthAccount()

async function refreshLinkedModrinthAccount() {
	try {
		linkedModrinthAccount.value = await amberiteSocialClient.linkedModrinthAccount()
	} catch (error) {
		handleErrorNotification(error)
	}
}

async function linkModrinthAccount() {
	if (!modrinthToken.value.trim()) return

	linkingModrinth.value = true
	startLoading()
	try {
		linkedModrinthAccount.value = await amberiteSocialClient.storeModrinthOAuthTokens({
			accessToken: modrinthToken.value.trim(),
			scopes: [],
		})
		modrinthToken.value = ''
		addNotification({
			title: formatMessage(messages.modrinthLinkSuccess),
			type: 'success',
		})
	} catch (error) {
		handleErrorNotification(error)
	} finally {
		linkingModrinth.value = false
		stopLoading()
	}
}

async function disconnectModrinthAccount() {
	startLoading()
	try {
		await amberiteSocialClient.disconnectModrinthAccount()
		linkedModrinthAccount.value = null
		addNotification({
			title: formatMessage(messages.modrinthDisconnectSuccess),
			type: 'success',
		})
	} catch (error) {
		handleErrorNotification(error)
	} finally {
		stopLoading()
	}
}

async function deleteAccount() {
	startLoading()
	try {
		await amberiteAuthClient.deleteCurrentAccount()
	} catch (error) {
		handleErrorNotification(error)
	}

	await useAuth('none')
	window.location.href = '/'
	stopLoading()
}

const generatingExport = ref(false)
const generated = ref<string>()

async function exportData() {
	startLoading()
	generatingExport.value = true
	try {
		const res = await useBaseFetch('gdpr/export', {
			method: 'POST',
			internal: true,
		})

		const jsonString = JSON.stringify(res, null, 2)
		const blob = new Blob([jsonString], { type: 'application/json' })
		generated.value = URL.createObjectURL(blob)
	} catch (error) {
		handleErrorNotification(error)
	} finally {
		generatingExport.value = false
		stopLoading()
	}
}

function handleErrorNotification(error: unknown) {
	const maybeApiError = error as { data?: { description?: string } }
	addNotification({
		title: formatMessage(commonMessages.errorNotificationTitle),
		text: maybeApiError?.data?.description ?? (error instanceof Error ? error.message : String(error)),
		type: 'error',
	})
}
</script>
