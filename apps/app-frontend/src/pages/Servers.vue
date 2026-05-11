<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { injectModrinthClient, ServersManagePageIndex } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed } from 'vue'

import { config } from '../config'
import { MOCK_SERVER_V0 } from '../helpers/api-mock'

const stripePublishableKey = (config.stripePublishableKey as string) || ''

const client = injectModrinthClient()

// AMBERITE PATCH: Pre-populate the server list query cache so the hosting page
// never shows an empty state (which opens the purchase/subscription modal).
if (import.meta.env.DEV) {
	const queryClient = useQueryClient()
	queryClient.setQueryDefaults(['servers'], { staleTime: Infinity })
	queryClient.setQueryData(['servers'], {
		servers: [MOCK_SERVER_V0],
		pagination: { current_page: 1, page_size: 100, total_pages: 1, total_items: 1 },
	})
}

const { data: products } = useQuery({
	queryKey: ['billing', 'products'],
	queryFn: () => client.labrinth.billing_internal.getProducts(),
})

const resolvedProducts = computed<Labrinth.Billing.Internal.Product[]>(() => products.value ?? [])
</script>

<template>
	<ServersManagePageIndex
		:stripe-publishable-key="stripePublishableKey"
		:products="resolvedProducts"
	/>
</template>
