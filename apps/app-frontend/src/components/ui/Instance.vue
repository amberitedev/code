<script setup>
import {
	DownloadIcon,
	GameIcon,
	PlayIcon,
	ServerIcon,
	SpinnerIcon,
	StopCircleIcon,
	TimerIcon,
} from '@modrinth/assets'
import { Avatar, IconButton, injectNotificationManager, useRelativeTime } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import dayjs from 'dayjs'
import { computed, inject, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { trackEvent } from '@/helpers/analytics'
import { process_listener } from '@/helpers/events'
import { install_existing_instance, install_pack_to_existing_instance } from '@/helpers/install'
import { kill, run } from '@/helpers/instance'
import { get_by_instance_id } from '@/helpers/process'
import { showInstanceInFolder } from '@/helpers/utils.js'
import { handleSevereError } from '@/store/error.js'

const { handleError } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()

const props = defineProps({
	instance: {
		type: Object,
		default() {
			return {}
		},
	},
	compact: {
		type: Boolean,
		default: false,
	},
	first: {
		type: Boolean,
		default: false,
	},
})

const playing = ref(false)
const loading = ref(false)
const modLoading = computed(
	() =>
		loading.value ||
		currentEvent.value === 'installing' ||
		(currentEvent.value === 'launched' && !playing.value),
)
const installing = computed(() => props.instance.install_stage.includes('installing'))
const installed = computed(() => props.instance.install_stage === 'installed')
const isServerProfile = computed(() => props.instance.profile_type === 'server')
const instanceTypeLabel = computed(() => {
	if (props.instance.profile_type === 'server') return 'Server'
	if (props.instance.profile_type === 'synced') return 'Synced'
	return 'Client'
})

const router = useRouter()
const beginLibraryInstanceOpenNavigation = inject('beginLibraryInstanceOpenNavigation', null)

const instanceRoute = computed(() => `/instance/${encodeURIComponent(props.instance.path)}`)

async function preloadInstanceRoute() {
	const matchedRoutes = router.resolve(instanceRoute.value).matched
	const preloadPromises = []

	for (const record of matchedRoutes) {
		for (const component of Object.values(record.components ?? {})) {
			if (typeof component === 'function') {
				preloadPromises.push(Promise.resolve(component()).catch(() => undefined))
			}
		}
	}

	await Promise.all(preloadPromises)
}

const seeInstance = async () => {
	const target = instanceRoute.value
	beginLibraryInstanceOpenNavigation?.(target)
	void preloadInstanceRoute()
	await router.push(target)
}

const checkProcess = async () => {
	const runningProcesses = await get_by_instance_id(props.instance.id).catch(handleError)

	playing.value = runningProcesses.length > 0
}

const handleCardWarmup = () => {
	void preloadInstanceRoute()
	void checkProcess()
}

const handleRouteWarmup = () => {
	void preloadInstanceRoute()
}

const play = async (e, context) => {
	e?.stopPropagation()
	if (props.instance.quarantined) return
	loading.value = true
	await run(props.instance.id)
		.catch((err) => handleSevereError(err, { instanceId: props.instance.id }))
		.finally(() => {
			trackEvent('InstanceStart', {
				loader: props.instance.loader,
				game_version: props.instance.game_version,
				source: context,
			})
		})
	loading.value = false
}

const stop = async (e, context) => {
	e?.stopPropagation()
	playing.value = false

	await kill(props.instance.id).catch(handleError)

	trackEvent('InstanceStop', {
		loader: props.instance.loader,
		game_version: props.instance.game_version,
		source: context,
	})
}

const repair = async (e) => {
	e?.stopPropagation()
	if (props.instance.quarantined) return

	if (
		props.instance.install_stage !== 'pack_installed' &&
		(props.instance.link?.type === 'modrinth_modpack' ||
			props.instance.link?.type === 'server_project_modpack')
	) {
		await install_pack_to_existing_instance(props.instance.id, {
			type: 'fromVersionId',
			project_id: props.instance.link.project_id ?? props.instance.link.server_project_id ?? '',
			version_id: props.instance.link.version_id ?? props.instance.link.content_version_id ?? '',
			title: props.instance.name,
		}).catch(handleError)
	} else {
		await install_existing_instance(props.instance.id, false).catch(handleError)
	}
}

const openFolder = async () => {
	await showInstanceInFolder(props.instance.id)
}

const addContent = async () => {
	if (props.instance.quarantined) return
	await router.push({
		path: `/browse/${props.instance.loader === 'vanilla' ? 'datapack' : 'mod'}`,
		query: { i: props.instance.id },
	})
}

defineExpose({
	play,
	stop,
	seeInstance,
	openFolder,
	addContent,
	instance: props.instance,
})

const currentEvent = ref(null)

let mounted = false
let unlistenProcess = null

const handleProcessEvent = (e) => {
	if (e.profile_path_id === props.instance.path) {
		currentEvent.value = e.event
		if (e.event === 'finished') {
			playing.value = false
		}
	}
}

onMounted(() => {
	mounted = true
	void checkProcess()

	void process_listener(handleProcessEvent)
		.then((unlisten) => {
			if (!mounted) {
				unlisten()
				return
			}

			unlistenProcess = unlisten
		})
		.catch(handleError)
})
onUnmounted(() => {
	mounted = false
	unlistenProcess?.()
	unlistenProcess = null
})
</script>

<template>
	<template v-if="compact">
		<div
			class="card-shadow grid grid-cols-[auto_1fr_auto] bg-bg-raised rounded-xl p-3 pl-4 gap-2 cursor-pointer hover:brightness-90 transition-all"
			@click="seeInstance"
			@mouseenter="handleCardWarmup"
			@focusin="handleRouteWarmup"
			@pointerdown="handleRouteWarmup"
		>
			<div class="relative flex items-center justify-center">
				<Avatar
					size="48px"
					:src="instance.icon_path ? convertFileSrc(instance.icon_path) : null"
					:tint-by="instance.path"
					alt="Mod card"
				/>
			</div>
			<div class="flex min-w-0 flex-col justify-center gap-1 leading-normal">
				<span class="line-clamp-1 font-bold text-contrast">{{ instance.name }}</span>
				<div class="flex min-w-0 items-center gap-1.5 text-xs font-semibold text-secondary">
					<component :is="isServerProfile ? ServerIcon : GameIcon" class="size-3.5 shrink-0" />
					<span class="truncate capitalize">
						{{ instanceTypeLabel }} - {{ instance.loader }} {{ instance.game_version }}
					</span>
				</div>
			</div>
			<div class="flex items-center">
				<IconButton
					v-if="playing"
					v-tooltip="'Stop'"
					type="colored"
					color="red"
					:label="'Stop'"
					@mouseenter="checkProcess"
					@click="(e) => stop(e, 'InstanceCard')"
				>
					<StopCircleIcon />
				</IconButton>
				<IconButton
					v-else-if="modLoading"
					v-tooltip="'Instance is loading...'"
					:label="'Instance is loading...'"
					disabled
				>
					<SpinnerIcon class="animate-spin" />
				</IconButton>
				<IconButton
					v-else-if="!instance.quarantined"
					v-tooltip="'Play'"
					:type="first ? 'colored' : 'base'"
					:color="first ? 'brand' : undefined"
					label="Play"
					@click="(e) => play(e, 'InstanceCard')"
					@mouseenter="checkProcess"
				>
					<!-- Translate for optical centering -->
					<PlayIcon class="translate-x-[1px]" />
				</IconButton>
			</div>
			<div class="flex min-w-0 items-center col-span-3 gap-1 text-secondary font-semibold">
				<TimerIcon />
				<span class="text-sm">
					<template v-if="instance.last_played">
						Played {{ formatRelativeTime(dayjs(instance.last_played).toISOString()) }}
					</template>
					<template v-else> Never played </template>
				</span>
			</div>
		</div>
	</template>
	<div v-else>
		<div
			class="button-base bg-bg-raised p-4 rounded-xl flex gap-3 group"
			@click="seeInstance"
			@mouseenter="handleCardWarmup"
			@focusin="handleRouteWarmup"
			@pointerdown="handleRouteWarmup"
		>
			<div class="relative flex items-center justify-center">
				<Avatar
					size="48px"
					:src="instance.icon_path ? convertFileSrc(instance.icon_path) : null"
					:tint-by="instance.id"
					alt="Mod card"
					:class="`transition-all ${modLoading || installing ? `brightness-[0.25] scale-[0.85]` : `group-hover:brightness-75`}`"
				/>
				<div class="absolute inset-0 flex items-center justify-center">
					<IconButton
						v-if="playing"
						v-tooltip="'Stop'"
						type="colored"
						color="red"
						size="xl"
						:label="'Stop'"
						:class="{ 'scale-100 opacity-100': playing }"
						class="transition-all scale-75 origin-bottom opacity-0 card-shadow"
						@click="(e) => stop(e, 'InstanceCard')"
						@mouseenter="checkProcess"
					>
						<StopCircleIcon />
					</IconButton>
					<SpinnerIcon
						v-else-if="modLoading || installing"
						v-tooltip="modLoading ? 'Instance is loading...' : 'Installing...'"
						class="animate-spin w-8 h-8"
						tabindex="-1"
					/>
					<IconButton
						v-else-if="!installed && !instance.quarantined"
						v-tooltip="'Repair'"
						type="colored"
						color="brand"
						size="xl"
						:label="'Repair'"
						class="transition-all scale-75 group-hover:scale-100 group-focus-within:scale-100 origin-bottom opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 card-shadow"
						@click="(e) => repair(e)"
					>
						<DownloadIcon />
					</IconButton>
					<IconButton
						v-else-if="!instance.quarantined"
						v-tooltip="'Play'"
						type="colored"
						color="brand"
						size="xl"
						:label="'Play'"
						class="transition-all scale-75 group-hover:scale-100 group-focus-within:scale-100 origin-bottom opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 card-shadow"
						@click="(e) => play(e, 'InstanceCard')"
						@mouseenter="checkProcess"
					>
						<PlayIcon class="translate-x-[2px]" />
					</IconButton>
				</div>
			</div>
			<div class="flex min-w-0 flex-col gap-1">
				<p class="m-0 text-md font-bold text-contrast leading-tight line-clamp-1">
					{{ instance.name }}
				</p>
				<div class="flex min-w-0 items-center col-span-3 gap-1 text-secondary font-semibold">
					<component :is="isServerProfile ? ServerIcon : GameIcon" class="size-4 shrink-0" />
					<span class="truncate text-sm capitalize">
						{{ instanceTypeLabel }} - {{ instance.loader }} {{ instance.game_version }}
					</span>
				</div>
			</div>
		</div>
	</div>
</template>
