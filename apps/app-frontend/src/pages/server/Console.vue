<template>
	<div class="flex flex-col gap-2 h-full">
		<div class="flex gap-2 shrink-0">
			<button
				class="px-3 py-1 rounded text-sm font-medium transition-colors"
				:class="mode === 'live' ? 'bg-brand text-white' : 'bg-[var(--color-raised-bg)] text-secondary'"
				@click="switchToLive"
			>
				Live
			</button>
			<button
				class="px-3 py-1 rounded text-sm font-medium transition-colors"
				:class="mode === 'logs' ? 'bg-brand text-white' : 'bg-[var(--color-raised-bg)] text-secondary'"
				@click="switchToLogs"
			>
				Logs
			</button>
		</div>

		<template v-if="mode === 'live'">
			<div
				ref="logEl"
				class="flex-1 overflow-y-auto font-mono text-xs bg-[var(--color-raised-bg)] rounded-lg p-3 min-h-0"
			>
				<div v-for="(line, i) in lines" :key="i" class="whitespace-pre-wrap break-words leading-5">
					{{ line }}
				</div>
				<div v-if="lines.length === 0 && connected" class="text-secondary">
					Waiting for output...
				</div>
				<div v-if="!connected" class="text-secondary">
					Not connected — start your server to see console output.
				</div>
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
		</template>

		<template v-else>
			<div class="flex gap-4 flex-1 min-h-0">
				<div class="w-48 shrink-0 overflow-y-auto flex flex-col gap-1">
					<p class="text-xs text-secondary font-medium uppercase mb-1">Logs</p>
					<button
						v-for="f in logFiles"
						:key="f"
						class="text-left px-2 py-1 rounded text-xs truncate hover:bg-[var(--color-raised-bg)] transition-colors"
						:class="selectedFile === f ? 'bg-[var(--color-raised-bg)] font-medium' : ''"
						@click="loadFile('log', f)"
					>
						{{ f }}
					</button>
					<p v-if="crashFiles.length" class="text-xs text-secondary font-medium uppercase mt-2 mb-1">
						Crash Reports
					</p>
					<button
						v-for="f in crashFiles"
						:key="f"
						class="text-left px-2 py-1 rounded text-xs truncate hover:bg-[var(--color-raised-bg)] transition-colors"
						:class="selectedFile === f ? 'bg-[var(--color-raised-bg)] font-medium' : ''"
						@click="loadFile('crash', f)"
					>
						{{ f }}
					</button>
				</div>
				<div
					class="flex-1 overflow-y-auto font-mono text-xs bg-[var(--color-raised-bg)] rounded-lg p-3 min-h-0 whitespace-pre-wrap break-words leading-5"
				>
					<span v-if="fileLoading" class="text-secondary">Loading...</span>
					<span v-else-if="fileContent">{{ fileContent }}</span>
					<span v-else class="text-secondary">Select a log file to view it.</span>
				</div>
			</div>
		</template>
	</div>
</template>

<script setup lang="ts">
import type { CoreWsConnection } from '@amberite/core-client'
import { ButtonStyled, injectCoreClient, injectModrinthServerContext } from '@modrinth/ui'
import { nextTick, onMounted, onUnmounted, ref } from 'vue'

const { serverId } = injectModrinthServerContext()
const client = injectCoreClient()

const mode = ref<'live' | 'logs'>('live')
const lines = ref<string[]>([])
const inputLine = ref('')
const connected = ref(false)
const connectError = ref('')
const logEl = ref<HTMLElement>()
const logFiles = ref<string[]>([])
const crashFiles = ref<string[]>([])
const selectedFile = ref('')
const fileContent = ref('')
const fileLoading = ref(false)

let ws: CoreWsConnection | null = null
let lastStatus = ''

async function connect() {
	connectError.value = ''
	try {
		const ticket = await client.issueWsTicket()
		ws = client.openConsole(serverId, ticket)
		ws.on('open', () => {
			connected.value = true
			connectError.value = ''
		})
		ws.on('log', async (line) => {
			lines.value.push(line)
			await nextTick()
			logEl.value?.scrollTo(0, logEl.value.scrollHeight)
		})
		ws.on('state', (status) => {
			lastStatus = status
		})
		ws.on('close', () => {
			connected.value = false
			switchToLogs()
		})
		ws.on('error', () => {
			connectError.value = 'WebSocket error — is the server running?'
		})
	} catch (e) {
		connectError.value = `Failed to connect: ${e}`
	}
}

function sendLine() {
	if (!ws || !inputLine.value.trim() || !connected.value) return
	ws.send(inputLine.value)
	inputLine.value = ''
}

async function switchToLogs() {
	mode.value = 'logs'
	try {
		const [logs, crashes] = await Promise.all([
			client.listLogs(serverId),
			client.listCrashReports(serverId),
		])
		logFiles.value = logs
		crashFiles.value = crashes
		if (lastStatus === 'crashed' && crashes.length) {
			await loadFile('crash', crashes[0])
		} else if (logs.includes('latest.log')) {
			await loadFile('log', 'latest.log')
		}
	} catch {
		// Files unavailable — leave list empty.
	}
}

function switchToLive() {
	mode.value = 'live'
}

async function loadFile(type: 'log' | 'crash', filename: string) {
	selectedFile.value = filename
	fileLoading.value = true
	fileContent.value = ''
	try {
		fileContent.value =
			type === 'log'
				? await client.readLog(serverId, filename)
				: await client.readCrashReport(serverId, filename)
	} finally {
		fileLoading.value = false
	}
}

onMounted(connect)
onUnmounted(() => ws?.close())
</script>
