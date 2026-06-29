<template>
	<div class="relative h-full w-full">
		<div v-if="data" class="flex h-full w-full flex-col">
			<div class="flex flex-col gap-6">
				<div class="flex justify-start gap-16">
					<div class="flex max-w-[500px] grow flex-col gap-6">
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
								<span>This name is only visible on this machine.</span>
								<div class="text-red font-medium">
									<span v-if="!isValidServerName"> Server name cannot be empty. </span>
								</div>
							</div>
						</div>

						<!-- Hostname (subdomain persistence deferred — UI kept for later) -->
						<div v-if="SUBDOMAIN_ENABLED" class="flex flex-col gap-2.5">
							<label for="server-subdomain" class="flex flex-col gap-2.5">
								<span class="text-lg font-semibold text-contrast">Hostname</span>
								<div
									class="flex w-full overflow-hidden rounded-xl bg-button-bg px-3 [box-shadow:var(--shadow-inset-sm)] transition-[box-shadow] duration-100 ease-in-out focus-within:[box-shadow:0_0_0_0.25rem_var(--color-brand-shadow)]"
								>
									<div class="relative inline-flex min-h-9 items-center">
										<span
											class="pointer-events-none invisible whitespace-pre px-px text-base font-medium"
											aria-hidden="true"
											>{{ serverSubdomain || 'Enter subdomain...' }}</span
										>
										<input
											id="server-subdomain"
											:value="serverSubdomain"
											placeholder="Enter subdomain..."
											:maxlength="32"
											class="absolute left-px inset-0 bg-transparent !p-0 text-base font-medium text-primary !shadow-none transition-colors placeholder:text-secondary focus:text-contrast"
											autocomplete="off"
											@input="serverSubdomain = ($event.target as HTMLInputElement).value"
											@keyup.enter="saveGeneral"
										/>
									</div>
									<div
										class="flex min-h-9 shrink-0 select-none items-center py-2 pr-4 font-medium opacity-50 [filter:grayscale(50%)]"
										:class="!serverSubdomain ? '!ml-auto' : ''"
									>
										.amberite.dev
									</div>
								</div>
							</label>
							<span>Your friends can connect to your server using this address.</span>
							<div v-if="!isValidSubdomain" class="text-red font-medium">
								<span v-if="!isValidLengthSubdomain">
									Subdomain must be at least 5 characters long.
								</span>
								<span v-if="!isValidCharsSubdomain">
									Subdomain can only contain alphanumeric characters and dashes.
								</span>
							</div>
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
								<div
									v-else-if="property.type === 'specs'"
									class="flex flex-col items-end text-right text-sm leading-5 break-words"
								>
									<span v-for="line in property.lines" :key="line">{{ line }}</span>
								</div>
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
import {
	CopyCode,
	injectModrinthServerContext,
	injectNotificationManager,
	injectPageContext,
	StyledInput,
	Toggle,
} from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'
import { edit } from '@/helpers/profile'

import { injectCoreServerContext } from '../../core-server-instance'
import SaveBanner from '../SaveBanner.vue'

/**
 * Forked verbatim from the hosting server-settings General page. Backend swaps:
 * server rename now hits Copal (`renameInstance`); billing/specs and
 * subdomain persistence are deferred (subdomain UI gated behind SUBDOMAIN_ENABLED).
 */
const SUBDOMAIN_ENABLED = false

const route = useRoute()
const { addNotification } = injectNotificationManager()
const core = useCoreClient()
const { server: data, serverId, busyReasons } = injectModrinthServerContext()
const { refreshServer } = injectCoreServerContext()
const { featureFlags } = injectPageContext()

const serverName = ref(data.value?.name)
const serverSubdomain = ref(data.value?.net?.domain ?? '')

watch(data, (newData) => {
	if (newData) {
		serverName.value = newData.name
		serverSubdomain.value = newData.net?.domain ?? ''
	}
})
const isValidLengthSubdomain = computed(() => serverSubdomain.value.length >= 5)
const isValidCharsSubdomain = computed(
	() => !serverSubdomain.value || /^[a-zA-Z0-9-]+$/.test(serverSubdomain.value),
)
const isValidSubdomain = computed(() => isValidLengthSubdomain.value && isValidCharsSubdomain.value)

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

const serverHostname = computed(() =>
	SUBDOMAIN_ENABLED && serverSubdomain.value ? `${serverSubdomain.value}.amberite.dev` : 'Unknown',
)

type InfoProperty =
	| {
			name: string
			value: string
			type: 'copy'
	  }
	| {
			name: string
			value: string
			type: 'text'
	  }
	| {
			name: string
			value: string
			type: 'specs'
			lines: string[]
	  }

// Info properties
const infoProperties = computed<InfoProperty[]>(() => [
	{ name: 'Server path', value: serverId ?? 'Unknown', type: 'copy' },
	{ name: 'Address', value: data.value?.net?.domain ?? 'Unknown', type: 'copy' },
	{ name: 'Hostname', value: serverHostname.value, type: 'copy' },
])

// Unsaved changes tracking (API fields + preferences)
const hasUnsavedChanges = computed(
	() =>
		(serverName.value && serverName.value !== data.value?.name) ||
		(SUBDOMAIN_ENABLED && serverSubdomain.value !== data.value?.net?.domain) ||
		JSON.stringify(newUserPreferences.value) !== JSON.stringify(userPreferences.value),
)

const saveGeneral = async () => {
	if (!isValidServerName.value || (SUBDOMAIN_ENABLED && !isValidSubdomain.value)) return

	try {
		isUpdating.value = true
		if (serverName.value !== data.value?.name) {
			await core.renameInstance(serverId, serverName.value ?? '')
			const profilePath = route.params.id as string
			if (profilePath) {
				await edit(profilePath, { name: serverName.value ?? '' }).catch(() => {})
			}
		}

		// Save preferences to localStorage
		userPreferences.value = { ...newUserPreferences.value }

		await refreshServer()
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
	serverSubdomain.value = data.value?.net?.domain ?? ''
	newUserPreferences.value = { ...userPreferences.value }
}
</script>
