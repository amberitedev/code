<template>
	<div class="relative h-full w-full">
		<div v-if="data" class="flex h-full w-full flex-col">
			<div class="flex flex-col gap-6">
				<!-- Server name -->
				<div class="flex flex-col gap-2.5">
					<label for="server-name-field" class="flex flex-col gap-2">
						<span class="text-lg font-semibold text-contrast">Server name</span>
					</label>
					<div class="flex flex-col gap-2.5">
						<StyledInput
							id="server-name-field"
							v-model="serverName"
							wrapper-class="w-full"
							:maxlength="48"
							@keyup.enter="!serverName && saveGeneral"
						/>
						<div class="text-red font-medium">
							<span v-if="!isValidServerName"> Server name cannot be empty. </span>
						</div>
					</div>
				</div>

				<!-- preferences -->
				<div
					v-for="(prefConfig, key) in preferences"
					:key="key"
					class="flex items-center justify-between gap-2"
				>
					<label :for="`pref-${key}`" class="flex flex-col gap-1">
						<div class="flex flex-row items-center gap-2">
							<span class="text-lg font-semibold text-contrast">{{ prefConfig.displayName }}</span>
							<div
								v-if="!prefConfig.implemented"
								class="hidden items-center gap-1 rounded-full bg-surface-2 p-1 px-1.5 text-xs font-semibold sm:flex"
							>
								Coming Soon
							</div>
						</div>
						<span>{{ prefConfig.description }}</span>
					</label>
					<div v-tooltip="getPreferenceTooltip(key)">
						<Toggle
							:id="`pref-${key}`"
							:model-value="getPreferenceValue(key)"
							class="flex-none"
							:disabled="!prefConfig.implemented || isPreferenceForcedByFeatureFlag(key)"
							@update:model-value="(value) => setPreferenceValue(key, !!value)"
						/>
					</div>
				</div>

				<!-- Info -->
				<div class="flex flex-col gap-2.5 pb-10">
					<div class="text-lg m-0 font-semibold text-contrast">Info</div>
					<div class="flex flex-col gap-2.5 rounded-xl bg-surface-2 p-4">
						<div
							v-for="property in infoProperties"
							:key="property.name"
							class="flex items-start justify-between gap-4"
						>
						<template v-if="property.value !== 'Unknown'">
							<span class="mt-1">{{ property.name }}</span>
							<CopyCode v-if="property.type === 'copy'" :text="property.value" />
							<span v-else class="text-right text-sm break-words">{{ property.value }}</span>
						</template>
						</div>
					</div>
				</div>
			</div>
		</div>
		<div v-else />
		<SaveBanner
			:is-visible="(!!hasUnsavedChanges && !!isValidServerName) || isUpdating"
			:server-id="serverId"
			:is-updating="isUpdating || busyReasons.length > 0"
			:save="saveGeneral"
			:reset="resetGeneral"
		/>
	</div>
</template>

<script setup lang="ts">
// general.vue — server name edit + local preferences. Billing and subdomain removed (Core has neither).
// saveGeneral: renames via coreClient.renameInstance, then saves preferences to localStorage.
// infoProperties: Server ID + Port only (from server context net.port).
import { useQueryClient } from '@tanstack/vue-query'
import { useStorage } from '@vueuse/core'
import { computed, ref, watch } from 'vue'

import { CopyCode, StyledInput, Toggle } from '#ui/components'
import SaveBanner from '#ui/components/servers/SaveBanner.vue'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
	injectPageContext,
} from '#ui/providers'

const { addNotification } = injectNotificationManager()
const coreClient = injectCoreClient()
const { server: data, serverId, busyReasons } = injectModrinthServerContext()
const { featureFlags } = injectPageContext()
const queryClient = useQueryClient()

const serverName = ref(data.value?.name)

watch(data, (newData) => {
	if (newData) {
		serverName.value = newData.name
	}
})

const isUpdating = ref(false)
const isValidServerName = computed(() => (serverName.value?.length ?? 0) > 0)

watch(serverName, (newValue, oldValue) => {
	if (!(newValue?.length ?? 0)) {
		serverName.value = oldValue
	}
})

// Preferences
const preferences = {
	hideSubdomainLabel: {
		displayName: 'Hide subdomain label',
		description: 'When enabled, the subdomain label will be hidden from the server header.',
		implemented: true,
	},
	// autoRestart: {
	// 	displayName: 'Auto restarts',
	// 	description: 'Automatically restart the server if it crashes.',
	// 	implemented: false,
	// },
	ramAsNumber: {
		displayName: 'RAM as bytes',
		description: 'Show RAM usage in bytes instead of a percentage.',
		implemented: true,
	},
} as const

type PreferenceKeys = keyof typeof preferences

type UserPreferences = {
	[K in PreferenceKeys]: boolean
}

const defaultPreferences: UserPreferences = {
	hideSubdomainLabel: false,
	// autoRestart: false,
	ramAsNumber: false,
}

const userPreferences = useStorage<UserPreferences>(
	`pyro-server-${serverId}-preferences`,
	defaultPreferences,
)

const newUserPreferences = ref<UserPreferences>(JSON.parse(JSON.stringify(userPreferences.value)))

const isRamAsBytesForcedByFeatureFlag = computed(
	() => featureFlags?.serverRamAsBytesAlwaysOn?.value ?? false,
)

const isPreferenceForcedByFeatureFlag = (key: string) =>
	key === 'ramAsNumber' && isRamAsBytesForcedByFeatureFlag.value

const getPreferenceTooltip = (key: string) =>
	isPreferenceForcedByFeatureFlag(key)
		? 'Feature flag enabled to always show RAM as bytes.'
		: undefined

const getPreferenceValue = (key: string) =>
	isPreferenceForcedByFeatureFlag(key) ? true : newUserPreferences.value[key as PreferenceKeys]

const setPreferenceValue = (key: string, value: boolean) => {
	if (isPreferenceForcedByFeatureFlag(key)) {
		return
	}
	newUserPreferences.value[key as PreferenceKeys] = value
}

type InfoProperty =
	| { name: string; value: string; type: 'copy' }
	| { name: string; value: string; type: 'text' }

// Info properties
const infoProperties = computed<InfoProperty[]>(() => [
	{ name: 'Server ID', value: serverId ?? 'Unknown', type: 'copy' },
	{
		name: 'Port',
		value: data.value?.net?.port ? String(data.value.net.port) : 'Unknown',
		type: 'copy',
	},
])

// Unsaved changes tracking (API fields + preferences)
const hasUnsavedChanges = computed(
	() =>
		(serverName.value && serverName.value !== data.value?.name) ||
		JSON.stringify(newUserPreferences.value) !== JSON.stringify(userPreferences.value),
)

const saveGeneral = async () => {
	if (!isValidServerName.value) return

	try {
		isUpdating.value = true
		if (serverName.value !== data.value?.name) {
			await coreClient.renameInstance(serverId, serverName.value ?? '')
		}

		// Save preferences to localStorage
		userPreferences.value = { ...newUserPreferences.value }

		await queryClient.invalidateQueries({
			queryKey: ['servers'],
		})
		addNotification({
			type: 'success',
			title: 'Server settings updated',
			text: 'Your server settings were successfully changed.',
		})
	} catch (error) {
		console.error(error)
		addNotification({
			type: 'error',
			title: 'Failed to update server settings',
			text: 'An error occurred while attempting to update your server settings.',
		})
	} finally {
		isUpdating.value = false
	}
}

const resetGeneral = () => {
	serverName.value = data.value?.name || ''
	newUserPreferences.value = { ...userPreferences.value }
}
</script>
