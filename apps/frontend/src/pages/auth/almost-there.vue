<template>
	<div
		class="universal-card mx-auto flex w-full max-w-[32rem] flex-col gap-6 border border-solid border-surface-5 !p-6"
	>
		<h1 class="m-0 text-center text-2xl font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h1>

		<section class="flex flex-col gap-2.5">
			<label>
				<span class="label__title">{{ formatMessage(messages.avatarLabel) }}</span>
			</label>
			<div class="flex items-center gap-4">
				<Avatar
					:src="previewImage ? previewImage : avatarUrl"
					size="md"
					circle
					:alt="username"
				/>
				<div class="flex flex-col gap-2">
					<ButtonStyled>
						<FileInput
							:max-size="262144"
							:show-icon="true"
							class="button-like"
							:prompt="formatMessage(commonMessages.uploadImageButton)"
							accept="image/png,image/jpeg,image/gif,image/webp"
							@change="showPreviewImage"
						>
							<UploadIcon />
						</FileInput>
					</ButtonStyled>
					<ButtonStyled v-if="avatarUrl !== null || previewImage">
						<button @click="removePreviewImage">
							<TrashIcon />
							{{ formatMessage(commonMessages.removeImageButton) }}
						</button>
					</ButtonStyled>
				</div>
			</div>
		</section>

		<section class="flex flex-col gap-2.5">
			<label for="display-name-field">
				<span class="label__title">{{ formatMessage(messages.displayNameLabel) }}</span>
			</label>
			<StyledInput id="display-name-field" v-model="displayName" wrapper-class="w-full" />
		</section>

		<section class="flex flex-col gap-2.5">
			<label for="username-field">
				<span class="label__title">{{ formatMessage(commonMessages.usernameLabel) }}</span>
			</label>
			<StyledInput id="username-field" v-model="username" wrapper-class="w-full" />
		</section>

		<ButtonStyled color="brand">
			<button
				class="!w-full"
				:disabled="!displayName.trim() || !username.trim() || saving"
				@click="save"
			>
				<SaveIcon />
				{{ formatMessage(messages.continueButton) }}
			</button>
		</ButtonStyled>
	</div>
</template>

<script setup lang="ts">
import type { AmberiteProfilePatch } from '@amberite/amberite-api'
import { SaveIcon, TrashIcon, UploadIcon } from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	commonMessages,
	defineMessages,
	FileInput,
	injectNotificationManager,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import type { LocationQueryValue } from 'vue-router'

import { useAmberiteAuthClient } from '@/composables/amberite-client.ts'

definePageMeta({
	middleware: 'auth',
})

const getQueryString = (
	value: LocationQueryValue | LocationQueryValue[] | null | undefined,
): string => {
	const firstValue = Array.isArray(value) ? value[0] : value
	return typeof firstValue === 'string' ? firstValue : ''
}

const normalizeRedirect = (value: string) => {
	return value.startsWith('/') && !value.startsWith('//') ? value : '/dashboard'
}

const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()
const amberiteAuthClient = useAmberiteAuthClient()
const auth = await useAuth()
const route = useNativeRoute()
const redirectTarget = normalizeRedirect(getQueryString(route.query.redirect))

const displayName = ref(auth.value.user?.name ?? auth.value.user?.username ?? '')
const username = ref(auth.value.user?.username ?? '')
const avatarUrl = ref(auth.value.user?.avatar_url ?? null)
const icon = shallowRef<File | null>(null)
const previewImage = shallowRef<string | null>(null)
const pendingAvatarDeletion = ref(false)
const saving = ref(false)

const messages = defineMessages({
	headTitle: {
		id: 'auth.almost-there.head-title',
		defaultMessage: 'Almost there',
	},
	title: {
		id: 'auth.almost-there.title',
		defaultMessage: 'Almost there',
	},
	avatarLabel: {
		id: 'auth.almost-there.avatar.label',
		defaultMessage: 'Avatar',
	},
	displayNameLabel: {
		id: 'auth.almost-there.display-name.label',
		defaultMessage: 'Display name',
	},
	continueButton: {
		id: 'auth.almost-there.continue',
		defaultMessage: 'Continue',
	},
})

useHead({
	title: () => `${formatMessage(messages.headTitle)} - Modrinth`,
})

function showPreviewImage(files: File[]) {
	if (!files[0]) return
	const reader = new FileReader()
	icon.value = files[0]
	reader.readAsDataURL(icon.value)
	reader.onload = (event) => {
		previewImage.value = typeof event.target?.result === 'string' ? event.target.result : null
		pendingAvatarDeletion.value = false
	}
}

function removePreviewImage() {
	icon.value = null
	previewImage.value = 'https://cdn.modrinth.com/placeholder.png'
	pendingAvatarDeletion.value = true
}

async function save() {
	if (!displayName.value.trim() || !username.value.trim()) return

	saving.value = true
	startLoading()
	try {
		const patch: AmberiteProfilePatch = {
			displayName: displayName.value.trim(),
			username: username.value.trim(),
		}

		if (pendingAvatarDeletion.value) {
			patch.avatar = null
		} else if (icon.value && previewImage.value) {
			patch.avatar = {
				url: previewImage.value,
				mimeType: icon.value.type,
				sizeBytes: icon.value.size,
			}
		}

		await amberiteAuthClient.updateCurrentProfile(patch)
		await useAuth(auth.value.token)
		await navigateTo(redirectTarget)
	} catch (err) {
		addNotification({
			title: formatMessage(commonMessages.errorNotificationTitle),
			text: err instanceof Error ? err.message : String(err),
			type: 'error',
		})
	} finally {
		saving.value = false
		stopLoading()
	}
}
</script>
