<template>
	<BrowsePageLayout />
</template>

<script setup lang="ts">
import { BrowsePageLayout } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, inject } from 'vue'
import { useRouter } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey } from './server/core-server-instance'
import { useCoreBrowseManager } from './server/use-core-browse-manager'

const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const router = useRouter()
const core = useCoreClient()

const modsQuery = useQuery({
	queryKey: computed(() => ['core-mods', ctx.instanceId.value]),
	queryFn: () => core.listMods(ctx.instanceId.value),
	staleTime: 30_000,
})

const installedIds = computed(
	() =>
		new Set(
			(modsQuery.data.value ?? [])
				.map((m) => m.modrinth_project_id)
				.filter((id): id is string => typeof id === 'string' && id.length > 0),
		),
)

useCoreBrowseManager({
	installedProjectIds: installedIds,
	onBack: () => {
		void router.push(`/instance/${encodeURIComponent(ctx.instanceId.value)}/content`)
	},
	onInstalled: async () => {
		await modsQuery.refetch()
	},
})
</script>
