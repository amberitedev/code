<template>
	<div v-if="loadError" class="flex min-h-full items-center justify-center p-6 text-contrast">
		<ErrorInformationCard
			title="Server unavailable"
			:description="loadError.message"
			:icon="TriangleAlertIcon"
			icon-color="red"
			:action="serversAction"
		/>
	</div>
	<div v-else-if="server" class="h-full w-full pt-6">
		<div
			data-core-server-manager-root
			class="relative mx-auto box-border flex w-full min-w-0 flex-col gap-4 px-6 pb-6 transition-all duration-300"
			:class="{ 'max-w-[1280px]': constrainWidth }"
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
					<div class="flex flex-wrap gap-2">
						<ButtonStyled v-if="powerState === 'running'" color="orange" size="large">
							<button @click="restartServer">
								<UpdatedIcon />
								Restart
							</button>
						</ButtonStyled>
						<ButtonStyled v-if="powerState === 'running'" color="red" size="large">
							<button @click="stopServer">
								<StopCircleIcon />
								Stop
							</button>
						</ButtonStyled>
						<ButtonStyled v-else-if="server.flows?.intro" color="brand" size="large">
							<button disabled>
								<SpinnerIcon class="animate-spin" />
								Installing...
							</button>
						</ButtonStyled>
						<ButtonStyled v-else-if="server.status === 'broken'" color="red" size="large">
							<button @click="repairServer">
								<TriangleAlertIcon />
								Repair
							</button>
						</ButtonStyled>
						<ButtonStyled v-else color="brand" size="large">
							<button
								:disabled="powerState === 'starting' || powerState === 'stopping'"
								@click="startServer"
							>
								<PlayIcon />
								{{ powerState === 'starting' ? 'Starting' : 'Start' }}
							</button>
						</ButtonStyled>
						<ButtonStyled v-if="showCopyIdAction" circular type="transparent" size="large">
							<button v-tooltip="'Copy server ID'" @click="copyId">
								<ClipboardCopyIcon />
							</button>
						</ButtonStyled>
						<ButtonStyled circular size="large">
							<button v-tooltip="'Server settings'" @click="settingsOpen = true">
								<SettingsIcon />
							</button>
						</ButtonStyled>
						<ButtonStyled v-if="powerState === 'running'" circular type="transparent" size="large">
							<button v-tooltip="'Kill server'" class="text-red" @click="killServer">
								<SlashIcon />
							</button>
						</ButtonStyled>
					</div>
				</template>
			</ServerManageHeader>

			<div
				data-core-navigation
				class="isolate flex w-full select-none flex-col justify-between gap-4 overflow-auto md:flex-row md:items-center"
			>
				<NavTabs :links="navLinks" replace />
			</div>

			<div data-core-mount class="h-full w-full flex-1">
				<Suspense>
					<slot :on-reinstall="repairServer" :on-reinstall-failed="refreshServer" />
				</Suspense>
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
</template>

<script setup lang="ts">
import {
	BoxesIcon,
	ClipboardCopyIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	PlayIcon,
	SettingsIcon,
	SlashIcon,
	SpinnerIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	TriangleAlertIcon,
	UpdatedIcon,
	UsersIcon,
} from '@modrinth/assets'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import ErrorInformationCard from '#ui/components/base/ErrorInformationCard.vue'
import NavTabs from '#ui/components/base/NavTabs.vue'
import ServerManageHeader from '#ui/components/servers/server-header/ServerManageHeader.vue'

import CoreServerSettingsModal from './settings/CoreServerSettingsModal.vue'
import { useCoreServerRuntime } from './use-core-server-runtime'

const props = withDefaults(
	defineProps<{
		serverId: string
		basePath?: string
		serversPath?: string
		constrainWidth?: boolean
		showCopyIdAction?: boolean
		reloadPage?: () => void
		resolveViewer?: () => Promise<{ userId: string | null; userRole: string | null }>
		authUser?: unknown
		navigateToServers?: () => void
		showAdvancedDebugInfo?: boolean
	}>(),
	{
		basePath: undefined,
		serversPath: '/hosting/manage',
		constrainWidth: false,
		showCopyIdAction: false,
		reloadPage: undefined,
		resolveViewer: undefined,
		authUser: undefined,
		navigateToServers: undefined,
		showAdvancedDebugInfo: false,
	},
)

const router = useRouter()
const settingsOpen = ref(false)
const {
	instanceId,
	server,
	loadError,
	statsData,
	powerState,
	startServer,
	stopServer,
	restartServer,
	killServer,
	repairServer,
	refreshServer,
	copyId,
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

const basePath = computed(() => props.basePath ?? `/hosting/manage/${encodeURIComponent(props.serverId)}`)
const displayUptimeSeconds = computed(() => {
	void tickSecond.value
	return statsData.value?.total_uptime_seconds ?? statsData.value?.uptime_seconds ?? 0
})
const navLinks = computed(() => [
	{ label: 'Overview', href: basePath.value, icon: TerminalSquareIcon },
	{ label: 'Content', href: `${basePath.value}/content`, icon: BoxesIcon },
	{ label: 'Files', href: `${basePath.value}/files`, icon: FolderOpenIcon },
	{ label: 'Backups', href: `${basePath.value}/backups`, icon: DatabaseBackupIcon },
	{ label: 'Access', href: `${basePath.value}/access`, icon: UsersIcon },
])
const serversAction = computed(() => ({
	label: 'Back to servers',
	onClick: () => {
		if (props.navigateToServers) {
			props.navigateToServers()
			return
		}
		void router.push(props.serversPath)
	},
	color: 'standard' as const,
	icon: TerminalSquareIcon,
}))
</script>
