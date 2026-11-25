<template>
	<div
		class="transition-all"
		:class="{
			pressable: !isDisabled,
			hoverable: !isDisabled,
			'cursor-pointer': !isDisabled,
		}"
		:role="!isDisabled ? 'link' : undefined"
		:tabindex="!isDisabled ? 0 : undefined"
		@click="navigateToServer"
		@pointerenter="prefetchServer"
		@keydown.enter.self="navigateToServer"
		@keydown.space.prevent.self="navigateToServer"
	>
		<div
			class="flex flex-row items-center overflow-x-hidden rounded-2xl border-[1px] border-solid border-surface-4 bg-bg-raised p-4 transition-all duration-150"
			:class="{
				'!rounded-b-none border-b-0': isProvisioning,
				'bg-surface-2': isDisabled,
			}"
			data-pyro-server-listing
			:data-pyro-server-listing-id="server_id"
		>
			<div
				v-if="isProvisioning"
				class="flex size-16 items-center justify-center rounded-xl border-[1px] border-solid border-button-border bg-button-bg shadow-sm"
			>
				<ServerIcon
					:image="image"
					:config="iconConfig"
					:disabled="isDisabled"
					class="!rounded-xl"
				/>
				<SpinnerIcon
					class="size-8 animate-spin absolute text-contrast"
					:class="{ 'opacity-50': isDisabled }"
				/>
			</div>
			<ServerIcon v-else :image="image" :config="iconConfig" :disabled="isDisabled" />
			<div class="ml-4 flex flex-col gap-1.5">
				<div class="flex flex-row items-center gap-2.5">
					<h2 class="m-0 text-xl font-bold text-contrast" :class="{ 'opacity-50': isDisabled }">
						{{ name }}
					</h2>
					<div
						v-if="owner"
						v-tooltip="formatMessage(messages.ownerTooltip, { username: owner.username })"
						class="flex min-w-0 items-center gap-1 rounded-full bg-surface-4 px-2 pr-2.5 py-1 text-sm font-medium text-primary !border !border-surface-5 border-solid"
						:class="{ 'opacity-50': isDisabled }"
					>
						<Avatar
							:src="owner.avatarUrl"
							:alt="formatMessage(messages.ownerAvatarAlt, { username: owner.username })"
							:tint-by="owner.username"
							size="1.25rem"
							circle
							no-shadow
						/>
						<span class="max-w-32 truncate">{{ owner.username }}</span>
					</div>
					<div
						v-if="isConfiguring"
						class="flex min-w-0 items-center gap-2 truncate text-sm font-medium text-brand rounded-full bg-brand-highlight border border-solid border-brand px-2.5 h-[28px]"
					>
						<SparklesIcon class="size-5 shrink-0 font-semibold" />
						{{ formatMessage(messages.newLabel) }}
					</div>
				</div>

				<div
					v-if="projectData?.title"
					class="m-0 flex flex-row items-center gap-2 text-sm font-medium"
					:class="{ 'opacity-50': isDisabled }"
				>
					<Avatar
						:src="iconUrl"
						no-shadow
						style="min-height: 20px; min-width: 20px; height: 20px; width: 20px"
						:alt="formatMessage(messages.serverIconAlt)"
					/>
					{{ formatMessage(messages.usingProjectLabel, { projectTitle: projectData?.title }) }}
				</div>

				<ServerInfoLabels
					:server-data="
						isConfiguring
							? { net }
							: {
									game,
									mc_version,
									loader,
									loader_version,
									net,
									online,
									players: playerCount
										? { current: playerCount.current, max: playerCount.max }
										: undefined,
								}
					"
					:server-id="server_id"
					:show-game-label="showGameLabel"
					:show-loader-label="showLoaderLabel"
					:show-player-count="showPlayerCount"
					:class="{ 'opacity-50': isDisabled }"
					:linked="false"
					class="flex w-full flex-row flex-wrap items-center gap-2 text-primary *:hidden sm:flex-row sm:*:flex"
				/>
			</div>
		</div>

		<div v-if="isProvisioning" class="server-listing-notice">
			{{ formatMessage(messages.provisioningNotice) }}
		</div>
	</div>
</template>

<script setup lang="ts">
import type { Server } from '@amberite/contracts/core'
import { SparklesIcon, SpinnerIcon } from '@modrinth/assets'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { serverDetailQueryOptions } from '../../composables/server-queries'
import { useServerIcon } from '../../composables/use-server-icon'
import { injectModrinthClient } from '../../providers/api-client'
import Avatar from '../base/Avatar.vue'
import type { ServerListingOwner } from './access/types'
import ServerIcon from './icons/ServerIcon.vue'
import ServerInfoLabels from './labels/ServerInfoLabels.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	newLabel: {
		id: 'servers.listing.new-label',
		defaultMessage: 'New',
	},
	serverIconAlt: {
		id: 'servers.listing.server-icon-alt',
		defaultMessage: 'Server icon',
	},
	usingProjectLabel: {
		id: 'servers.listing.using-project-label',
		defaultMessage: 'Using {projectTitle}',
	},
	ownerTooltip: {
		id: 'servers.listing.owner-tooltip',
		defaultMessage: 'Owned by {username}',
	},
	ownerAvatarAlt: {
		id: 'servers.listing.owner-avatar-alt',
		defaultMessage: "{username}'s avatar",
	},
	provisioningNotice: {
		id: 'servers.listing.notice.provisioning',
		defaultMessage: 'Please wait while we set up your server. This can take up to 10 minutes.',
	},
})

type ServerListingProps = Pick<Server, 'server_id' | 'name' | 'status'> &
	Partial<
		Pick<Server, 'mc_version' | 'loader' | 'loader_version' | 'net' | 'upstream' | 'icon' | 'flows'>
	> & {
		game?: string
		online?: boolean
		playerCount?: {
			current?: number
			max?: number
		}
		isProvisioning?: boolean
		owner?: ServerListingOwner
	}

const props = defineProps<ServerListingProps>()
const router = useRouter()

const client = injectModrinthClient()
const { labrinth } = client
const queryClient = useQueryClient()

/** Warms the request the server panel opens with while the pointer is over the row. */
function prefetchServer() {
	if (isDisabled.value) return
	void queryClient.prefetchQuery(serverDetailQueryOptions(client, props.server_id))
}

const isConfiguring = computed(() => props.flows?.intro)
const isDisabled = computed(() => !!props.isProvisioning)

const showGameLabel = computed(() => !!props.game && !isConfiguring.value)
const showLoaderLabel = computed(() => !!props.loader && !isConfiguring.value)
const showPlayerCount = computed(() => !!props.playerCount && !isConfiguring.value)

const { data: projectData } = useQuery({
	queryKey: ['project', props.upstream?.project_id] as const,
	queryFn: async () => {
		if (!props.upstream?.project_id) return null
		return await labrinth.projects_v2.get(props.upstream.project_id)
	},
	enabled: computed(() => !!props.upstream?.project_id),
})

const iconUrl = computed(() => projectData.value?.icon_url)

const { config: iconConfig, image } = useServerIcon(() => props.server_id, {
	icon: () => props.icon,
	enabled: computed(() => props.status === 'available'),
})

function navigateToServer(event: MouseEvent | KeyboardEvent) {
	if (isDisabled.value) return

	const target = event.target
	if (
		target instanceof HTMLElement &&
		target.closest('[data-subdomain-label], [data-server-listing-button]')
	) {
		return
	}

	router.push(`/hosting/manage/${props.server_id}`)
}
</script>

<style scoped>
.server-listing-notice {
	@apply relative flex w-full rounded-b-2xl border-[1px] border-solid p-4 flex-col gap-4 border-surface-4 bg-bg-raised text-primary;
}

.hoverable:hover:not(:has([data-subdomain-label]:hover, [data-server-listing-button]:hover)) {
	filter: brightness(1.2);
}

.pressable:active:not(:has([data-subdomain-label]:active, [data-server-listing-button]:active)) {
	transform: scale(0.985);
}
</style>
