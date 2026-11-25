<template>
	<div class="relative h-full w-full">
		<div v-if="data" class="flex h-full w-full flex-col">
			<div class="flex flex-col gap-6">
				<div class="flex justify-start gap-16">
					<div class="flex max-w-[500px] grow flex-col gap-6">
						<!-- Server name -->
						<div class="flex flex-col gap-2.5">
							<label for="server-name-field" class="flex flex-col gap-2 self-start">
								<span class="text-lg font-semibold text-contrast">Server name</span>
							</label>
							<div class="flex flex-col gap-2.5">
								<Input
									id="server-name-field"
									v-model="serverName"
									v-tooltip="advancedActionTooltip"
									wrapper-class="w-full"
									:maxlength="48"
									:disabled="!canUseAdvancedSettings"
									@keyup.enter="!serverName && saveGeneral"
								/>
								<div class="text-red font-medium">
									<span v-if="!isValidServerName"> Server name cannot be empty. </span>
								</div>
							</div>
						</div>
					</div>

					<EditServerIcon
						:can-edit="canSetup"
						:permission-denied-message="permissionDeniedMessage"
					/>
				</div>

				<div class="flex flex-col gap-2.5">
					<span class="text-lg font-semibold text-contrast">
						{{ formatMessage(messages.updateChannel) }}
					</span>
					<Chips
						v-model="updateChannel"
						:items="updateChannelOptions"
						:format-label="formatUpdateChannelLabel"
						:capitalize="false"
						:disabled-items="canSetup ? [] : updateChannelOptions"
						:disabled-tooltip="permissionDeniedMessage"
						:aria-label="formatMessage(messages.selectUpdateChannelAriaLabel)"
					/>
					<span>{{ formatUpdateChannelDescription(updateChannel) }}</span>
				</div>

				<!-- preferences -->
				<div
					v-for="(prefConfig, key) in shownPreferences"
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

				<!-- Crash restarts (a Core setting, saved with the rest of the page) -->
				<div v-if="performance" class="flex items-center justify-between gap-2">
					<label for="restart-on-crash" class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Restart if it crashes</span>
						<span>Starts the server again on its own after a crash.</span>
					</label>
					<div v-tooltip="advancedActionTooltip">
						<Toggle
							id="restart-on-crash"
							v-model="restartOnCrash"
							class="flex-none"
							:disabled="!canUseAdvancedSettings"
						/>
					</div>
				</div>

				<!-- Delete server -->
				<div class="flex items-center justify-between gap-2">
					<div class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Delete server</span>
						<span
							>Removes the server with its world, files, and backups. This cannot be undone.</span
						>
					</div>
					<Button
						v-tooltip="
							stopBeforeDeleting ? 'Stop the server before deleting it.' : advancedActionTooltip
						"
						type="colored"
						color="red"
						:disabled="!canUseAdvancedSettings || deleting || stopBeforeDeleting"
						@click="deleteModal?.show()"
					>
						<TrashIcon aria-hidden="true" />
						Delete server
					</Button>
				</div>
				<ConfirmModal
					ref="deleteModal"
					title="Delete this server?"
					:description="`${data.name} will be deleted with its world, files, and backups. This cannot be undone.`"
					proceed-label="Delete server"
					:has-to-type="true"
					:confirmation-text="data.name"
					@proceed="deleteServer"
				/>
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
import type { ReleaseChannel } from '@amberite/contracts/protocol'
import { core as coreRoutes } from '@amberite/contracts/core'
import { TrashIcon } from '@modrinth/assets'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { ConfirmModal, Input, Toggle } from '#ui/components'
import { Button } from '#ui/components/base/buttons'
import Chips from '#ui/components/base/Chips.vue'
import EditServerIcon from '#ui/components/servers/edit-server-icon/EditServerIcon.vue'
import SaveBanner from '#ui/components/servers/SaveBanner.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerPermissions } from '#ui/composables/server-permissions'
import { useServerPreferences } from '#ui/composables/server-preferences'
import {
	injectModrinthServerContext,
	injectNotificationManager,
	injectPageContext,
} from '#ui/providers'

const { addNotification } = injectNotificationManager()
const { server: data, serverId, core, busyReasons, powerState } = injectModrinthServerContext()
// A Core only deletes a stopped server, so say so before the name is typed.
const stopBeforeDeleting = computed(
	() => powerState.value !== 'stopped' && powerState.value !== 'crashed',
)
const { featureFlags } = injectPageContext()
const queryClient = useQueryClient()
const { canUseAdvancedSettings, canSetup, permissionDeniedMessage } = useServerPermissions()
const advancedActionTooltip = computed(() =>
	canUseAdvancedSettings.value ? undefined : permissionDeniedMessage.value,
)

const router = useRouter()
const deleteModal = ref<InstanceType<typeof ConfirmModal>>()
const deleting = ref(false)

async function deleteServer() {
	if (deleting.value) return
	deleting.value = true
	try {
		await core.call(coreRoutes.DeleteServer, { params: { id: serverId } })
		queryClient.removeQueries({ queryKey: ['servers', 'detail', serverId] })
		await queryClient.invalidateQueries({ queryKey: ['servers'] })
		await router.push('/hosting/manage')
	} catch (error) {
		addNotification({
			type: 'error',
			title: 'Could not delete the server',
			text: error instanceof Error ? error.message : undefined,
		})
		deleting.value = false
	}
}

const serverName = ref(data.value?.name)

watch(data, (newData) => {
	if (newData) serverName.value = newData.name
})
const updateChannelOptions: ReleaseChannel[] = ['release', 'beta', 'alpha']

const worldUpdateChannel = computed(() => data.value.update_channel)
const updateChannel = ref<ReleaseChannel>(worldUpdateChannel.value)

watch(worldUpdateChannel, (channel) => {
	updateChannel.value = channel
})

function formatUpdateChannelLabel(channel: ReleaseChannel) {
	switch (channel) {
		case 'release':
			return formatMessage(messages.updateChannelRelease)
		case 'beta':
			return formatMessage(messages.updateChannelBeta)
		case 'alpha':
			return formatMessage(messages.updateChannelAlpha)
	}
}

function formatUpdateChannelDescription(channel: ReleaseChannel) {
	switch (channel) {
		case 'release':
			return formatMessage(messages.updateChannelReleaseDescription)
		case 'beta':
			return formatMessage(messages.updateChannelBetaDescription)
		case 'alpha':
			return formatMessage(messages.updateChannelAlphaDescription)
	}
}

const isUpdating = ref(false)
const isValidServerName = computed(() => (serverName.value?.length ?? 0) > 0)

// Preferences
const { formatMessage } = useVIntl()
const messages = defineMessages({
	updateChannel: {
		id: 'hosting.settings.update-channel',
		defaultMessage: 'Update channel',
	},
	updateChannelRelease: {
		id: 'hosting.settings.update-channel.release',
		defaultMessage: 'Release',
	},
	updateChannelBeta: {
		id: 'hosting.settings.update-channel.beta',
		defaultMessage: 'Beta',
	},
	updateChannelAlpha: {
		id: 'hosting.settings.update-channel.alpha',
		defaultMessage: 'Alpha',
	},
	updateChannelReleaseDescription: {
		id: 'hosting.settings.update-channel.release.description',
		defaultMessage: 'Only release versions will be shown as available updates.',
	},
	updateChannelBetaDescription: {
		id: 'hosting.settings.update-channel.beta.description',
		defaultMessage: 'Release and beta versions will be shown as available updates.',
	},
	updateChannelAlphaDescription: {
		id: 'hosting.settings.update-channel.alpha.description',
		defaultMessage: 'Release, beta, and alpha versions will be shown as available updates.',
	},
	selectUpdateChannelAriaLabel: {
		id: 'hosting.settings.update-channel.select',
		defaultMessage: 'Select update channel',
	},
	environmentChangesTitle: {
		id: 'hosting.settings.warn-environment-changes.title',
		defaultMessage: 'Warn about incompatible or required content',
	},
	environmentChangesDescription: {
		id: 'hosting.settings.warn-environment-changes.description',
		defaultMessage:
			'Ask for confirmation before enabling content on an unsupported side or disabling it on a required side.',
	},
})
const preferences = computed(() => ({
	warnOnIncompatibleContent: {
		displayName: formatMessage(messages.environmentChangesTitle),
		description: formatMessage(messages.environmentChangesDescription),
		implemented: true,
	},
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
}))

type PreferenceKeys = keyof typeof preferences.value
const preferenceKeys = Object.keys(preferences.value) as PreferenceKeys[]

// A Core server has no subdomain label to hide, and reports no RAM usage to show.
const shownPreferences = computed(() => {
	const { hideSubdomainLabel: _, ramAsNumber: __, ...rest } = preferences.value
	return rest
})

type UserPreferences = {
	[K in PreferenceKeys]: boolean
}

const userPreferences = useServerPreferences(serverId)

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

const { data: performance } = useQuery({
	queryKey: ['servers', 'performance', serverId],
	queryFn: () => core.call(coreRoutes.GetPerformance, { params: { id: serverId } }),
})
const restartOnCrash = ref(true)
watch(
	performance,
	(saved) => {
		if (saved) restartOnCrash.value = saved.restarts.crash
	},
	{ immediate: true },
)
const hasRestartChanges = computed(
	() => !!performance.value && restartOnCrash.value !== performance.value.restarts.crash,
)

// Unsaved changes tracking (API fields + preferences)
const hasServerSettingsChanges = computed(
	() => (serverName.value && serverName.value !== data.value?.name) || hasRestartChanges.value,
)
const hasPreferenceChanges = computed(() =>
	preferenceKeys.some((key) => newUserPreferences.value[key] !== userPreferences.value[key]),
)
const hasUpdateChannelChanges = computed(() => updateChannel.value !== worldUpdateChannel.value)
const hasUnsavedChanges = computed(
	() =>
		hasServerSettingsChanges.value || hasPreferenceChanges.value || hasUpdateChannelChanges.value,
)

const saveGeneral = async () => {
	if (!isValidServerName.value) return
	if (hasServerSettingsChanges.value && !canUseAdvancedSettings.value) return
	if (hasUpdateChannelChanges.value && !canSetup.value) return

	try {
		isUpdating.value = true
		if (serverName.value !== data.value?.name || hasUpdateChannelChanges.value) {
			await core.call(coreRoutes.ChangeServer, {
				params: { id: serverId },
				body: {
					name: serverName.value !== data.value?.name ? (serverName.value ?? '') : null,
					update_channel: hasUpdateChannelChanges.value ? updateChannel.value : null,
				},
			})
		}
		if (hasRestartChanges.value && performance.value) {
			const saved = await core.call(coreRoutes.SetRestarts, {
				params: { id: serverId },
				body: { ...performance.value.restarts, crash: restartOnCrash.value },
			})
			queryClient.setQueryData(['servers', 'performance', serverId], saved)
		}
		// Save preferences to localStorage
		for (const key of preferenceKeys) userPreferences.value[key] = newUserPreferences.value[key]

		await queryClient.invalidateQueries({
			queryKey: ['servers', 'detail', serverId],
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
	updateChannel.value = worldUpdateChannel.value
	if (performance.value) restartOnCrash.value = performance.value.restarts.crash
}
</script>
