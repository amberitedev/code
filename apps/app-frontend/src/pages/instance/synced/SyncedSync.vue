<template>
	<div class="flex h-full flex-col gap-4 overflow-y-auto">
		<div class="flex flex-col gap-3 rounded-2xl bg-bg-raised p-4">
			<div class="flex items-center gap-2">
				<UploadIcon class="size-5 text-secondary" />
				<span class="text-lg font-bold text-contrast">Publish snapshot</span>
			</div>
			<p class="m-0 text-sm text-secondary">
				Capture the current mods of both sides as a versioned snapshot. Snapshots build a history
				the group can sync from, like commits.
			</p>
			<template v-if="canPublish">
				<textarea
					v-model="notes"
					class="min-h-16 w-full resize-y rounded-xl bg-bg p-3 text-sm text-contrast"
					placeholder="Describe what changed (optional)"
				/>
				<div class="flex items-center gap-2">
					<ButtonStyled color="brand">
						<button :disabled="publishing" @click="onPublish">
							<UploadIcon />
							{{ publishing ? 'Publishing…' : 'Publish snapshot' }}
						</button>
					</ButtonStyled>
					<ButtonStyled type="transparent">
						<button :disabled="loading" @click="refresh"><RefreshCwIcon /> Refresh</button>
					</ButtonStyled>
				</div>
			</template>
			<NoPermissionCard
				v-else-if="!isLinked"
				label="This synced profile isn't linked to a friend group, so there's nothing to sync to yet."
			/>
			<NoPermissionCard
				v-else
				label="You don't have permission to publish snapshots for this profile."
			/>
		</div>

		<div class="flex flex-col gap-2">
			<div class="flex items-center gap-2 px-1">
				<HistoryIcon class="size-4 text-secondary" />
				<span class="text-sm font-bold uppercase text-secondary">History</span>
			</div>
			<div
				v-if="snapshots.length === 0"
				class="rounded-2xl bg-bg-raised p-4 text-sm text-secondary"
			>
				No snapshots yet.
			</div>
			<div
				v-for="snapshot in snapshots"
				:key="snapshot._id"
				class="flex items-start gap-3 rounded-2xl bg-bg-raised p-4"
			>
				<UserIcon class="mt-0.5 size-5 shrink-0 text-secondary" />
				<div class="flex min-w-0 flex-1 flex-col gap-1">
					<div class="flex flex-wrap items-center gap-2">
						<span class="font-semibold text-contrast">{{ authorName(snapshot.authorUserId) }}</span>
						<span class="text-xs text-secondary">{{ fromNow(snapshot.createdAt) }}</span>
					</div>
					<span v-if="snapshot.notes" class="text-sm text-primary">{{ snapshot.notes }}</span>
					<span class="text-xs text-secondary">{{ manifestSummary(snapshot) }}</span>
				</div>
			</div>
		</div>

		<div v-if="events.length > 0" class="flex flex-col gap-2">
			<div class="flex items-center gap-2 px-1">
				<ClockIcon class="size-4 text-secondary" />
				<span class="text-sm font-bold uppercase text-secondary">Sync events</span>
			</div>
			<div
				v-for="event in events"
				:key="event._id"
				class="flex items-center gap-3 rounded-2xl bg-bg-raised px-4 py-3"
			>
				<span
					class="rounded-full bg-bg px-2 py-0.5 text-xs font-semibold capitalize text-secondary"
				>
					{{ event.status }}
				</span>
				<span class="min-w-0 flex-1 truncate text-sm text-primary">{{ event.message }}</span>
				<span class="text-xs text-secondary">{{ fromNow(event.createdAt) }}</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { ConvexProfileSnapshot } from '@amberite/amberite-api'
import { ClockIcon, HistoryIcon, RefreshCwIcon, UploadIcon, UserIcon } from '@modrinth/assets'
import { ButtonStyled, injectNotificationManager } from '@modrinth/ui'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed, onMounted, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'
import type { GameInstance } from '@/helpers/types'

import NoPermissionCard from './NoPermissionCard.vue'
import type { SyncedManifest } from './synced-manifest'
import { getLinkedServerId } from './use-synced-link'
import { useSyncedPermissions } from './use-synced-permissions'
import { useSyncedSnapshots } from './use-synced-snapshots'

dayjs.extend(relativeTime)

const props = defineProps<{
	instance: GameInstance
	offline?: boolean
	playing?: boolean
}>()

const { handleError } = injectNotificationManager()
const { members } = useSocial()
const { has } = useSyncedPermissions()

const serverInstanceId = getLinkedServerId(props.instance.path)
const { snapshots, events, loading, publishing, error, isLinked, refresh, publish } =
	useSyncedSnapshots(props.instance, serverInstanceId, props.instance.path)

const notes = ref('')
const canPublish = computed(
	() => isLinked.value && (has('server:content') || has('client:content')),
)

function authorName(userId: string): string {
	return members.value.find((m) => m.userId === userId)?.user?.username ?? 'Unknown'
}

function fromNow(timestamp: number): string {
	return dayjs(timestamp).fromNow()
}

function manifestSummary(snapshot: ConvexProfileSnapshot): string {
	const manifest = snapshot.manifest as SyncedManifest | null
	const client = manifest?.client?.length ?? 0
	const server = manifest?.server?.length ?? 0
	return `${client} client · ${server} server mods`
}

async function onPublish() {
	try {
		await publish(notes.value)
		notes.value = ''
	} catch {
		if (error.value) handleError(error.value)
	}
}

onMounted(refresh)
</script>
