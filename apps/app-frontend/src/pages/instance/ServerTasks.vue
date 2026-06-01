<template>
	<div class="flex w-full flex-col gap-4">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<h2 class="m-0 text-2xl font-semibold text-contrast">Scheduled Tasks</h2>
			<ButtonStyled color="brand">
				<button :disabled="creating" @click="showCreate = true">
					<PlusIcon />
					{{ creating ? 'Creating...' : 'Add task' }}
				</button>
			</ButtonStyled>
		</div>

		<div v-if="error" class="rounded-2xl bg-bg-red p-4 font-semibold text-contrast">
			Failed to load tasks: {{ error.message }}
		</div>

		<div v-else-if="loading" class="rounded-[20px] bg-surface-3 p-6 font-semibold text-secondary">
			Loading tasks...
		</div>

		<div
			v-else-if="tasks.length === 0"
			class="mt-6 flex flex-col items-center justify-center gap-2 rounded-[20px] bg-surface-3 p-12 text-center text-secondary"
		>
			<ClockIcon class="size-16" />
			<h3 class="m-0 text-2xl font-bold text-contrast">No scheduled tasks</h3>
			<p class="m-0">Automate restarts, backups, announcements and commands with cron schedules.</p>
		</div>

		<div v-else class="flex flex-col gap-3">
			<div
				v-for="task in tasks"
				:key="task.id"
				class="flex flex-col gap-2 rounded-2xl bg-surface-2 p-4"
			>
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<Badge
							:type="task.enabled ? 'green' : 'gray'"
							:class="
								task.enabled ? 'bg-green-500/20 text-green-500' : 'bg-gray-500/20 text-gray-500'
							"
						>
							{{ task.enabled ? 'Enabled' : 'Disabled' }}
						</Badge>
						<span class="font-semibold text-contrast">{{ taskTypeLabel(task.task_type) }}</span>
					</div>
					<div class="flex items-center gap-2">
						<button
							class="rounded-lg p-2 text-secondary hover:text-contrast hover:bg-surface-3 transition-colors"
							@click="toggleEnabled(task)"
						>
							<PowerIcon class="size-4" />
						</button>
						<button
							class="rounded-lg p-2 text-secondary hover:text-red-500 hover:bg-red-500/10 transition-colors"
							@click="removeTask(task.id)"
						>
							<TrashIcon class="size-4" />
						</button>
					</div>
				</div>
				<div class="text-sm text-secondary">
					<span class="font-mono">{{ task.cron }}</span>
					<span v-if="task.last_run_at">
						&middot; Last run: {{ formatDate(task.last_run_at) }}</span
					>
				</div>
				<div v-if="task.payload" class="text-sm text-secondary">
					{{ taskPayloadPreview(task) }}
				</div>
			</div>
		</div>

		<!-- Create modal -->
		<div
			v-if="showCreate"
			class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
			@click.self="showCreate = false"
		>
			<div class="w-full max-w-md rounded-2xl bg-surface-1 p-6">
				<h3 class="m-0 mb-4 text-xl font-bold text-contrast">Add Scheduled Task</h3>
				<div class="flex flex-col gap-4">
					<div class="flex flex-col gap-2">
						<label class="text-sm font-medium text-secondary">Task Type</label>
						<select v-model="newTaskType" class="rounded-lg bg-surface-3 p-2 text-contrast">
							<option value="backup">Backup</option>
							<option value="restart">Restart</option>
							<option value="command">Command</option>
							<option value="announce">Announce</option>
						</select>
					</div>
					<div class="flex flex-col gap-2">
						<label class="text-sm font-medium text-secondary">Cron Expression</label>
						<input
							v-model="newTaskCron"
							class="rounded-lg bg-surface-3 p-2 font-mono text-contrast"
							placeholder="0 4 * * *"
						/>
					</div>
					<div v-if="newTaskType === 'command'" class="flex flex-col gap-2">
						<label class="text-sm font-medium text-secondary">Command</label>
						<input
							v-model="newTaskCommand"
							class="rounded-lg bg-surface-3 p-2 text-contrast"
							placeholder="say Server restart in 5 minutes"
						/>
					</div>
					<div v-if="newTaskType === 'announce'" class="flex flex-col gap-2">
						<label class="text-sm font-medium text-secondary">Message</label>
						<input
							v-model="newTaskMessage"
							class="rounded-lg bg-surface-3 p-2 text-contrast"
							placeholder="Scheduled maintenance tonight"
						/>
					</div>
					<div class="flex justify-end gap-2">
						<ButtonStyled color="standard">
							<button @click="showCreate = false">Cancel</button>
						</ButtonStyled>
						<ButtonStyled color="brand">
							<button :disabled="!isValidCron" @click="submitCreate">Create</button>
						</ButtonStyled>
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreScheduledTask, CoreTaskType } from '@amberite/amberite-api'
import { ClockIcon, PlusIcon, PowerIcon, TrashIcon } from '@modrinth/assets'
import { Badge, ButtonStyled, injectNotificationManager } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, inject, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey } from './server/core-server-instance'

const core = useCoreClient()
const { handleError } = injectNotificationManager()
const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const showCreate = ref(false)
const newTaskType = ref<CoreTaskType>('backup')
const newTaskCron = ref('0 4 * * *')
const newTaskCommand = ref('')
const newTaskMessage = ref('')
const creating = ref(false)

const tasksQuery = useQuery({
	queryKey: computed(() => ['core-tasks', ctx.instanceId.value]),
	queryFn: () => core.listTasks(ctx.instanceId.value),
	staleTime: 30_000,
})

const tasks = computed<CoreScheduledTask[]>(() => tasksQuery.data.value ?? [])
const loading = computed(() => tasksQuery.isLoading.value)
const error = computed(() => (tasksQuery.error.value as Error | null) ?? null)

const isValidCron = computed(() => {
	const parts = newTaskCron.value.trim().split(/\s+/)
	return parts.length === 5
})

function taskTypeLabel(type: string): string {
	const map: Record<string, string> = {
		backup: 'Backup',
		restart: 'Restart',
		command: 'RCON Command',
		announce: 'Announcement',
	}
	return map[type] ?? type
}

function taskPayloadPreview(task: CoreScheduledTask): string {
	if (!task.payload) return ''
	if (task.task_type === 'command') return `Command: ${task.payload.command ?? ''}`
	if (task.task_type === 'announce') return `Message: ${task.payload.message ?? ''}`
	return ''
}

function formatDate(iso: string): string {
	return new Date(iso).toLocaleString()
}

async function refresh() {
	await tasksQuery.refetch()
}

async function toggleEnabled(task: CoreScheduledTask) {
	try {
		await core.updateTask(ctx.instanceId.value, task.id, { enabled: !task.enabled })
		await refresh()
	} catch (err) {
		handleError(err as Error)
	}
}

async function removeTask(taskId: string) {
	try {
		await core.deleteTask(ctx.instanceId.value, taskId)
		await refresh()
	} catch (err) {
		handleError(err as Error)
	}
}

async function submitCreate() {
	const payload: Record<string, unknown> | undefined =
		newTaskType.value === 'command' && newTaskCommand.value
			? { command: newTaskCommand.value }
			: newTaskType.value === 'announce' && newTaskMessage.value
				? { message: newTaskMessage.value }
				: undefined

	creating.value = true
	try {
		await core.createTask(ctx.instanceId.value, {
			task_type: newTaskType.value,
			cron: newTaskCron.value.trim(),
			payload,
		})
		newTaskCron.value = '0 4 * * *'
		newTaskCommand.value = ''
		newTaskMessage.value = ''
		showCreate.value = false
		await refresh()
	} catch (err) {
		handleError(err as Error)
	} finally {
		creating.value = false
	}
}
</script>
