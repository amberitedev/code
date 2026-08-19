<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import {
	Button,
	defineMessages,
	injectModrinthClient,
	injectNotificationManager,
	useFormatDateTime,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

const client = injectModrinthClient()
const queryClient = useQueryClient()
const notifications = injectNotificationManager()
const { formatMessage } = useVIntl()
const relativeTime = useRelativeTime()
const dateTime = useFormatDateTime({ timeStyle: 'short', dateStyle: 'long' })

const messages = defineMessages({
	description: {
		id: 'settings.sessions.amberite-description',
		defaultMessage:
			'Devices signed in to your Amberite account. Revoke any session you do not recognize.',
	},
	current: { id: 'settings.sessions.current-session', defaultMessage: 'Current session' },
	revoke: { id: 'settings.sessions.action.revoke-session', defaultMessage: 'Revoke session' },
	unknown: { id: 'settings.sessions.unknown-device', defaultMessage: 'Unknown device' },
})

const { data: sessions } = useQuery({
	queryKey: ['amberite', 'sessions'],
	queryFn: () => client.labrinth.sessions_v2.list(),
})

async function revoke(id: string) {
	try {
		await client.labrinth.sessions_v2.delete(id)
		await queryClient.invalidateQueries({ queryKey: ['amberite', 'sessions'] })
	} catch (error) {
		notifications.handleError(error instanceof Error ? error : new Error(String(error)))
	}
}
</script>

<template>
	<div class="flex flex-col gap-4">
		<div>
			<h2 class="m-0 text-lg font-semibold text-contrast">Device sessions</h2>
			<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.description) }}</p>
		</div>
		<div
			v-for="session in sessions ?? []"
			:key="session.id"
			class="flex items-center gap-4 rounded-xl bg-surface-2 p-4"
		>
			<div class="min-w-0 flex-1">
				<div class="font-semibold text-contrast">
					{{ session.os ?? formatMessage(messages.unknown) }}
					<span v-if="session.platform"> · {{ session.platform }}</span>
				</div>
				<div class="text-sm text-secondary" :title="dateTime(session.last_login)">
					Last used {{ relativeTime(session.last_login) }}
					<span v-if="session.city || session.country">
						· {{ [session.city, session.country].filter(Boolean).join(', ') }}
					</span>
				</div>
			</div>
			<span v-if="session.current" class="text-sm font-semibold text-brand">
				{{ formatMessage(messages.current) }}
			</span>
			<Button v-else @click="revoke(session.id)">
				<XIcon /> {{ formatMessage(messages.revoke) }}
			</Button>
		</div>
	</div>
</template>
