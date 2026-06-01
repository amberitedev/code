<template>
	<div v-if="server" class="h-full w-full pt-6">
		<div
			data-pyro-server-manager-root
			class="relative mx-auto box-border flex w-full min-w-0 flex-col gap-4 px-6 pb-6 transition-all duration-300"
			:style="{
				'--server-bg-image':
					'linear-gradient(180deg, rgba(153,153,153,1) 0%, rgba(87,87,87,1) 100%)',
			}"
		>
			<ServerManageHeader
				v-if="!isBrowsePage"
				:server="server"
				:uptime-seconds="displayUptimeSeconds"
			>
				<template #actions>
					<div class="flex gap-2">
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
						<ButtonStyled v-else-if="server?.flows?.intro" color="brand" size="large">
							<button disabled>
								<SpinnerIcon class="animate-spin" />
								Installing...
							</button>
						</ButtonStyled>
						<ButtonStyled v-else-if="server?.status === 'broken'" color="red" size="large">
							<button @click="repairServer">
								<TriangleAlertIcon />
								Install failed — Repair
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
						<ButtonStyled circular size="large">
							<button v-tooltip="'Server settings'" @click="openSettings()">
								<SettingsIcon />
							</button>
						</ButtonStyled>
						<ButtonStyled circular type="transparent" size="large">
							<OverflowMenu
								:options="[
									{ id: 'copy-id', action: copyId },
									{ id: 'kill', action: killServer, color: 'red' },
								]"
							>
								<MoreVerticalIcon />
								<template #copy-id><ClipboardCopyIcon /> Copy ID</template>
								<template #kill><SlashIcon /> Kill server</template>
							</OverflowMenu>
						</ButtonStyled>
					</div>
				</template>
			</ServerManageHeader>

			<div
				v-if="!isBrowsePage"
				data-pyro-navigation
				class="isolate flex w-full select-none flex-col justify-between gap-4 overflow-auto md:flex-row md:items-center"
			>
				<NavTabs :links="navLinks" replace />
			</div>

			<div data-pyro-mount class="h-full w-full flex-1">
				<Suspense>
					<component :is="activePage" />
				</Suspense>
			</div>
		</div>
		<ServerSettingsModal />
	</div>
	<div v-else-if="loadError" class="flex min-h-full items-center justify-center p-6 text-contrast">
		<ErrorInformationCard
			title="Server unavailable"
			:description="loadError.message"
			:icon="TriangleAlertIcon"
			icon-color="red"
		/>
	</div>
	<div v-else class="h-full w-full pt-6">
		<div class="mx-auto flex w-full min-w-0 flex-col gap-4 px-6 pb-6">
			<div
				class="flex animate-pulse flex-row items-center gap-4 rounded-2xl border-[1px] border-solid border-button-bg bg-bg-raised p-4"
			>
				<div class="size-16 rounded-xl bg-button-bg"></div>
				<div class="flex flex-1 flex-col gap-2">
					<div class="h-6 w-48 rounded bg-button-bg"></div>
					<div class="h-4 w-64 rounded bg-button-bg opacity-75"></div>
				</div>
				<div class="h-10 w-28 rounded-xl bg-button-bg"></div>
			</div>
			<div class="flex animate-pulse flex-row gap-3">
				<div v-for="i in 4" :key="i" class="h-8 w-24 rounded-lg bg-button-bg"></div>
			</div>
			<div class="h-64 w-full animate-pulse rounded-2xl bg-bg-raised"></div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {
	BoxesIcon,
	ClipboardCopyIcon,
	ClockIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	MoreVerticalIcon,
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
import { ButtonStyled, ErrorInformationCard, NavTabs, OverflowMenu } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import ServerManageHeader from './server/ServerManageHeader.vue'
import ServerSettingsModal from './server/settings/ServerSettingsModal.vue'
import { useCoreServerRuntime } from './server/use-core-server-runtime'
import ServerBackups from './ServerBackups.vue'
import ServerBrowsePage from './ServerBrowsePage.vue'
import ServerContent from './ServerContent.vue'
import ServerFiles from './ServerFiles.vue'
import ServerOverview from './ServerOverview.vue'
import ServerPlayers from './ServerPlayers.vue'
import ServerTasks from './ServerTasks.vue'

const route = useRoute()
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
	copyId,
	openSettings,
} = useCoreServerRuntime()

// Tick every second so the displayed uptime increments smoothly.
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

// Use total_uptime_seconds (accumulated + current session) for a meaningful
// all-time uptime counter. Falls back to session uptime_seconds when total
// is not yet available (older Core versions).
const displayUptimeSeconds = computed(() => {
	void tickSecond.value // reactive dependency to tick
	const total = statsData.value?.total_uptime_seconds
	if (total != null) return total
	return statsData.value?.uptime_seconds ?? 0
})

const navLinks = computed(() => {
	const basePath = `/instance/${encodeURIComponent(instanceId.value)}`
	return [
		{ label: 'Overview', href: basePath, icon: TerminalSquareIcon },
		{ label: 'Content', href: `${basePath}/content`, icon: BoxesIcon },
		{ label: 'Files', href: `${basePath}/files`, icon: FolderOpenIcon },
		{ label: 'Players', href: `${basePath}/players`, icon: UsersIcon },
		{ label: 'Backups', href: `${basePath}/backups`, icon: DatabaseBackupIcon },
		{ label: 'Tasks', href: `${basePath}/tasks`, icon: ClockIcon },
	]
})
const activePage = computed(() => {
	if (route.path.endsWith('/browse')) return ServerBrowsePage
	if (route.path.endsWith('/content')) return ServerContent
	if (route.path.endsWith('/files')) return ServerFiles
	if (route.path.endsWith('/players')) return ServerPlayers
	if (route.path.endsWith('/backups')) return ServerBackups
	if (route.path.endsWith('/tasks')) return ServerTasks
	return ServerOverview
})

const isBrowsePage = computed(() => route.path.endsWith('/browse'))
</script>
