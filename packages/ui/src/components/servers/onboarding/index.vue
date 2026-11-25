<template>
	<CreationFlowModal
		ref="modalRef"
		type="server-onboarding"
		:available-loaders="['vanilla', 'fabric', 'neoforge', 'forge', 'quilt', 'paper']"
		show-snapshot-toggle
		:get-loader-manifest="getLoaderManifest"
		:finish-disabled="!canSetup"
		:finish-disabled-tooltip="
			!canSetup ? formatMessage(commonMessages.noPermissionAction) : undefined
		"
		:browse-modpacks="browseModpacks"
		:on-back="onBack"
		@create="onCreate"
		@after-hide="onAfterHide"
		@after-show="flow.markShown"
	/>
	<ServerShareUnlinkModal ref="shareUnlinkModal" />
</template>

<script setup lang="ts">
import type { Server } from '@amberite/contracts/core'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { until } from '@vueuse/core'
import { computed, nextTick, useTemplateRef, watch } from 'vue'
import { isNavigationFailure, NavigationFailureType, type RouteLocationRaw } from 'vue-router'

import type {
	CreationFlowContextValue,
	LoaderManifestResolver,
} from '#ui/components/flows/creation-flow-modal/creation-flow-context'
import CreationFlowModal from '#ui/components/flows/creation-flow-modal/index.vue'
import { installServerCreationConfig } from '#ui/components/flows/creation-flow-modal/server-install'
import ServerShareUnlinkModal from '#ui/components/servers/sharing/ServerShareUnlinkModal.vue'
import { useVIntl } from '#ui/composables/i18n'
import { hasServerPermission } from '#ui/composables/server-permissions'
import { serverKeys } from '#ui/composables/server-queries'
import { provideShareUnlinkPrompt } from '#ui/composables/server-sharing'
import { injectModrinthClient, injectNotificationManager } from '#ui/providers'
import { injectInstanceServerSource } from '#ui/providers/instance-server-source'
import { commonMessages } from '#ui/utils/common-messages'

import { injectServerOnboardingFlow, type ServerOnboardingRequest } from './context'
import { useDismissServerIntro } from './use-dismiss-server-intro'

const props = defineProps<{
	browsePath: string
	navigate: (to: RouteLocationRaw) => Promise<unknown>
	pageReady: boolean
	getLoaderManifest?: LoaderManifestResolver
}>()

const flow = injectServerOnboardingFlow()
const client = injectModrinthClient()
const queryClient = useQueryClient()
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const dismissServerIntro = useDismissServerIntro()
const instanceSource = injectInstanceServerSource(null)
const modalRef = useTemplateRef<InstanceType<typeof CreationFlowModal>>('modalRef')
const shareUnlinkModal =
	useTemplateRef<InstanceType<typeof ServerShareUnlinkModal>>('shareUnlinkModal')
provideShareUnlinkPrompt({
	ask: async (server) => (await shareUnlinkModal.value?.ask(server)) ?? false,
})
const canSetup = computed(() => hasServerPermission(0, 'SETUP'))
let opening = false
let introCompletion: Promise<void> | null = null

async function navigateAndWaitForPage(to: RouteLocationRaw) {
	const result = await props.navigate(to)
	if (
		result === false ||
		(isNavigationFailure(result) && !isNavigationFailure(result, NavigationFailureType.duplicated))
	) {
		throw result || new Error('Navigation was cancelled')
	}
	await nextTick()
	await until(() => props.pageReady).toBe(true)
	await nextTick()
}

async function browseModpacks() {
	const request = flow.request.value
	if (!request) return
	await navigateAndWaitForPage({
		path: props.browsePath,
		query: { sid: request.serverId, from: 'onboarding' },
	})
}

function onBack() {
	if (!flow.request.value?.backToBrowse) return false
	modalRef.value?.hide()
}

async function onAfterHide() {
	if (opening) return
	const request = flow.request.value
	request?.onHide?.()
	await introCompletion?.catch(() => {})
	if (flow.request.value === request) flow.clear()
}

const install = useMutation({
	mutationFn: async ({
		config,
		request,
	}: {
		config: CreationFlowContextValue
		request: ServerOnboardingRequest
	}) => {
		// A server created from an instance installs that instance's share and keeps following it.
		const instance = config.instanceSelection.value
		if (instance && instanceSource) {
			return instanceSource.pushInstance({
				serverId: request.serverId,
				instanceId: instance.id,
				properties: config.buildProperties(),
			})
		}
		const { id: coreId } = await client.cores.coreOf(request.serverId)
		return installServerCreationConfig(client, config, coreId, request.serverId)
	},
	onSuccess: (_result, { request }) => {
		queryClient.setQueryData<Server>(serverKeys.detail(request.serverId), (server) =>
			server ? { ...server, status: 'installing' } : server,
		)
	},
})

async function onCreate(config: CreationFlowContextValue) {
	if (install.isPending.value) return
	const request = flow.request.value
	if (!request || !canSetup.value) {
		config.loading.value = false
		return
	}

	try {
		await install.mutateAsync({ config, request })
	} catch (error) {
		config.loading.value = false
		handleError(error as Error)
		return
	}

	// Players reach a server that follows an instance through the instance's share, not a server
	// invite link, so setup ends here.
	if (config.instanceSelection.value && instanceSource) {
		modalRef.value?.hide()
		introCompletion = dismissServerIntro.mutateAsync(request.serverId).then(() => {})
		try {
			await navigateAndWaitForPage(`/hosting/manage/${request.serverId}`)
		} catch (error) {
			handleError(error as Error)
		}
		return
	}

	flow.inviteActive.value = true
	const navigateToOverview = () => navigateAndWaitForPage(`/hosting/manage/${request.serverId}`)
	config.navigating.value = true
	config.showInvite(request.serverId, request.siteUrl, () => {
		introCompletion = dismissServerIntro.mutateAsync(request.serverId).then(() => {})
		return introCompletion
	})
	try {
		await navigateToOverview()
	} catch (error) {
		handleError(error as Error)
	} finally {
		config.navigating.value = false
	}
}

watch(
	() => flow.request.value,
	async (request) => {
		if (!request) return
		opening = true
		introCompletion = null
		try {
			await nextTick()
			if (!modalRef.value) throw new Error('Server setup modal is unavailable')
			await modalRef.value.show(async (ctx) => {
				const project = request.project
				if (!project) return
				if (project.contentType && project.contentType !== 'modpack') {
					await ctx.selectProject(project.projectId, project.contentType, project.versionId)
				} else {
					ctx.setupType.value = 'modpack'
					ctx.modpackSelection.value = project
					ctx.modal.value?.setStage('final-config')
				}
			})
		} catch (error) {
			flow.fail(error)
		} finally {
			opening = false
		}
	},
	{ immediate: true },
)
</script>
