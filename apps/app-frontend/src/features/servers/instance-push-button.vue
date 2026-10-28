<template>
	<Button
		v-if="servers.length > 0 || publishesToFriends"
		:type="primary ? 'colored' : 'outlined'"
		:color="primary ? 'blue' : undefined"
		size="lg"
		:class="primary ? 'shrink-0' : 'shrink-0 !border'"
		:disabled="isDisabled"
		@click="review($event)"
	>
		<SpinnerIcon v-if="busy" class="animate-spin" aria-hidden="true" />
		<UploadIcon v-else aria-hidden="true" />
		{{
			formatMessage(
				pushing ? messages.pushing : reviewing ? messages.reviewing : messages.pushUpdate,
			)
		}}
	</Button>

	<SharedInstancePublishModal
		ref="publishModal"
		:instance="instance"
		:servers="servers"
		:ask-restart="askRestart"
		:push-servers="pushToServers"
		@published="emit('published')"
		@state-change="publishState = $event"
	/>
	<NewModal
		ref="playersModal"
		:header="formatMessage(messages.playersHeader)"
		:on-hide="() => settleRestart(null)"
	>
		<div class="flex max-w-[30rem] flex-col gap-6">
			<p class="m-0">{{ formatMessage(messages.playersBody, { count: playersOnlineCount }) }}</p>
			<div class="flex flex-wrap justify-end gap-2">
				<Button type="outlined" class="!border" @click="answerPlayers(null)">
					<XIcon aria-hidden="true" />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="outlined" class="!border" @click="answerPlayers(false)">
					{{ formatMessage(messages.applyOnNextStop) }}
				</Button>
				<Button type="colored" color="brand" @click="answerPlayers(true)">
					<UpdatedIcon aria-hidden="true" />
					{{ formatMessage(messages.restartNow) }}
				</Button>
			</div>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import { SpinnerIcon, UpdatedIcon, UploadIcon, XIcon } from '@modrinth/assets'
import {
	Button,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { findRunningSharedInstancePublish } from '@/components/ui/download-manager/shared-instance-publishes'
import SharedInstancePublishModal from '@/components/ui/shared-instances/SharedInstancePublishModal.vue'
import {
	coreProblem,
	playersOnline,
	pushInstanceToServer,
	serversQueryKey,
	useLinkedServers,
} from '@/features/servers/linked-servers'
import type { GameInstance } from '@/helpers/types'

/**
 * The one "Push update" button of an instance. It always opens the review, which publishes one
 * version of the instance's share for friends and servers alike. With linked servers the push
 * is "Push and restart", which asks first when players are online. Renders nothing for an
 * instance that has neither friends nor servers to push to.
 */
const props = defineProps<{
	instance: GameInstance
	disabled?: boolean
	/** Filled style, for the banner that prompts the push. */
	primary?: boolean
}>()

const emit = defineEmits<{
	published: []
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const queryClient = useQueryClient()
const { servers } = useLinkedServers(() => props.instance.id)

const publishModal = ref<InstanceType<typeof SharedInstancePublishModal>>()
const playersModal = ref<InstanceType<typeof NewModal>>()
const publishState = ref<'idle' | 'reviewing' | 'publishing'>('idle')
const uploading = ref(false)
const playersOnlineCount = ref(0)

const publishesToFriends = computed(() => props.instance.shared_instance?.role === 'owner')
// A push started from another button of this instance shows here too, as upstream's does.
const pushing = computed(
	() =>
		uploading.value ||
		publishState.value === 'publishing' ||
		!!findRunningSharedInstancePublish(props.instance.id),
)
const reviewing = computed(() => publishState.value === 'reviewing')
const busy = computed(() => pushing.value || reviewing.value)
const isDisabled = computed(
	() => props.disabled || busy.value || props.instance.install_stage !== 'installed',
)

function review(event?: MouseEvent) {
	if (isDisabled.value) return
	void publishModal.value?.show(event)
}

let restartAnswer: ((restart: boolean | null) => void) | undefined

/**
 * Asked when the review is confirmed, before anything is pushed: whether to restart servers
 * with players on them now (true), apply on their next stop (false), or cancel (null).
 */
function askRestart(): Promise<boolean | null> {
	playersOnlineCount.value = servers.value.reduce((sum, server) => sum + playersOnline(server), 0)
	if (playersOnlineCount.value === 0) return Promise.resolve(true)
	playersModal.value?.show()
	return new Promise((resolve) => (restartAnswer = resolve))
}

function settleRestart(restart: boolean | null) {
	restartAnswer?.(restart)
	restartAnswer = undefined
}

function answerPlayers(restart: boolean | null) {
	settleRestart(restart)
	playersModal.value?.hide()
}

/**
 * Has every linked server apply the share's latest version. The review calls it after its
 * publish, by which time this button may be gone (the banner holding it closes once the
 * instance is up to date), so it relies on nothing that unmounting stops.
 */
async function pushToServers(restart: boolean) {
	const targets = servers.value
	if (targets.length === 0 || uploading.value) return
	uploading.value = true
	try {
		const failed: string[] = []
		let problem = ''
		for (const server of targets) {
			try {
				await pushInstanceToServer(
					{ id: props.instance.id, name: props.instance.name },
					server.server_id,
					{ restart },
				)
			} catch (error) {
				console.error(`Failed to push to server ${server.server_id}`, error)
				failed.push(server.name)
				problem ||= coreProblem(error)
			}
		}
		if (failed.length > 0) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.pushFailedTitle),
				text: `${formatMessage(messages.pushFailedText, { servers: failed.join(', ') })} ${problem}`,
			})
		} else {
			addNotification({
				type: 'success',
				title: formatMessage(messages.pushedTitle),
				text: formatMessage(restart ? messages.pushedRestartText : messages.pushedText),
			})
		}
	} catch (error) {
		addNotification({
			type: 'error',
			title: formatMessage(messages.pushFailedTitle),
			text: error instanceof Error ? error.message : String(error),
		})
	} finally {
		uploading.value = false
		await queryClient.invalidateQueries({ queryKey: serversQueryKey })
	}
}

const messages = defineMessages({
	pushUpdate: {
		id: 'app.instance.admonitions.shared-instance.publish-button',
		defaultMessage: 'Push update',
	},
	pushing: {
		id: 'app.instance.admonitions.shared-instance.publishing-button',
		defaultMessage: 'Pushing...',
	},
	reviewing: {
		id: 'app.instance.admonitions.shared-instance.reviewing-button',
		defaultMessage: 'Reviewing...',
	},
	playersHeader: {
		id: 'instance.linked-servers.players-online.header',
		defaultMessage: 'Players are online',
	},
	playersBody: {
		id: 'instance.linked-servers.players-online.body',
		defaultMessage:
			'{count, plural, one {# player is} other {# players are}} on the server. Restarting now disconnects them.',
	},
	restartNow: {
		id: 'instance.linked-servers.players-online.restart-now',
		defaultMessage: 'Restart now',
	},
	applyOnNextStop: {
		id: 'instance.linked-servers.players-online.apply-on-next-stop',
		defaultMessage: 'Apply on next stop',
	},
	pushedTitle: {
		id: 'instance.linked-servers.pushed.title',
		defaultMessage: 'Update pushed',
	},
	pushedText: {
		id: 'instance.linked-servers.pushed.text',
		defaultMessage: 'A running server takes it the next time it stops.',
	},
	pushedRestartText: {
		id: 'instance.linked-servers.pushed.restart-text',
		defaultMessage: 'A running server restarts to apply it.',
	},
	pushFailedTitle: {
		id: 'instance.linked-servers.push-failed.title',
		defaultMessage: 'Server not updated',
	},
	pushFailedText: {
		id: 'instance.linked-servers.push-failed.text',
		defaultMessage: 'Could not update {servers}.',
	},
})
</script>
