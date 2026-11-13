import type { KyrosLogsV1Module } from '../modules/kyros/logs/v1'
import type { Adapter } from './adapter'

export function kyrosLogs(): Adapter<KyrosLogsV1Module> {
	// A Core keeps no log buffer of its own: the console reads the server's log files, which stay.
	return { async clear() {} }
}
