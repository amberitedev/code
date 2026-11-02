import { SharedInstancesModule } from '../module'
import type { SharedInstances } from '../types'

/**
 * The signed-in account's shared storage: how much of its allowance on other people's Cores its
 * shares use, and the versions they keep there.
 */
export class SharedInstancesStorageV1Module extends SharedInstancesModule {
	public getModuleID(): string {
		return 'sharedinstances_storage_v1'
	}

	public async get() {
		return this.request<SharedInstances.Storage.v1.Storage>('/storage', {
			api: 'sharedinstances',
			version: 1,
			method: 'GET',
		})
	}
}
