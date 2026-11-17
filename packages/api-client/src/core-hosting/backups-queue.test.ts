import type { BackupOperation } from '@amberite/contracts'
import { describe, expect, it } from 'vite-plus/test'

import { queue } from './backups-queue'

function operation(id: number, kind: BackupOperation['kind']): BackupOperation {
	return {
		id,
		kind,
		state: 'done',
		queued_at: id,
		started_at: id,
		ended_at: id,
		error: null,
		acked: false,
		user: null,
	}
}

describe('queue', () => {
	// The panel shows a backup's newest operation, so a restore is reported over the backup's
	// own creation.
	it('lists a backup history newest first', () => {
		const { backups } = queue({
			backups: [
				{
					id: 'backup',
					name: 'Fresh',
					created_at: 1,
					state: 'done',
					kind: 'manual',
					rotates_at: null,
					operations: [operation(1, 'create'), operation(2, 'restore')],
				},
			],
			schedule: { interval_hours: 24, keep: 7, slots: 15 },
			next_scheduled_at: null,
		})
		expect(backups[0]!.history.map((entry) => entry.operation_type)).toEqual(['restore', 'create'])
	})
})
