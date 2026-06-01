<template>
	<div class="flex w-full flex-col gap-4">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<h2 class="m-0 text-2xl font-semibold text-contrast">Players</h2>
			<ButtonStyled>
				<button :disabled="loading" @click="refresh"><UpdatedIcon /> Refresh</button>
			</ButtonStyled>
		</div>

		<div v-if="error" class="rounded-2xl bg-bg-red p-4 font-semibold text-contrast">
			Failed to load players: {{ error.message }}
		</div>

		<div v-if="!running" class="rounded-2xl bg-surface-3 p-4 font-semibold text-orange">
			Server is offline. Player and whitelist changes require a running server.
		</div>

		<section class="rounded-[20px] bg-surface-3 p-5">
			<h3 class="m-0 mb-3 text-lg font-bold text-contrast">Online ({{ players.online.length }})</h3>
			<p v-if="!players.online.length" class="m-0 text-secondary">No players online.</p>
			<div v-else class="flex flex-col gap-2">
				<div
					v-for="name in players.online"
					:key="name"
					class="flex items-center justify-between gap-3 rounded-xl bg-surface-2 px-3 py-2"
				>
					<span class="font-semibold text-contrast">{{ name }}</span>
					<div class="flex gap-2">
						<button class="player-action" @click="act(() => core.opPlayer(id, name))">Op</button>
						<button class="player-action" @click="act(() => core.kickPlayer(id, name))">
							Kick
						</button>
						<button class="player-action danger" @click="act(() => core.banPlayer(id, name))">
							Ban
						</button>
					</div>
				</div>
			</div>
		</section>

		<section class="rounded-[20px] bg-surface-3 p-5">
			<div class="mb-3 flex items-center justify-between gap-3">
				<h3 class="m-0 text-lg font-bold text-contrast">Whitelist</h3>
				<label class="flex items-center gap-2 text-sm font-semibold text-secondary">
					<input
						type="checkbox"
						:checked="players.whitelist_enabled"
						:disabled="!running"
						@change="toggleWhitelist"
					/>
					Enforced
				</label>
			</div>
			<PlayerAddRow
				placeholder="Add player to whitelist"
				:disabled="!running"
				@add="addWhitelist"
			/>
			<PlayerList
				:names="players.whitelist.map((e) => e.name)"
				:disabled="!running"
				action-label="Remove"
				@action="(name) => act(() => core.removeFromWhitelist(id, name))"
			/>
		</section>

		<section class="rounded-[20px] bg-surface-3 p-5">
			<h3 class="m-0 mb-3 text-lg font-bold text-contrast">Operators</h3>
			<PlayerAddRow placeholder="Op a player" :disabled="!running" @add="addOp" />
			<PlayerList
				:names="players.ops.map((e) => e.name)"
				:disabled="!running"
				action-label="Deop"
				@action="(name) => act(() => core.deopPlayer(id, name))"
			/>
		</section>

		<section class="rounded-[20px] bg-surface-3 p-5">
			<h3 class="m-0 mb-3 text-lg font-bold text-contrast">Banned players</h3>
			<PlayerList
				:names="players.banned_players.map((e) => e.name)"
				:disabled="!running"
				action-label="Pardon"
				empty-label="No banned players."
				@action="(name) => act(() => core.pardonPlayer(id, name))"
			/>
		</section>

		<section class="rounded-[20px] bg-surface-3 p-5">
			<h3 class="m-0 mb-3 text-lg font-bold text-contrast">Banned IPs</h3>
			<PlayerList
				:names="players.banned_ips.map((e) => e.ip)"
				:disabled="!running"
				action-label="Pardon"
				empty-label="No banned IPs."
				@action="(ip) => act(() => core.pardonIp(id, ip))"
			/>
		</section>
	</div>
</template>

<script setup lang="ts">
import type { CorePlayers } from '@amberite/amberite-api'
import { UpdatedIcon } from '@modrinth/assets'
import { ButtonStyled, injectNotificationManager } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

import { injectCoreServerContext } from './server/core-server-instance'
import PlayerAddRow from './server/players/PlayerAddRow.vue'
import PlayerList from './server/players/PlayerList.vue'

const core = useCoreClient()
const { handleError } = injectNotificationManager()
const ctx = injectCoreServerContext()
const id = ctx.instanceId.value

const playersQuery = useQuery({
	queryKey: computed(() => ['core-players', ctx.instanceId.value]),
	queryFn: () => core.listPlayers(ctx.instanceId.value),
	staleTime: 15_000,
})

const empty: CorePlayers = {
	online: [],
	whitelist: [],
	ops: [],
	banned_players: [],
	banned_ips: [],
	whitelist_enabled: false,
	running: false,
}

const players = computed<CorePlayers>(() => playersQuery.data.value ?? empty)
const running = computed(() => players.value.running)
const loading = computed(() => playersQuery.isLoading.value)
const error = computed(() => (playersQuery.error.value as Error | null) ?? null)

async function refresh() {
	await playersQuery.refetch()
}

async function act(fn: () => Promise<unknown>) {
	try {
		await fn()
		await refresh()
	} catch (err) {
		handleError(err as Error)
	}
}

function addWhitelist(name: string) {
	void act(() => core.addToWhitelist(id, name))
}

function addOp(name: string) {
	void act(() => core.opPlayer(id, name))
}

function toggleWhitelist(event: Event) {
	const enabled = (event.target as HTMLInputElement).checked
	void act(() => core.setWhitelistEnabled(id, enabled))
}
</script>

<style scoped>
.player-action {
	border-radius: 0.5rem;
	background: var(--color-button-bg);
	padding: 0.25rem 0.75rem;
	font-weight: 600;
	color: var(--color-contrast);
}
.player-action.danger {
	color: var(--color-red);
}
.player-action:hover {
	filter: brightness(1.1);
}
</style>
