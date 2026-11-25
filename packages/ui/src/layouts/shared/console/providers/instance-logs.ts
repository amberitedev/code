import { type ContractClient, type Logs, logs as logRoutes } from '@modrinth/api-client'
import { type QueryKey, useQuery, useQueryClient } from '@tanstack/vue-query'
import {
	computed,
	type MaybeRefOrGetter,
	type Ref,
	ref,
	shallowRef,
	toValue,
	triggerRef,
	watch,
	watchEffect,
} from 'vue'

import { createConsoleState } from '#ui/composables/server-console'

import type { LogSource } from '../types'

type ConsoleState = ReturnType<typeof createConsoleState>

export interface InstanceLogsOptions {
	/** `client.local` for an instance on this computer, `client.core(id)` for a server. */
	client: MaybeRefOrGetter<ContractClient>
	instanceId: MaybeRefOrGetter<string>
	/** The live console, which the host fills from the `log` events of its event socket. */
	live: ConsoleState
	/** Whether the instance runs now, so its `latest.log` is still being written. */
	running: Ref<boolean>
	queryKey: MaybeRefOrGetter<QueryKey>
	onError: (error: unknown) => void
}

type LogEntry = Logs & { name: string }

/** Old logs worth showing: the game's own, never empty, not the launcher's. */
function shown(log: Logs) {
	return (
		log.filename !== 'latest_stdout.log' &&
		log.filename !== 'latest_stdout' &&
		log.filename !== 'launcher_log.txt' &&
		(log.output == null || log.output !== '') &&
		(log.filename.includes('.log') || log.filename.endsWith('.txt'))
	)
}

/**
 * An instance's console: its live output and its old logs over the engine's log routes, which an
 * App and a Core both serve. The Console tab's provider for either; the host feeds the live
 * console and adds what only it has, such as a server's command input.
 */
export function useInstanceLogs(options: InstanceLogsOptions) {
	const queryClient = useQueryClient()
	const client = () => toValue(options.client)
	const id = () => toValue(options.instanceId)

	const list = useQuery(
		computed(() => ({
			queryKey: toValue(options.queryKey),
			queryFn: () =>
				client().call(logRoutes.ListInstanceLogs, {
					params: { id: id() },
					query: { clearContents: true },
				}),
			staleTime: 0,
		})),
	)
	watch(list.error, (error) => {
		if (error) options.onError(error)
	})

	const logs = computed<LogEntry[]>(() =>
		(list.data.value ?? []).filter(shown).map((log) => ({ ...log, name: log.filename })),
	)
	// While the instance runs, its latest log is the live console.
	const listed = computed(() =>
		options.running.value ? logs.value.filter((log) => log.filename !== 'latest.log') : logs.value,
	)

	const activeLogSourceIndex = ref(0)
	const logSources = computed<LogSource[]>(() => [
		{ id: '0', name: 'Live Log', live: true },
		...listed.value.map((log, index) => ({ id: String(index + 1), name: log.name, live: false })),
	])
	const selected = computed(() =>
		activeLogSourceIndex.value === 0 ? null : listed.value[activeLogSourceIndex.value - 1],
	)

	const historical = createConsoleState()
	const outputs = new Map<string, string>()
	watch(selected, async (log) => {
		if (!log) return
		historical.clear()
		const output =
			outputs.get(log.filename) ??
			(await client()
				.call(logRoutes.GetInstanceLogOutput, {
					params: { id: id(), log_type: log.log_type, filename: log.filename },
				})
				.catch((error: unknown) => {
					options.onError(error)
					return null
				}))
		if (output === null || selected.value !== log) return
		outputs.set(log.filename, output)
		historical.addLegacyLog(output)
	})

	const active = computed(() => (selected.value ? historical : options.live))
	const logLines = shallowRef(active.value.output.value)
	watchEffect(() => {
		logLines.value = active.value.output.value
		triggerRef(logLines)
	})

	/** Reads the list of old logs again, such as after the instance stopped. */
	async function refresh() {
		outputs.clear()
		await queryClient.invalidateQueries({ queryKey: toValue(options.queryKey) })
	}

	return {
		logLines,
		logSources,
		activeLogSourceIndex,
		isLive: computed(() => selected.value === null),
		deleteDisabled: computed(() => selected.value === null),
		onDelete: async () => {
			const log = selected.value
			if (!log) return
			await client().call(logRoutes.DeleteInstanceLog, {
				params: { id: id(), log_type: log.log_type, filename: log.filename },
			})
			activeLogSourceIndex.value = 0
			await refresh()
		},
		refresh,
	}
}
