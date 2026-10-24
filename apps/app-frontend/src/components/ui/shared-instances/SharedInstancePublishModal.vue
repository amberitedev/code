<template>
	<ContentDiffModal
		ref="publishReviewModal"
		:header="formatMessage(messages.header)"
		:admonition-header="formatMessage(messages.admonitionHeader)"
		:description="formatMessage(messages.description)"
		:diffs="publishDiffs"
		:confirm-label="
			formatMessage(hasServers ? messages.pushAndRestartButton : messages.publishButton)
		"
		:confirm-icon="UploadIcon"
		:secondary-confirm-label="hasServers ? formatMessage(messages.publishButton) : undefined"
		:confirm-disabled="
			publishDiffs.length === 0 &&
			selectedConfigPaths.length === 0 &&
			!serverFailed &&
			!serverUnchecked
		"
		:added-label="formatMessage(messages.addedLabel)"
		:removed-label="formatMessage(messages.removedLabel)"
		@confirm="publishChanges(hasServers)"
		@secondary-confirm="publishChanges(false)"
		@cancel="finishReview"
	>
		<template v-if="configFilePaths.length > 0" #additional-content>
			<Accordion
				class="min-w-0"
				overflow-visible
				button-class="w-full bg-transparent m-0 p-0 border-none"
			>
				<template #title>
					<FileCogIcon class="size-4 shrink-0 text-primary" />
					<span class="text-lg font-semibold text-contrast">
						{{ formatMessage(messages.configTitle) }}
					</span>
				</template>
				<div class="flex min-w-0 flex-col gap-3 pt-4">
					<div ref="configFileTreeContainer" class="max-h-[292px] overflow-y-auto rounded-[20px]">
						<FileTreeSelect
							v-model="includedConfigPaths"
							v-model:excluded-paths="excludedConfigPaths"
							:items="configFileItems"
							:show-size="false"
							:show-modified="false"
							@navigate="scrollConfigFileTreeToTop"
						/>
					</div>
				</div>
			</Accordion>
		</template>
	</ContentDiffModal>
</template>

<script setup lang="ts">
import { FileCogIcon, UploadIcon } from '@modrinth/assets'
import {
	Accordion,
	type ContentDiffItem,
	ContentDiffModal,
	defineMessages,
	FileTreeSelect,
	type FileTreeSelectItem,
	injectNotificationManager,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { isSharedInstanceUnavailableError } from '@/helpers/install'
import { get_shared_instance_publish_preview, publish_shared_instance } from '@/helpers/instance'
import { diffInstanceAgainstServer, type LinkedServer } from '@/helpers/linked-servers'
import {
	sharedInstanceErrorMessages,
	useSharedInstanceErrors,
} from '@/helpers/shared-instance-errors'
import type { GameInstance, SharedInstanceAttachment } from '@/helpers/types'
import { injectSharedInstance } from '@/pages/instance/shared-instance-context'
import { apiClient } from '@/services/api-client'

type SharedInstancePublishState = 'idle' | 'reviewing' | 'publishing'

/**
 * The review every push goes through. It lists what changed and only lets a push happen when
 * something did. A shared instance publishes to friends; an instance with a server also
 * offers "Push and restart" and emits `push-servers` for the caller to update it.
 */
const props = defineProps<{
	instance: GameInstance
	server?: LinkedServer
}>()

const emit = defineEmits<{
	published: []
	'push-servers': [restart: boolean]
	'state-change': [state: SharedInstancePublishState]
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const { notifySharedInstanceError, notifySharedInstanceAvailability } = useSharedInstanceErrors()
const sharedInstanceState = injectSharedInstance()
const publishReviewModal = ref<InstanceType<typeof ContentDiffModal>>()
const configFileTreeContainer = ref<HTMLElement>()
const publishDiffs = ref<ContentDiffItem[]>([])
const configFilePaths = ref<string[]>([])
const includedConfigPaths = ref<string[]>([])
const excludedConfigPaths = ref<string[]>([])
const state = ref<SharedInstancePublishState>('idle')
const hasServers = computed(() => !!props.server)
// A server that failed to take the instance can always be pushed to again.
const serverFailed = computed(() => props.server?.linked_instance.state === 'failed')
// A server that could not be compared may be behind, so it can be pushed to as well.
const serverUnchecked = ref(false)
const sharesWithFriends = computed(() => props.instance.shared_instance?.role === 'owner')
// Whether the changes on screen are ones friends have not received yet.
const friendsBehind = ref(false)
const configFileItems = computed<FileTreeSelectItem[]>(() =>
	configFilePaths.value.map((path) => ({ path, type: 'file' })),
)
const selectedConfigPaths = computed(() => {
	const includedPaths = new Set(includedConfigPaths.value)
	const excludedPaths = new Set(excludedConfigPaths.value)

	return configFilePaths.value.filter((path) =>
		isConfigPathSelected(path, includedPaths, excludedPaths),
	)
})

async function show(e?: MouseEvent) {
	if (state.value !== 'idle') return

	setState('reviewing')
	let reviewOpened = false
	try {
		let friendDiffs: ContentDiffItem[] = []
		configFilePaths.value = []
		if (sharesWithFriends.value) {
			const preview = await get_shared_instance_publish_preview(props.instance.id)
			if (!preview) {
				// The local record is already detached, so there is nothing left to push to.
				sharedInstanceState.setUnavailable('deleted')
				notifySharedInstanceAvailability(sharedInstanceState.availability.value ?? 'deleted')
				emit('published')
				if (!hasServers.value) return
			} else {
				friendDiffs = preview.diffs.map((diff) => ({
					type: diff.type,
					projectName: diff.projectName ?? undefined,
					fileName: diff.fileName ?? undefined,
					currentVersionName: diff.currentVersionName ?? undefined,
					newVersionName: diff.newVersionName ?? undefined,
					fileCount: diff.configFileCount ?? undefined,
					disabled: diff.disabled,
				}))
				configFilePaths.value = preview.configFiles
			}
		}
		friendsBehind.value = friendDiffs.length > 0
		// Friends can be up to date while a server is not, so fall back to what the server lacks.
		let serverDiffs: ContentDiffItem[] = []
		if (friendDiffs.length === 0) {
			serverUnchecked.value = false
			if (props.server) {
				try {
					serverDiffs = await diffInstanceAgainstServer(props.instance.id, props.server)
				} catch (error) {
					console.error(`Failed to compare with server ${props.server.server_id}`, error)
					serverUnchecked.value = true
				}
			}
		}
		publishDiffs.value = friendDiffs.length > 0 ? friendDiffs : serverDiffs
		includedConfigPaths.value = []
		excludedConfigPaths.value = []
		if (!publishReviewModal.value) return

		publishReviewModal.value.show(e)
		reviewOpened = true
	} catch (error) {
		handlePublishError(error)
	} finally {
		if (!reviewOpened) finishReview()
	}
}

async function publishChanges(restart: boolean) {
	if (state.value !== 'reviewing') return

	setState('publishing')
	try {
		if (sharesWithFriends.value && (friendsBehind.value || selectedConfigPaths.value.length > 0)) {
			const attachment = await publish_shared_instance(props.instance.id, selectedConfigPaths.value)
			if (!(await pushReachedFriends(attachment))) {
				addNotification({
					type: 'error',
					title: formatMessage(sharedInstanceErrorMessages.pushUnfinishedTitle),
					text: formatMessage(sharedInstanceErrorMessages.pushUnfinishedText),
				})
				return
			}
			emit('published')
		}
		if (hasServers.value) emit('push-servers', restart)
	} catch (error) {
		handlePublishError(error)
	} finally {
		setState('idle')
	}
}

/**
 * Checks a finished push against the backend. The version it made must be complete with every
 * file stored, and the instance must no longer differ from what friends get, which compares the
 * file list and each uploaded file's hash. Anything else means the push stopped partway.
 */
async function pushReachedFriends(attachment: SharedInstanceAttachment) {
	const version = attachment.latest_version
	if (version == null) {
		console.error('Push finished without a version', attachment)
		return false
	}

	const remote = await apiClient.sharedinstances.instances_v1.getVersion(attachment.id, version)
	const notStored = remote.external_files.filter((file) => !file.sha256)
	const preview = await get_shared_instance_publish_preview(props.instance.id)
	const complete =
		remote.ready &&
		notStored.length === 0 &&
		preview?.latestVersion === version &&
		preview.diffs.length === 0
	if (!complete) {
		console.error('Push did not reach the backend intact', {
			sharedInstanceId: attachment.id,
			version,
			ready: remote.ready,
			filesNotStored: notStored.map((file) => file.file_name),
			latestVersion: preview?.latestVersion,
			remainingDiffs: preview?.diffs,
		})
	}
	return complete
}

function isConfigPathSelected(
	path: string,
	includedPaths: Set<string>,
	excludedPaths: Set<string>,
) {
	let selected = false
	let prefix = ''

	for (const segment of path.split('/').filter(Boolean)) {
		prefix = prefix ? `${prefix}/${segment}` : segment
		if (includedPaths.has(prefix)) selected = true
		if (excludedPaths.has(prefix)) selected = false
	}

	return selected
}

function scrollConfigFileTreeToTop() {
	if (configFileTreeContainer.value) {
		configFileTreeContainer.value.scrollTop = 0
	}
}

function finishReview() {
	if (state.value === 'reviewing') {
		setState('idle')
	}
}

function handlePublishError(error: unknown) {
	void sharedInstanceState.reportActionError(error, notifySharedInstanceError)
	if (isSharedInstanceUnavailableError(error)) emit('published')
}

function setState(nextState: SharedInstancePublishState) {
	state.value = nextState
	emit('state-change', nextState)
}

const messages = defineMessages({
	header: {
		id: 'instance.shared-instance.publish-review.header',
		defaultMessage: 'Review changes',
	},
	admonitionHeader: {
		id: 'instance.shared-instance.publish-review.admonition-header',
		defaultMessage: 'Push update',
	},
	description: {
		id: 'instance.shared-instance.publish-review.description',
		defaultMessage:
			'Review the content changes and choose any config files to include in this update.',
	},
	publishButton: {
		id: 'instance.shared-instance.publish-review.publish-button',
		defaultMessage: 'Push update',
	},
	pushAndRestartButton: {
		id: 'instance.shared-instance.publish-review.push-and-restart-button',
		defaultMessage: 'Push and restart',
	},
	addedLabel: {
		id: 'instance.shared-instance.publish-review.added-label',
		defaultMessage: 'Added',
	},
	removedLabel: {
		id: 'instance.shared-instance.publish-review.removed-label',
		defaultMessage: 'Removed',
	},
	configTitle: {
		id: 'instance.shared-instance.publish-review.config-title-v2',
		defaultMessage: 'Select config files',
	},
})

defineExpose({ show })
</script>
