<template>
	<section v-if="auth.user" class="universal-card">
		<AccountSocialSettings
			:get-blocked-users="getBlockedUsers"
			:get-users="getUsers"
			:unblock-user="unblockUser"
			:get-preferences="getPreferences"
			:edit-preferences="editPreferences"
		/>
	</section>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
	AccountSocialSettings,
	commonSettingsMessages,
	injectModrinthClient,
	useVIntl,
} from '@modrinth/ui'

definePageMeta({
	middleware: 'auth',
})

const auth = await useAuth()
const client = injectModrinthClient()
const { formatMessage } = useVIntl()

function getBlockedUsers(): Promise<Labrinth.BlockedUsers.v3.BlockedUserId[]> {
	return client.labrinth.blocked_users_v3.list()
}

function getUsers(userIds: string[]): Promise<Labrinth.Users.v2.User[]> {
	return client.labrinth.users_v2.getMultiple(userIds)
}

function unblockUser(userId: string): Promise<void> {
	return client.labrinth.blocked_users_v3.unblock(userId)
}

function getPreferences(userId: string): Promise<Labrinth.Users.v3.UserPreferences> {
	return client.labrinth.users_v3.getPreferences(userId)
}

function editPreferences(
	userId: string,
	preferences: Labrinth.Users.v3.PartialUserPreferences,
): Promise<Labrinth.Users.v3.UserPreferences> {
	return client.labrinth.users_v3.editPreferences(userId, preferences)
}

useHead({
	title: () => `${formatMessage(commonSettingsMessages.social)} - Modrinth`,
})
</script>
