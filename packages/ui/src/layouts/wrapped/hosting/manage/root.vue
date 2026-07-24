<template>
	<div
		v-if="loadError"
		data-library-instance-page-ready
		data-library-instance-drag-disabled
		data-library-instance-error-state
		class="flex min-h-full items-center justify-center p-6 text-contrast"
	>
		<ErrorInformationCard
			title="Server unavailable"
			:description="loadError.message"
			:icon="TriangleAlertIcon"
			icon-color="red"
			:action="serversAction"
		/>
	</div>
	<div
		v-else-if="server"
		class="relative mx-auto box-border flex w-full min-w-0 flex-col gap-4 px-6 pt-6 transition-all duration-300"
		:class="
			constrainWidth ? 'min-h-[100svh] max-w-[1280px] pb-16' : 'min-h-[calc(100svh-100px)] pb-6'
		"
	>
		<div
			data-core-server-manager-root
			class="relative flex w-full min-w-0 flex-col gap-4"
			:style="{
				'--server-bg-image':
					'linear-gradient(180deg, rgba(153,153,153,1) 0%, rgba(87,87,87,1) 100%)',
			}"
		>
			<ServerManageHeader
				:server="server"
				:uptime-seconds="displayUptimeSeconds"
				:server-address="server.net?.domain"
			>
				<template #actions>
					<div class="flex gap-2">
						<PanelServerActionButton />
						<ButtonStyled circular size="large">
							<button v-tooltip="'Server settings'" @click="settingsOpen = true">
								<SettingsIcon />
							</button>
						</ButtonStyled>
						<PanelServerOverflowMenu
							:uptime-seconds="displayUptimeSeconds"
							:show-copy-id-action="showCopyIdAction"
							:copy-id-label="copyIdLabel"
							:show-debug-info="showAdvancedDebugInfo"
						/>
					</div>
					</template>
				</ServerManageHeader>

			<div
				data-core-navigation
				class="isolate flex w-full select-none flex-col justify-between gap-4 overflow-auto md:flex-row md:items-center"
			>
				<NavTabs
					mode="local"
					:links="navLinks"
					:active-index="visibleManageTabIndex"
					@tab-click="manageTabController.selectTab"
				/>
			</div>

			<div data-core-mount class="h-full w-full flex-1">
				<NavTabContentTransition
					:content-key="manageTabContentKey"
					:direction="manageTabSlideDirection"
					:visible="manageTabContentVisible"
					@before-leave="manageTabController.handleBeforeLeave"
					@after-leave="manageTabController.handleAfterLeave"
					@after-enter="manageTabController.handleAfterEnter"
					@enter-cancelled="manageTabController.handleEnterCancelled"
					@leave-cancelled="manageTabController.handleLeaveCancelled"
				>
					<Suspense>
						<slot :on-reinstall="repairServer" :on-reinstall-failed="refreshServer" />
					</Suspense>
				</NavTabContentTransition>
			</div>
		</div>
		<CoreServerSettingsModal
			v-model:open="settingsOpen"
			:server-id="instanceId"
			@refresh="refreshServer"
		/>
	</div>
	<div v-else class="h-full w-full pt-6">
		<div class="mx-auto flex w-full min-w-0 flex-col gap-4 px-6 pb-6">
			<div class="flex animate-pulse flex-row items-center gap-4 rounded-2xl bg-surface-3 p-4">
				<div class="size-16 rounded-xl bg-surface-4"></div>
				<div class="flex flex-1 flex-col gap-2">
					<div class="h-6 w-48 rounded bg-surface-4"></div>
					<div class="h-4 w-64 rounded bg-surface-4"></div>
				</div>
			</div>
		</div>
	</div>
	<div
		v-if="server && showAdvancedDebugInfo"
		class="relative mx-auto mt-6 box-border w-full min-w-0 px-6"
		:class="{ 'max-w-[1280px]': constrainWidth }"
	>
		<h2 class="m-0 text-lg font-extrabold text-contrast">Server data</h2>
		<pre class="markdown-body w-full overflow-auto rounded-2xl bg-bg-raised p-4 text-sm">{{
			safeStringify(server)
		}}</pre>
	</div>
</template>

<script setup lang="ts">
import {
	BoxesIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	LayoutTemplateIcon,
	LeftArrowIcon,
	SettingsIcon,
	TriangleAlertIcon,
	UsersIcon,
} from '@modrinth/assets'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import ErrorInformationCard from '#ui/components/base/ErrorInformationCard.vue'
import NavTabContentTransition from '#ui/components/base/NavTabContentTransition.vue'
import NavTabs from '#ui/components/base/NavTabs.vue'
import {
	PanelServerActionButton,
	PanelServerOverflowMenu,
	ServerManageHeader,
} from '#ui/components/servers/server-header'
import { useNavTabContentController } from '#ui/composables/nav-tab-content-controller'

import CoreServerSettingsModal from './settings/CoreServerSettingsModal.vue'
import { useCoreServerRuntime } from './use-core-server-runtime'

const props = withDefaults(
	defineProps<{
		serverId: string
		basePath?: string
		serversPath?: string
		constrainWidth?: boolean
		showCopyIdAction?: boolean
		copyIdLabel?: string
		reloadPage?: () => void
		resolveViewer?: () => Promise<{ userId: string | null; userRole: string | null }>
		authUser?: unknown
		navigateToServers?: () => void
		serversActionLabel?: string
		showAdvancedDebugInfo?: boolean
	}>(),
	{
		basePath: undefined,
		serversPath: '/hosting/manage',
		constrainWidth: false,
		showCopyIdAction: false,
		copyIdLabel: 'Copy ID',
		reloadPage: undefined,
		resolveViewer: undefined,
		authUser: undefined,
		navigateToServers: undefined,
		serversActionLabel: 'Back to servers',
		showAdvancedDebugInfo: false,
	},
)

const router = useRouter()
const route = useRoute()
const settingsOpen = ref(false)
const {
	instanceId,
	server,
	loadError,
	statsData,
	repairServer,
	refreshServer,
} = useCoreServerRuntime(computed(() => props.serverId))

const tickSecond = ref(0)
let tickInterval: ReturnType<typeof setInterval> | null = null
onMounted(() => {
	tickInterval = setInterval(() => {
		tickSecond.value++
	}, 1000)
})
onBeforeUnmount(() => {
	if (tickInterval !== null) clearInterval(tickInterval)
})

const basePath = computed(
	() => props.basePath ?? `/hosting/manage/${encodeURIComponent(props.serverId)}`,
)
function getManageTabKind(path: string) {
	if (path.endsWith('/content')) return 'content'
	if (path.endsWith('/files')) return 'files'
	if (path.endsWith('/backups')) return 'backups'
	if (path.endsWith('/access')) return 'access'
	if (path.endsWith('/browse')) return 'browse'
	return 'overview'
}

const displayUptimeSeconds = computed(() => {
	void tickSecond.value
	return statsData.value?.total_uptime_seconds ?? statsData.value?.uptime_seconds ?? 0
})
const navLinks = computed(() => [
	{ label: 'Overview', href: basePath.value, icon: LayoutTemplateIcon, kind: 'overview' },
	{ label: 'Content', href: `${basePath.value}/content`, icon: BoxesIcon, kind: 'content' },
	{ label: 'Files', href: `${basePath.value}/files`, icon: FolderOpenIcon, kind: 'files' },
	{ label: 'Backups', href: `${basePath.value}/backups`, icon: DatabaseBackupIcon, kind: 'backups' },
	{ label: 'Access', href: `${basePath.value}/access`, icon: UsersIcon, kind: 'access' },
])
const activeManageTabKind = computed(() => getManageTabKind(route.path))
const activeManageTabIndex = computed(() =>
	navLinks.value.findIndex((tab) => tab.kind === activeManageTabKind.value),
)
const manageTabController = useNavTabContentController({
	activeIndex: activeManageTabIndex,
	router,
	replace: true,
})
const visibleManageTabIndex = manageTabController.activeIndex
const manageTabSlideDirection = manageTabController.direction
const manageTabContentVisible = manageTabController.visible
const manageTabContentKey = computed(
	() => `${basePath.value}:${activeManageTabKind.value}`,
)
const serversAction = computed(() => ({
	label: props.serversActionLabel,
	onClick: () => {
		if (props.navigateToServers) {
			props.navigateToServers()
			return
		}
		void router.push(props.serversPath)
	},
	color: 'standard' as const,
	icon: LeftArrowIcon,
}))

function safeStringify(value: unknown, indent = ' '): string {
	const seen = new WeakSet()
	return JSON.stringify(
		value,
		(_key, nextValue) => {
			if (typeof nextValue === 'object' && nextValue !== null) {
				if (seen.has(nextValue)) {
					return '[Circular]'
				}
				seen.add(nextValue)
			}
			return nextValue
		},
		indent,
	)
}
</script>
