<script setup lang="ts">
import { HeartHandshakeIcon, ServerStackIcon, Settings2Icon, UsersIcon } from '@modrinth/assets'
import { ButtonStyled } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { ensureAmberiteSession } from '@/helpers/amberite-auth'
import { getCoreMetadata, listCoreMembers } from '@/helpers/amberite-core'
import { listMyFriendGroups } from '@/helpers/friend-groups'

const router = useRouter()
const core = ref(null)
const groups = ref([])
const members = ref([])

const group = computed(() => groups.value[0])
const online = computed(() => group.value?.core?.status === 'online' || core.value !== null)

onMounted(async () => {
	await ensureAmberiteSession().catch(() => null)
	core.value = await getCoreMetadata().catch(() => null)
	groups.value = await listMyFriendGroups().catch(() => [])
	const coreMembers = await listCoreMembers().catch(() => [])
	members.value = Array.isArray(coreMembers) ? coreMembers : (coreMembers.members ?? [])
})
</script>

<template>
	<div class="rounded-2xl border border-solid border-button-border bg-bg-raised p-4">
		<div class="flex items-center justify-between gap-3">
			<div class="flex items-center gap-3">
				<div class="grid h-10 w-10 place-items-center rounded-2xl bg-brand-highlight text-brand">
					<HeartHandshakeIcon />
				</div>
				<div>
					<h3 class="m-0 text-base font-black text-contrast">
						{{ group?.group?.name ?? core?.name ?? 'Friend group' }}
					</h3>
					<p class="m-0 text-xs text-secondary">{{ online ? 'Core reachable' : 'Setup needed' }}</p>
				</div>
			</div>
			<span class="h-2.5 w-2.5 rounded-full" :class="online ? 'bg-green' : 'bg-orange'"></span>
		</div>

		<div class="mt-4 grid grid-cols-2 gap-2 text-sm">
			<div class="rounded-xl bg-bg p-2"><UsersIcon /> {{ members.length }} members</div>
			<div class="rounded-xl bg-bg p-2"><ServerStackIcon /> {{ group?.role ?? 'owner' }}</div>
		</div>

		<ButtonStyled class="mt-4 w-full" color="brand">
			<button class="w-full" @click="router.push('/core')"><Settings2Icon /> Manage Core</button>
		</ButtonStyled>
	</div>
</template>
