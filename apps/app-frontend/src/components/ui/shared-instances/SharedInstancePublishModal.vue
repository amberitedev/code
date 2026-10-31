<template>
	<SharedFilesPublishModal
		ref="modal"
		:diffs="publishDiffs"
		:files="files"
		:requires-restart="hasServers"
		:allow-empty="serversBehind"
		:limit="SHARE_LIMIT_BYTES"
		@confirm="confirmPublish"
		@cancel="finishReview"
	/>
</template>

<script setup lang="ts">
import { type ContentDiffItem, injectModrinthClient, SharedFilesPublishModal } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { trackSharedInstancePublish } from '@/components/ui/download-manager/shared-instance-publishes'
import { isServerBehind, type LinkedServer } from '@/features/servers/linked-servers'
import { getErrorMessage, isSharedInstanceUnavailableError } from '@/helpers/install'
import {
	ensure_shared_instance,
	get_shared_instance_publish_preview,
	get_shared_instance_publish_support_details,
	publish_shared_instance,
	type SharedInstancePublishFile,
} from '@/helpers/instance'
import { useSharedInstanceErrors } from '@/helpers/shared-instance-errors'
import type { GameInstance } from '@/helpers/types'
import { injectSharedInstance } from '@/pages/instance/shared-instance-context'

type SharedInstancePublishState = 'idle' | 'reviewing' | 'publishing'

/**
 * The review every push goes through. It lists what changed and only lets a push happen when
 * something did, or when a server is behind. The push publishes one version of the instance's
 * share, which friends and servers both read. With linked servers it is "Push and restart" and
 * calls `pushServers` to have them apply it. An instance with servers gets a share here if it
 * had none.
 */
const props = defineProps<{
	instance: GameInstance
	servers?: LinkedServer[]
	/** Whether to restart servers now (true), on their next stop (false), or cancel (null). */
	askRestart?: () => Promise<boolean | null>
	/**
	 * Has the servers apply the share's latest version. A function, not an event: the push runs
	 * on after the review closes, and an event from an unmounted component is dropped.
	 */
	pushServers?: (restart: boolean) => Promise<void>
}>()

const emit = defineEmits<{
	published: []
	'state-change': [state: SharedInstancePublishState]
}>()

const client = injectModrinthClient()
const { notifySharedInstanceError, notifySharedInstanceAvailability } = useSharedInstanceErrors()
const sharedInstanceState = injectSharedInstance()
// The pool stores at most 1 GiB per shared version (MAX_SHARE_BYTES in apps/backend).
const SHARE_LIMIT_BYTES = 1024 ** 3

const modal = ref<InstanceType<typeof SharedFilesPublishModal>>()
const publishDiffs = ref<ContentDiffItem[]>([])
const files = ref<SharedInstancePublishFile[]>([])
const state = ref<SharedInstancePublishState>('idle')
const hasServers = computed(() => (props.servers?.length ?? 0) > 0)
// Whether this push publishes a version: only the share's owner does. A member who hosts a
// server only has it apply the latest version.
const publishes = ref(false)
// Whether the changes on screen are not in the share's latest version yet.
const unpublished = ref(false)
// Whether a server runs something other than the share's latest version.
const serversBehind = ref(false)

async function show(e?: MouseEvent) {
	if (state.value !== 'idle') return

	setState('reviewing')
	let reviewOpened = false
	try {
		let diffs: ContentDiffItem[] = []
		files.value = []
		const attachment =
			props.instance.shared_instance ??
			(hasServers.value ? await ensure_shared_instance(props.instance.id) : undefined)
		publishes.value = attachment?.role === 'owner'
		// A member's server can always be told to apply the latest version.
		serversBehind.value = hasServers.value && !publishes.value
		if (publishes.value) {
			const preview = await get_shared_instance_publish_preview(props.instance.id)
			if (!preview) {
				// The local record is already detached, so there is nothing left to push to.
				sharedInstanceState.setUnavailable('deleted')
				notifySharedInstanceAvailability(sharedInstanceState.availability.value ?? 'deleted')
				emit('published')
				return
			}
			diffs = preview.diffs.map((diff) => ({
				type: diff.type,
				projectName: diff.projectName ?? undefined,
				fileName: diff.fileName ?? undefined,
				currentVersionName: diff.currentVersionName ?? undefined,
				newVersionName: diff.newVersionName ?? undefined,
				fileCount: diff.configFileCount ?? undefined,
				disabled: diff.disabled,
			}))
			files.value = preview.files
			serversBehind.value = (props.servers ?? []).some((server) =>
				isServerBehind(server, preview.sharedInstanceId, preview.latestVersion),
			)
		}
		unpublished.value = diffs.length > 0
		publishDiffs.value = diffs
		if (!modal.value) return

		modal.value.show({ event: e })
		reviewOpened = true
	} catch (error) {
		handlePublishError(error)
	} finally {
		if (!reviewOpened) finishReview()
	}
}

/**
 * Runs the push in the background, where the task list shows it. "Push and restart" asks first
 * when players are online; cancelling that question returns to the review.
 */
async function confirmPublish(paths: string[], restart: boolean) {
	let restartNow = restart
	if (restart && props.askRestart) {
		const answer = await props.askRestart()
		if (answer === null) {
			modal.value?.show({ keepSelection: true })
			return
		}
		restartNow = answer
	}
	void publishChanges(paths, restartNow)
}

async function publishChanges(paths: string[], restart: boolean) {
	if (state.value !== 'reviewing') return

	setState('publishing')
	try {
		if (publishes.value && (unpublished.value || paths.length > 0)) {
			if (!(await publishToFriends(paths))) return
			emit('published')
		}
		if (hasServers.value) await props.pushServers?.(restart)
	} finally {
		setState('idle')
	}
}

/** Publishes to friends and reports whether it succeeded. */
async function publishToFriends(paths: string[]) {
	const task = trackSharedInstancePublish(props.instance)
	try {
		await publish_shared_instance(props.instance.id, paths, await ownServerCore(), task.update)
		await loadSupportDetails(task)
		task.succeed()
		return true
	} catch (error) {
		await loadSupportDetails(task)
		task.fail(getErrorMessage(error))
		handlePublishError(error)
		return false
	}
}

/**
 * The first of the account's own Cores that runs one of this instance's servers. The share's home
 * copy is kept there. A server on someone else's Core, or one whose Core can't be found, is skipped.
 */
async function ownServerCore() {
	for (const server of props.servers ?? []) {
		try {
			const core = await client.archon.cores_v1.coreOf(server.server_id)
			if (core.owned) return core.id
		} catch (error) {
			console.error(`Could not find the Core of server ${server.server_id}`, error)
		}
	}
	return null
}

async function loadSupportDetails(task: ReturnType<typeof trackSharedInstancePublish>) {
	try {
		const details = await get_shared_instance_publish_support_details(props.instance.id)
		if (details) task.setDetails(details)
	} catch {
		return
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

defineExpose({ show })
</script>
