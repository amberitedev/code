import { ContractClient, ops } from '@modrinth/api-client'
import { computed, reactive, ref } from 'vue'

import { toError } from '@/helpers/errors'
import { save } from '@/platform/dialog'
import { localTransport } from '@/platform/local'
import { cancelOp, runOp } from '@/platform/ops'

export interface DebugInfoExportProgress {
	stage: 'preparing' | 'exporting' | 'finishing'
	processedBytes: number
	totalBytes: number | null
}

export interface DebugInfoExportTask extends DebugInfoExportProgress {
	id: string
	filename: string
	status: 'running' | 'succeeded' | 'failed' | 'canceled'
	started: boolean
	canceling: boolean
	createdAt: string
	finishedAt?: string
	error?: string
	cancel: () => Promise<void>
}

const local = new ContractClient(localTransport)

export const debugInfoExports = ref(new Map<string, DebugInfoExportTask>())
const selectingDestination = ref(false)
export const exportingDebugInfo = computed(
	() =>
		selectingDestination.value ||
		[...debugInfoExports.value.values()].some((task) => task.status === 'running'),
)

export async function exportDebugInfo(title: string) {
	if (exportingDebugInfo.value) return
	selectingDestination.value = true
	let path: string | null
	try {
		path = await save({
			title,
			defaultPath: `modrinth-debug-${new Date().toISOString().replace(/[:.]/g, '-')}.zip`,
			filters: [{ name: 'ZIP', extensions: ['zip'] }],
		})
	} finally {
		selectingDestination.value = false
	}
	if (!path) return

	const task = reactive<DebugInfoExportTask>({
		id: crypto.randomUUID(),
		filename: path.split(/[/\\]/).pop() ?? path,
		status: 'running',
		stage: 'preparing',
		processedBytes: 0,
		totalBytes: null,
		started: false,
		canceling: false,
		createdAt: new Date().toISOString(),
		async cancel() {
			if (
				task.status !== 'running' ||
				!task.started ||
				task.canceling ||
				task.stage === 'finishing'
			)
				return
			task.canceling = true
			try {
				await cancelOp(task.id)
			} catch (error) {
				task.canceling = false
				throw error
			}
		},
	})
	debugInfoExports.value.set(task.id, task)
	try {
		const { error } = await runOp(
			task.id,
			() => local.call(ops.exportDebugInfo, { body: { op: task.id, path } }),
			({ stage, done, total }) => {
				if (task.status !== 'running') return
				task.started = true
				if (stage === 'preparing' || stage === 'exporting' || stage === 'finishing') {
					task.stage = stage
				}
				task.processedBytes = done
				task.totalBytes = total
			},
		)
		if (error && error.error !== 'cancelled') throw new Error(error.description)
		task.status = error ? 'canceled' : 'succeeded'
	} catch (error) {
		task.status = 'failed'
		task.error = toError(error).message
		throw error
	} finally {
		task.canceling = false
		task.finishedAt = new Date().toISOString()
	}
}
