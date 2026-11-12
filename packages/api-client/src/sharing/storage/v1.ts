import { sharing } from '@amberite/contracts'

import { AbstractModule } from '../../core/abstract-module'

/**
 * The signed-in account's shared storage: how much of its allowance on other people's Cores its
 * shares use, and the versions they keep there.
 */
export class SharedInstancesStorageV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'sharedinstances_storage_v1'
	}

	public async get() {
		return this.client.backend.call(sharing.GetShareStorage)
	}
}
