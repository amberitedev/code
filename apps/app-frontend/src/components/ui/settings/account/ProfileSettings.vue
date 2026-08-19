<template>
	<AccountProfileSettings
		ref="profileSettings"
		:patch-user="patchUser"
		:change-avatar="changeAvatar"
		:delete-avatar="deleteAvatar"
		:get-authenticated-user="getAuthenticatedUser"
		name-kind="display-name"
		profile-kind="amberite"
		:editable-name="editableName"
		@profile-link-click="handleProfileLinkClick"
	/>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { AccountProfileSettings, injectAuth, injectModrinthClient } from '@modrinth/ui'
import { computed, inject, onBeforeUnmount, onMounted, ref } from 'vue'

import { appSettingsModalContextKey } from '@/providers/app-settings-modal'

const settingsModal = inject(appSettingsModalContextKey, null)
const auth = injectAuth()
const client = injectModrinthClient()
const editableName = computed(() => {
	const user = auth.user.value as Labrinth.Users.v3.User | null
	return user?.display_name ?? user?.name
})
const profileSettings = ref<InstanceType<typeof AccountProfileSettings> | null>(null)

onMounted(() => {
	settingsModal?.registerUnsavedChangesController({
		hasChanges: () => profileSettings.value?.hasChanges ?? false,
		getOriginal: () => profileSettings.value?.originalState ?? {},
		getModified: () => profileSettings.value?.modifiedState ?? {},
		isSaving: () => profileSettings.value?.saving ?? false,
		reset: () => profileSettings.value?.reset(),
		save: () => profileSettings.value?.save(),
	})
})

onBeforeUnmount(() => {
	settingsModal?.registerUnsavedChangesController(null)
})

function handleProfileLinkClick(event: MouseEvent): void {
	if (settingsModal && !settingsModal.close()) {
		event.preventDefault()
	}
}

function patchUser(
	userId: string,
	patch: Partial<Pick<Labrinth.Users.v2.User, 'bio' | 'username'>>,
): Promise<void> {
	return client.labrinth.users_v2.patch(userId, patch)
}

async function changeAvatar(userId: string, file: Blob, extension: string): Promise<void> {
	await client.labrinth.users_v2.changeIcon(userId, file, extension)
}

function deleteAvatar(userId: string): Promise<void> {
	return client.labrinth.users_v2.deleteIcon(userId)
}

function getAuthenticatedUser(): Promise<Labrinth.Users.v3.User> {
	const userId = auth.user.value?.id
	if (!userId) throw new Error('Cannot refresh a signed-out user.')
	return client.labrinth.users_v3.getAuthenticated()
}
</script>
