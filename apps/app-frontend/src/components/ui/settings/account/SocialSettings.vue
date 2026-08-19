<template>
	<AccountSocialSettings
		:get-blocked-users="get_blocked_users"
		:get-users="getUsers"
		:unblock-user="unblock_user"
		:get-preferences="getPreferences"
		:edit-preferences="editPreferences"
		identity-kind="amberite"
	/>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { AccountSocialSettings, injectModrinthClient } from '@modrinth/ui'

import { get_blocked_users, unblock_user } from '@/helpers/users'

const client = injectModrinthClient()

function getUsers(userIds: string[]): Promise<Labrinth.Users.v2.User[]> {
	return client.labrinth.users_v2.getMultiple(userIds)
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
</script>
