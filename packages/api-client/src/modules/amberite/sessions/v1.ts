import { AbstractModule } from '../../../core/abstract-module'
import type { Amberite } from '../types'

export class AmberiteSessionsV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'amberite_sessions_v1'
	}

	public async registerCurrent(metadata: Amberite.Sessions.v1.DeviceMetadata): Promise<void> {
		return this.client.request('/sessions/current', {
			api: 'amberite',
			version: 1,
			method: 'PUT',
			body: metadata,
		})
	}
}
