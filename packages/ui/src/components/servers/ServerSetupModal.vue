<template>
	<CreationFlowModal
		ref="creationFlowRef"
		type="reset-server"
		:available-loaders="serverLoaders"
		:show-snapshot-toggle="true"
		:disable-close="props.initialSetup"
		:is-initial-setup="props.initialSetup"
		:initial-loader="initialLoader"
		:initial-game-version="initialGameVersion"
		:fade="props.initialSetup ? undefined : 'danger'"
		:get-project-versions="getProjectVersions"
		:finish-disabled="!canCompleteSetup"
		:finish-disabled-tooltip="!canCompleteSetup ? permissionDeniedMessage : undefined"
		@create="onFlowComplete"
		@hide="$emit('hide')"
		@browse-modpacks="$emit('browse-modpacks')"
	/>
</template>

<script setup lang="ts">
import type { ModrinthApiError } from '@modrinth/api-client'
import { computed, useTemplateRef } from 'vue'

import { useDebugLogger } from '#ui/composables/debug-logger'
import type { ServerPlatform } from '#ui/composables/server-installation-tracker'
import { useServerPermissions } from '#ui/composables/server-permissions'
import { setUpServer } from '#ui/composables/server-setup'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { injectModrinthClient } from '../../providers/api-client'
import { injectModrinthServerContext } from '../../providers/server-context'
import { injectNotificationManager } from '../../providers/web-notifications'
import type { CreationFlowContextValue } from '../flows/creation-flow-modal/creation-flow-context'
import CreationFlowModal from '../flows/creation-flow-modal/index.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	rateLimitTitle: {
		id: 'servers.setup.rate-limit.title',
		defaultMessage: 'Cannot reinstall server',
	},
	rateLimitText: {
		id: 'servers.setup.rate-limit.text',
		defaultMessage: 'You are being rate limited. Please try again later.',
	},
	reinstallFailedTitle: {
		id: 'servers.setup.reinstall-failed.title',
		defaultMessage: 'Reinstall Failed',
	},
	reinstallFailedText: {
		id: 'servers.setup.reinstall-failed.text',
		defaultMessage: 'An unexpected error occurred while reinstalling. Please try again later.',
	},
})

const debug = useDebugLogger('ServerSetupModal')
const client = injectModrinthClient()
const serverContext = injectModrinthServerContext()
const { addNotification } = injectNotificationManager()

const serverLoaders = ['vanilla', 'fabric', 'neoforge', 'forge', 'quilt', 'paper']
const { canSetup, canResetServer, permissionDeniedMessage } = useServerPermissions()

async function getProjectVersions(projectId: string) {
	const versions = await client.labrinth.versions_v3.getProjectVersions(projectId)
	return versions.map((v) => ({ id: v.id }))
}

const props = defineProps<{
	initialSetup?: boolean
}>()

const emit = defineEmits<{
	reinstall: [args?: { loader: string; lVersion: string; mVersion: string | null }]
	hide: []
	'browse-modpacks': []
}>()

const initialLoader = computed(() => {
	const loader = serverContext.server.value?.loader
	if (!loader || loader === 'Vanilla') return undefined
	return loader.toLowerCase()
})

const initialGameVersion = computed(() => serverContext.server.value?.mc_version ?? undefined)
const canCompleteSetup = computed(() =>
	props.initialSetup ? canSetup.value : canResetServer.value,
)

const creationFlowRef = useTemplateRef<InstanceType<typeof CreationFlowModal>>('creationFlowRef')

async function onFlowComplete(ctx: CreationFlowContextValue) {
	if (!canCompleteSetup.value) {
		ctx.loading.value = false
		return
	}

	debug('onFlowComplete:', {
		setupType: ctx.setupType.value,
		hasModpackFile: !!ctx.modpackFile.value,
		modpackSelection: ctx.modpackSelection.value,
		selectedLoader: ctx.selectedLoader.value,
		selectedGameVersion: ctx.selectedGameVersion.value,
		selectedLoaderVersion: ctx.selectedLoaderVersion.value,
	})

	const { coreId, serverId } = serverContext
	try {
		if (ctx.projectInstall.value) {
			await ctx.installServerContent(coreId, serverId)
			emitReinstall()
		} else if (ctx.setupType.value === 'modpack' && ctx.modpackFile.value) {
			debug('onFlowComplete: mrpack upload path')
			const file = ctx.modpackFile.value
			// Before the upload: the Core's install job starts before it answers, and may end before.
			serverContext.beginInstallation({ type: 'local_modpack', filename: file.name })
			await ctx.trackUpload((onUploadProgress) =>
				setUpServer(
					client,
					coreId,
					serverId,
					{ type: 'pack-file', file },
					{ properties: ctx.buildProperties(), onUploadProgress },
				),
			)
			emitReinstall()
		} else if (ctx.setupType.value === 'modpack' && ctx.modpackSelection.value) {
			const { projectId, versionId } = ctx.modpackSelection.value
			serverContext.beginInstallation({
				type: 'modrinth_modpack',
				project_id: projectId,
				version_id: versionId,
			})
			await setUpServer(
				client,
				coreId,
				serverId,
				{ type: 'modpack', projectId, versionId },
				{ properties: ctx.buildProperties() },
			)
			emitReinstall()
		} else {
			const loader = ctx.selectedLoader.value ?? 'vanilla'
			const loaderVersion = loader === 'vanilla' ? '' : (ctx.selectedLoaderVersion.value ?? '')
			const gameVersion = ctx.selectedGameVersion.value ?? ''

			serverContext.beginInstallation({
				type: 'platform',
				platform: loader as ServerPlatform,
				platform_version: loaderVersion,
				game_version: gameVersion,
			})
			await setUpServer(
				client,
				coreId,
				serverId,
				{ type: 'loader', loader, loaderVersion, gameVersion },
				{ properties: ctx.buildProperties() },
			)
			emitReinstall({ loader, lVersion: loaderVersion, mVersion: ctx.selectedGameVersion.value })
		}

		creationFlowRef.value?.hide()
	} catch (error) {
		debug('onFlowComplete: ERROR', error)
		serverContext.cancelOptimisticInstallation()
		if ((error as ModrinthApiError).statusCode === 429) {
			addNotification({
				title: formatMessage(messages.rateLimitTitle),
				text: formatMessage(messages.rateLimitText),
				type: 'error',
			})
		} else {
			addNotification({
				title: formatMessage(messages.reinstallFailedTitle),
				text: formatMessage(messages.reinstallFailedText),
				type: 'error',
			})
		}
	} finally {
		ctx.loading.value = false
	}
}

function emitReinstall(args?: { loader: string; lVersion: string; mVersion: string | null }) {
	debug('emitReinstall:', args)
	emit('reinstall', args)
}

function show() {
	if (!canCompleteSetup.value) return

	void creationFlowRef.value?.ctx?.fetchLoaderMetadata('paper')
	creationFlowRef.value?.show()
}

function hide() {
	creationFlowRef.value?.hide()
}

defineExpose({ show, hide, ctx: computed(() => creationFlowRef.value?.ctx) })
</script>
