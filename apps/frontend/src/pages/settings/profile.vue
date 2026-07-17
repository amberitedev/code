<template>
	<div>
		<section class="card">
			<h2 class="text-2xl">Profile information</h2>
			<p class="mb-4 text-secondary">
				Your verified Minecraft handle identifies your Amberite account. Your display name and bio
				are editable.
			</p>

			<label><span class="label__title">Profile picture</span></label>
			<div class="avatar-changer">
				<Avatar :src="previewImage || avatarUrl" size="md" circle :alt="auth.user.username" />
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
							<TrashIcon />{{ formatMessage(commonMessages.removeImageButton) }}
						</button>
					</ButtonStyled>
					<ButtonStyled v-if="previewImage">
						<button @click="resetAvatar">
							<UndoIcon />{{ formatMessage(commonMessages.resetButton) }}
						</button>
					</ButtonStyled>
				</div>
			</div>

			<label for="minecraft-handle-field">
				<span class="label__title">Verified Minecraft handle</span>
				<span class="label__description">Updated only after Minecraft ownership is verified.</span>
			</label>
			<StyledInput
				id="minecraft-handle-field"
				:model-value="auth.user.verifiedMinecraftHandle"
				disabled
			/>

			<label for="display-name-field">
				<span class="label__title">Display name</span>
				<span class="label__description">The non-unique name shown on your Amberite profile.</span>
			</label>
			<StyledInput id="display-name-field" v-model="current.displayName" />

			<label for="bio-field">
				<span class="label__title">Bio</span>
				<span class="label__description">A short description to tell people about you.</span>
			</label>
			<StyledInput id="bio-field" v-model="current.bio" multiline />

			<div class="input-group mt-4">
				<ButtonStyled>
					<NuxtLink :to="`/user/${auth.user.username}`">
						<UserIcon />{{ formatMessage(commonMessages.visitYourProfile) }}
					</NuxtLink>
				</ButtonStyled>
			</div>
		</section>
		<UnsavedChangesPopup
			:original="originalState"
			:modified="modifiedState"
			:saving="saving"
			@reset="reset"
			@save="save"
		/>
	</div>
</template>

<script setup lang="ts">
import type { AmberiteProfilePatch } from '@amberite/amberite-api'
import { TrashIcon, UndoIcon, UploadIcon, UserIcon } from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	commonMessages,
	FileInput,
	injectNotificationManager,
	StyledInput,
	UnsavedChangesPopup,
	useSavable,
	useVIntl,
} from '@modrinth/ui'

import { useAmberiteAuthClient } from '@/composables/amberite-client.ts'
import { retryAuthRestore } from '@/composables/auth.ts'

definePageMeta({ middleware: 'auth' })
useHead({ title: 'Profile settings - Amberite' })

const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()
const authState = await useAuth()
const auth = computed(
	() =>
		authState.value as typeof authState.value & { user: NonNullable<typeof authState.value.user> },
)
const amberiteAuthClient = useAmberiteAuthClient()
const avatarUrl = ref(auth.value.user.avatar_url)
const icon = shallowRef<File | null>(null)
const previewImage = shallowRef<string | null>(null)
const pendingAvatarDeletion = ref(false)
const saving = ref(false)

const {
	saved,
	current,
	reset: resetFields,
} = useSavable(
	() => ({ displayName: auth.value.user.name, bio: auth.value.user.bio ?? '' }),
	async () => {},
)

const originalState = computed(() => ({ ...saved.value, avatarChanged: false }))
const modifiedState = computed(() => ({
	...current.value,
	avatarChanged: Boolean(previewImage.value || pendingAvatarDeletion.value),
}))

function reset() {
	resetFields()
	resetAvatar()
}

function showPreviewImage(files: File[]) {
	if (!files[0]) return
	icon.value = files[0]
	const reader = new FileReader()
	reader.readAsDataURL(files[0])
	reader.onload = (event) => {
		previewImage.value = typeof event.target?.result === 'string' ? event.target.result : null
		pendingAvatarDeletion.value = false
	}
}

function removePreviewImage() {
	pendingAvatarDeletion.value = true
	previewImage.value = 'https://cdn.modrinth.com/placeholder.png'
}

function resetAvatar() {
	icon.value = null
	previewImage.value = null
	pendingAvatarDeletion.value = false
}

async function save() {
	saving.value = true
	try {
		const patch: AmberiteProfilePatch = {}
		if (current.value.displayName !== auth.value.user.name)
			patch.displayName = current.value.displayName.trim()
		if (current.value.bio !== (auth.value.user.bio ?? '')) patch.bio = current.value.bio
		if (pendingAvatarDeletion.value) patch.avatar = null
		else if (icon.value && previewImage.value) {
			patch.avatar = {
				url: previewImage.value,
				mimeType: icon.value.type,
				sizeBytes: icon.value.size,
			}
		}
		if (Object.keys(patch).length) await amberiteAuthClient.updateCurrentProfile(patch)
		await retryAuthRestore()
		avatarUrl.value = auth.value.user.avatar_url
		resetAvatar()
	} catch (error) {
		addNotification({
			title: formatMessage(commonMessages.errorNotificationTitle),
			text: error instanceof Error ? error.message : String(error),
			type: 'error',
		})
	} finally {
		saving.value = false
	}
}
</script>

<style lang="scss" scoped>
.avatar-changer {
	display: flex;
	gap: var(--gap-lg);
	margin-top: var(--gap-md);
}
</style>
