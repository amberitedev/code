import { SharedInstancesModule } from '../module'
import type { SharedInstances } from '../types'

/**
 * The signed-in account's shared storage: how much of its allowance on other people's Cores its
 * shares use, how many versions it keeps, and the versions themselves. Deleting a version is
 * `instances_v1.deleteVersion`.
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

	/** Lowering `kept_versions` drops older versions at once. Returns the storage afterwards. */
	public async change(settings: SharedInstances.Storage.v1.StorageSettings) {
		return this.request<SharedInstances.Storage.v1.Storage>('/storage', {
			api: 'sharedinstances',
			version: 1,
			method: 'PATCH',
			body: settings,
		})
	}
}
