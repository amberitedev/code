<script setup lang="ts">
import type { Copal } from '@modrinth/api-client'
import { InfoIcon, PlusIcon, UnlinkIcon, UpdatedIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	Combobox,
	type ComboboxOption,
	ConfirmModal,
	injectAuth,
	injectModrinthClient,
	injectNotificationManager,
	SegmentedBar,
	Slider,
	Toggle,
	useFormatBytes,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useDebounceFn } from '@vueuse/core'
import { computed, ref, watch } from 'vue'

import AccountRequired from '@/features/cores/account-required.vue'
import { coresQueryKey, injectCoreActions, useCores } from '@/features/cores/use-cores'
import { coreOffline, serversQueryKey, useServers } from '@/features/servers/linked-servers'
import { useSharedStorage } from '@/features/settings/sharing/use-shared-storage'

// One of the account's Cores, picked at the top: the storage it lends the pool and its daily
// transfer. Read from the Core itself, so the settings show only while it answers.

const GiB = 1024 ** 3
/** The daily transfer slider's top, unless the saved cap is higher. */
const TRANSFER_TOP_GIB = 250

const client = injectModrinthClient()
const auth = injectAuth()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const formatBytes = useFormatBytes()
// Kept fresh from the App root, so this page has its answer the moment it opens. Only the
// account's own Cores are managed here, not ones it was invited to.
const { ownCores: cores, data, isFetching, error, refetch } = useCores()
const { openAddCore } = injectCoreActions()
const { data: servers } = useServers()
const { data: shared } = useSharedStorage()

const ADD = 'add'
const selectedId = ref<string>()
const core = computed(
	() => cores.value.find((each) => each.id === selectedId.value) ?? cores.value[0],
)
const coreOptions = computed<ComboboxOption<string>[]>(() => [
	...cores.value.map((each) => ({ value: each.id, label: each.name })),
	{ value: ADD, label: 'Add Core', icon: PlusIcon, action: () => openAddCore() },
])
function pick(id: string | undefined) {
	if (id && id !== ADD) selectedId.value = id
}

/** Whether the Core answered the last server check. Undefined before the first check finishes. */
const online = computed(() => {
	if (!core.value) return undefined
	if (coreOffline.value) return false
	return servers.value ? !servers.value.offline_cores?.includes(core.value.id) : undefined
})

const storageKey = computed(() => ['core-storage', core.value?.id])
const { data: storage } = useQuery({
	queryKey: storageKey,
	queryFn: () => client.copal.storage_v1.get(core.value!.id),
	enabled: computed(() => !!core.value && online.value === true),
	staleTime: 30_000,
	retry: false,
})

const change = useMutation({
	mutationFn: (settings: Copal.StorageSettings) =>
		client.copal.storage_v1.change(core.value!.id, settings),
	onSuccess: (status) => queryClient.setQueryData(storageKey.value, status),
	onError: (cause) => {
		console.error('Failed to change Core storage', cause)
		resetSliders()
		addNotification({
			type: 'error',
			title: `Could not change ${core.value?.name}`,
			text: 'Check that the Core is online, then try again.',
		})
	},
})

const enabled = computed({
	get: () => storage.value?.enabled ?? false,
	set: (value: boolean) => change.mutate({ enabled: value }),
})

// Everything the store may use: what it holds and what is free on its disk.
const diskGiB = computed(() =>
	storage.value ? Math.max(Math.floor((storage.value.used + storage.value.free) / GiB), 1) : 1,
)

const segments = computed(() => {
	const status = storage.value
	if (!status) return []
	// The account's own shares on this Core. Shares save space between versions, so this is an
	// estimate and stays within what the Core holds.
	const own = Math.min(
		(shared.value?.shares ?? [])
			.filter((share) => share.cores.includes(core.value!.id))
			.reduce((sum, share) => sum + share.size, 0),
		status.used,
	)
	const room = Math.max(status.cap - status.used, 0)
	return [
		{ id: 'own', label: 'Your shares', value: own, color: 'bg-blue' },
		{ id: 'friends', label: "Friends' shares", value: status.used - own, color: 'bg-purple' },
		{ id: 'room', label: 'Room left', value: room, color: 'bg-gray' },
	].map((segment) => ({ ...segment, display: formatBytes(segment.value, 1) }))
})

// What the sliders show, in GiB. They follow the saved caps and reset when a change fails.
const capGiB = ref(1)
const transferGiB = ref(1)
function resetSliders() {
	capGiB.value = Math.round((storage.value?.cap ?? GiB) / GiB)
	transferGiB.value = Math.round((storage.value?.egress_cap ?? GiB) / GiB)
}
watch(storage, resetSliders, { immediate: true })

// Saves once the slider rests.
const saveCap = useDebounceFn((field: 'cap' | 'egress_cap', gib: number) => {
	const bytes = gib * GiB
	if (bytes !== storage.value?.[field]) change.mutate({ [field]: bytes })
}, 600)

const confirmRemove = ref<InstanceType<typeof ConfirmModal>>()
const removing = ref(false)

async function remove() {
	const removed = core.value
	if (!removed || removing.value) return
	removing.value = true
	try {
		await client.archon.cores_v1.revoke(removed)
		selectedId.value = undefined
	} catch (cause) {
		console.error('Failed to remove Core', cause)
		addNotification({
			type: 'error',
			title: `Could not remove ${removed.name}`,
			text: 'Check your connection, then try again.',
		})
	} finally {
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: coresQueryKey }),
			queryClient.invalidateQueries({ queryKey: serversQueryKey }),
		])
		removing.value = false
	}
}
</script>

<template>
	<AccountRequired v-if="!auth.user.value" description="Sign in to add a Core" />
	<div v-else-if="error && !data" class="flex items-center justify-between gap-4">
		<span class="text-red">Could not load your Cores</span>
		<Button :disabled="isFetching" @click="refetch()">
			<UpdatedIcon aria-hidden="true" /> Try again
		</Button>
	</div>
	<div v-else-if="data && !core" class="flex items-center justify-between gap-4">
		<span class="text-secondary">No Cores</span>
		<Button @click="openAddCore()"><PlusIcon aria-hidden="true" /> Add Core</Button>
	</div>
	<div v-else-if="core" class="flex flex-col gap-6">
		<ConfirmModal
			ref="confirmRemove"
			:title="`Remove ${core.name}?`"
			description="Its servers stay on it, but you can't manage them until you add it again."
			proceed-label="Remove"
			:proceed-icon="UnlinkIcon"
			@proceed="remove"
		/>
		<div class="flex items-center gap-3">
			<Combobox
				:model-value="core.id"
				:options="coreOptions"
				class="w-max"
				@update:model-value="pick"
			/>
			<span
				v-if="online !== undefined"
				class="flex items-center gap-2"
				:class="online ? 'text-secondary' : 'text-red'"
			>
				<span
					class="size-2 shrink-0 rounded-full"
					:class="online ? 'bg-green' : 'bg-red'"
					aria-hidden="true"
				/>
				{{ online ? 'Online' : 'Offline' }}
			</span>
		</div>

		<template v-if="storage">
			<section class="flex flex-col gap-3" aria-labelledby="core-pool-title">
				<div class="flex items-center justify-between gap-4">
					<h2
						id="core-pool-title"
						class="m-0 flex items-center gap-2 text-lg font-semibold text-contrast"
					>
						Pool storage
						<InfoIcon
							v-tooltip="
								'Lend part of this disk to the pool. Friends\' shared files are stored here, encrypted. Off, your shares stay private.'
							"
							class="size-4 text-secondary"
							aria-hidden="true"
						/>
					</h2>
					<Toggle
						v-model="enabled"
						aria-labelledby="core-pool-title"
						:disabled="change.isPending.value"
					/>
				</div>
				<Admonition v-if="storage.enabled && !storage.service" type="warning">
					Friends can't store files here until Core runs as a service.
				</Admonition>
			</section>

			<section class="flex flex-col gap-3" aria-labelledby="core-storage-title">
				<div class="flex items-baseline justify-between gap-4">
					<h2 id="core-storage-title" class="m-0 text-lg font-semibold text-contrast">Storage</h2>
					<span class="font-semibold tabular-nums text-contrast">
						{{ formatBytes(storage.used, 1) }} of {{ formatBytes(storage.cap, 0) }}
					</span>
				</div>
				<SegmentedBar :segments="segments" :max="storage.used + storage.free" legend />
				<Slider
					v-model="capGiB"
					:min="1"
					:max="Math.max(diskGiB, capGiB)"
					:step="1"
					unit="GiB"
					aria-label="Storage limit in GiB"
					:disabled="change.isPending.value"
					@update:model-value="saveCap('cap', $event)"
				/>
			</section>

			<section class="flex flex-col gap-2.5" aria-labelledby="core-transfer-title">
				<h2
					id="core-transfer-title"
					class="m-0 flex items-center gap-2 text-lg font-semibold text-contrast"
				>
					Daily transfer
					<InfoIcon
						v-tooltip="
							'How much this computer uploads to friends a day. Lower it if your internet has a data cap. Once it is reached, friends download from another copy.'
						"
						class="size-4 text-secondary"
						aria-hidden="true"
					/>
				</h2>
				<Slider
					v-model="transferGiB"
					:min="1"
					:max="Math.max(TRANSFER_TOP_GIB, transferGiB)"
					:step="1"
					unit="GiB"
					aria-label="Daily transfer in GiB"
					:disabled="change.isPending.value"
					@update:model-value="saveCap('egress_cap', $event)"
				/>
			</section>
		</template>

		<div>
			<Button type="colored" color="red" :disabled="removing" @click="confirmRemove?.show()">
				<UnlinkIcon aria-hidden="true" /> Remove Core
			</Button>
		</div>
	</div>
</template>
