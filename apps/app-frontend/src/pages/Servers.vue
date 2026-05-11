<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/core-client'
import { CoreApiClient } from '@amberite/core-client'
import type { Archon } from '@modrinth/api-client'
import { ServersManagePageIndex } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { core_get_url } from '@/helpers/core'

import { config } from '../config'

const stripePublishableKey = (config.stripePublishableKey as string) || ''

function mapCoreInstanceToServer(inst: CoreInstanceSummary): Archon.Servers.v0.Server {
	return {
		server_id: inst.id,
		name: inst.name,
		owner_id: '',
		net: { ip: '127.0.0.1', port: inst.port, domain: null },
		game: 'java',
		backup_quota: 999,
		used_backup_quota: 0,
		status: 'available',
		suspension_reason: null,
		loader: inst.loader as Archon.Servers.v0.Loader,
		loader_version: inst.loader_version ?? '',
		mc_version: inst.game_version,
		upstream: null,
		sftp_username: '',
		sftp_password: '',
		sftp_host: '',
		sftp_port: 22,
		datacenter: 'local',
		notices: [],
		node: { token: '', instance: '' },
		flows: { intro: false },
		is_medal: false,
	} as Archon.Servers.v0.Server
}

// Pre-populate billing and region query caches so the billing queries fired by
// index.vue's setup never hit the network. staleTime: Infinity keeps them frozen.
const queryClient = useQueryClient()
const neverStale = { staleTime: Infinity }
const emptyPagination = { current_page: 1, page_size: 100, total_pages: 1, total_items: 0 }
queryClient.setQueryDefaults(['billing', 'customer'], neverStale)
queryClient.setQueryDefaults(['billing', 'payment-methods'], neverStale)
queryClient.setQueryDefaults(['billing', 'subscriptions'], neverStale)
queryClient.setQueryDefaults(['billing', 'payments'], neverStale)
queryClient.setQueryDefaults(['billing', 'products'], neverStale)
queryClient.setQueryDefaults(['servers', 'regions'], neverStale)
queryClient.setQueryDefaults(['servers', 'v1'], neverStale)
queryClient.setQueryData(['billing', 'customer'], null)
queryClient.setQueryData(['billing', 'payment-methods'], [])
queryClient.setQueryData(['billing', 'subscriptions'], [])
queryClient.setQueryData(['billing', 'payments'], [])
queryClient.setQueryData(['billing', 'products'], [])
queryClient.setQueryData(['servers', 'regions'], [])
queryClient.setQueryData(['servers', 'v1'], { servers: [], pagination: emptyPagination })

// Register the ['servers'] query with a Core queryFn before ServersManagePageIndex
// mounts. Vue renders parents first, so this queryFn wins: Archon's queryFn never runs.
useQuery({
	queryKey: ['servers'],
	staleTime: Infinity,
	queryFn: async () => {
		const baseUrl = await core_get_url()
		const coreClient = new CoreApiClient(baseUrl)
		const instances = await coreClient.listInstances()
		return {
			servers: instances.map(mapCoreInstanceToServer),
			pagination: {
				current_page: 1,
				page_size: 100,
				total_pages: 1,
				total_items: instances.length,
			},
		}
	},
})
</script>

<template>
	<ServersManagePageIndex :stripe-publishable-key="stripePublishableKey" :products="[]" />
</template>
