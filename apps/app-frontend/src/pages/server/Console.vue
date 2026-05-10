<template>
	<div class="flex flex-col gap-2 h-full">
		<div
			ref="logEl"
			class="flex-1 overflow-y-auto font-mono text-xs bg-[var(--color-raised-bg)] rounded-lg p-3 min-h-0"
		>
			<div v-for="(line, i) in lines" :key="i" class="whitespace-pre-wrap break-words leading-5">
				{{ line }}
			</div>
			<div v-if="lines.length === 0" class="text-secondary">Waiting for console output...</div>
		</div>

		<div class="flex gap-2 shrink-0">
			<input
				v-model="inputLine"
				class="flex-1 bg-[var(--color-raised-bg)] rounded px-3 py-1.5 font-mono text-sm outline-none"
				placeholder="Enter command..."
				:disabled="!connected"
				@keydown.enter="sendLine"
			/>
			<ButtonStyled :disabled="!connected">
				<button @click="sendLine">Send</button>
			</ButtonStyled>
		</div>

		<p v-if="connectError" class="text-red-400 text-sm shrink-0">{{ connectError }}</p>
	</div>
</template>

<script setup lang="ts">
import { ButtonStyled } from '@modrinth/ui'
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { core_get_url, core_issue_ws_token } from '@/helpers/core'
import type { CoreInstanceDetail } from '@/helpers/core'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
	instance: GameInstance
	coreInstance: CoreInstanceDetail
}>()

const lines = ref<string[]>([])
const inputLine = ref('')
const connected = ref(false)
const connectError = ref('')
const logEl = ref<HTMLElement>()

let ws: WebSocket | null = null

async function connect() {
	connectError.value = ''
	try {
		const baseUrl = await core_get_url()
		const ticket = await core_issue_ws_token()
		const wsBase = baseUrl.replace(/^http/, 'ws')
		ws = new WebSocket(`${wsBase}/instances/${props.coreInstance.id}/console?ticket=${ticket}`)

		ws.onopen = () => {
			connected.value = true
			connectError.value = ''
		}
		ws.onmessage = async (e) => {
			lines.value.push(e.data as string)
			await nextTick()
			logEl.value?.scrollTo(0, logEl.value.scrollHeight)
		}
		ws.onerror = () => {
			connectError.value = 'WebSocket error — is the server running?'
			connected.value = false
		}
		ws.onclose = () => {
			connected.value = false
		}
	} catch (e) {
		connectError.value = `Failed to connect: ${e}`
	}
}

function sendLine() {
	if (!ws || !inputLine.value.trim() || !connected.value) return
	ws.send(inputLine.value)
	inputLine.value = ''
}

onMounted(connect)
onUnmounted(() => ws?.close())
</script>
