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
				<div v-for="(line, i) in consoleLines" :key="i" class="whitespace-pre-wrap break-words leading-5">
					{{ line.text }}
				</div>
				<div v-if="consoleLines.length === 0 && isConnected" class="text-secondary">
					Waiting for output...
				</div>
				<div v-if="!isConnected" class="text-secondary">
					Not connected — start your server to see console output.
				</div>
			</div>
			<div class="flex gap-2 shrink-0">
				<input
					v-model="inputLine"
					class="flex-1 bg-[var(--color-raised-bg)] rounded px-3 py-1.5 font-mono text-sm outline-none"
					placeholder="Enter command..."
					:disabled="!isConnected"
					@keydown.enter="sendLine"
				/>
				<ButtonStyled :disabled="!isConnected">
					<button @click="sendLine">Send</button>
				</ButtonStyled>
			</div>
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
import { ButtonStyled, injectCoreClient, injectModrinthServerContext, useModrinthServersConsole } from '@modrinth/ui'
import { nextTick, ref, watch } from 'vue'

const { serverId } = injectModrinthServerContext()
const client = injectCoreClient()
const console$ = useModrinthServersConsole()

const mode = ref<'live' | 'logs'>('live')
const inputLine = ref('')
const logEl = ref<HTMLElement>()
const logFiles = ref<string[]>([])
const crashFiles = ref<string[]>([])
const selectedFile = ref('')
const fileContent = ref('')
const fileLoading = ref(false)

const consoleLines = console$.output
const isConnected = injectModrinthServerContext().isConnected

watch(consoleLines, async () => {
	if (mode.value !== 'live') return
	await nextTick()
	logEl.value?.scrollTo(0, logEl.value.scrollHeight)
}, { deep: true })

function sendLine() {
	if (!inputLine.value.trim() || !isConnected.value) return
	client.sendCommand(serverId, inputLine.value)
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
		if (logs.includes('latest.log')) {
			await loadFile('log', 'latest.log')
		} else if (crashes.length) {
			await loadFile('crash', crashes[0])
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
</script>
