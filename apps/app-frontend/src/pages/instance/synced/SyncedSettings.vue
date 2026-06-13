<template>
	<TabView v-model="side" :tabs="tabs">
		<template #server>
			<div class="flex flex-col gap-4">
				<div v-if="has('server:settings')" class="flex flex-col gap-3 rounded-2xl bg-bg-raised p-6">
					<div class="flex flex-col gap-1">
						<span class="text-lg font-bold text-contrast">Server settings</span>
						<span class="text-sm font-medium text-secondary">
							Configure the Copal server: properties, network, installation and more.
						</span>
					</div>
					<div>
						<ButtonStyled color="blue">
							<button @click="openServerSettings?.()">
								<SettingsIcon />
								Open server settings
							</button>
						</ButtonStyled>
					</div>
				</div>
				<NoPermissionCard v-else label="You don't have permission to change server settings." />
			</div>
		</template>
		<template #client>
			<div class="flex flex-col gap-4">
				<div v-if="has('client:settings')" class="flex flex-col gap-3 rounded-2xl bg-bg-raised p-6">
					<div class="flex flex-col gap-1">
						<span class="text-lg font-bold text-contrast">Client settings</span>
						<span class="text-sm font-medium text-secondary">
							Configure how this instance launches: window, Java, memory, and more.
						</span>
					</div>
					<div>
						<ButtonStyled>
							<button @click="openClientSettings?.()">
								<SettingsIcon />
								Open client settings
							</button>
						</ButtonStyled>
					</div>
				</div>
				<NoPermissionCard v-else label="You don't have permission to change client settings." />
			</div>
		</template>
	</TabView>
</template>

<script setup lang="ts">
import { SettingsIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'
import { inject } from 'vue'

import TabView from '@/components/ui/TabView.vue'
import type { GameInstance } from '@/helpers/types'

import NoPermissionCard from './NoPermissionCard.vue'
import { useSyncedPermissions, useSyncedSideTabs } from './use-synced-permissions'
import { SYNCED_OPEN_CLIENT_SETTINGS, SYNCED_OPEN_SERVER_SETTINGS } from './use-synced-side'

defineProps<{
	instance?: GameInstance
	offline?: boolean
	playing?: boolean
}>()

const { side, tabs } = useSyncedSideTabs()
const { has } = useSyncedPermissions()
const openServerSettings = inject(SYNCED_OPEN_SERVER_SETTINGS)
const openClientSettings = inject(SYNCED_OPEN_CLIENT_SETTINGS)
</script>
