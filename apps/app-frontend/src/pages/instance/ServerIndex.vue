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
			<ServerManageHeader :server="server" :uptime-seconds="statsData?.uptime_seconds ?? 0">
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
						<ButtonStyled v-else color="brand" size="large">
							<button
								:disabled="powerState === 'starting' || powerState === 'stopping'"
								@click="startServer"
							>
								<PlayIcon />
								{{ powerState === 'starting' ? 'Starting' : 'Start' }}
							</button>
						</ButtonStyled>
						<ButtonStyled circular type="transparent" size="large">
							<OverflowMenu
								:options="[
									{ id: 'allServers', action: () => router.push('/library/servers') },
									{ id: 'copy-id', action: copyId },
									{ id: 'kill', action: killServer, color: 'red' },
								]"
							>
								<MoreVerticalIcon />
								<template #allServers><ServerStackIcon /> All servers</template>
								<template #copy-id><ClipboardCopyIcon /> Copy ID</template>
								<template #kill><SlashIcon /> Kill server</template>
							</OverflowMenu>
						</ButtonStyled>
					</div>
				</template>
			</ServerManageHeader>

			<div
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
	</div>
	<div v-else-if="loadError" class="flex min-h-full items-center justify-center p-6 text-contrast">
		<ErrorInformationCard
			title="Server unavailable"
			:description="loadError.message"
			:icon="TriangleAlertIcon"
			icon-color="red"
		/>
	</div>
	<div v-else class="flex min-h-full items-center justify-center">
		<SpinnerIcon class="animate-spin w-8 h-8 text-contrast" />
	</div>
</template>

<script setup lang="ts">
import {
	BoxesIcon,
	ClipboardCopyIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	MoreVerticalIcon,
	PlayIcon,
	ServerStackIcon,
	SlashIcon,
	SpinnerIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	TriangleAlertIcon,
	UpdatedIcon,
} from '@modrinth/assets'
import {
	ButtonStyled,
	ErrorInformationCard,
	NavTabs,
	OverflowMenu,
	ServerManageHeader,
} from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useCoreServerRuntime } from './server/use-core-server-runtime'
import ServerBackups from './ServerBackups.vue'
import ServerContent from './ServerContent.vue'
import ServerFiles from './ServerFiles.vue'
import ServerOverview from './ServerOverview.vue'

const route = useRoute()
const router = useRouter()
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
	copyId,
} = useCoreServerRuntime()

const navLinks = computed(() => {
	const basePath = `/instance/${encodeURIComponent(instanceId.value)}`
	return [
		{ label: 'Overview', href: basePath, icon: TerminalSquareIcon },
		{ label: 'Content', href: `${basePath}/content`, icon: BoxesIcon },
		{ label: 'Files', href: `${basePath}/files`, icon: FolderOpenIcon },
		{ label: 'Backups', href: `${basePath}/backups`, icon: DatabaseBackupIcon },
	]
})
const activePage = computed(() => {
	if (route.path.endsWith('/content')) return ServerContent
	if (route.path.endsWith('/files')) return ServerFiles
	if (route.path.endsWith('/backups')) return ServerBackups
	return ServerOverview
})
</script>
