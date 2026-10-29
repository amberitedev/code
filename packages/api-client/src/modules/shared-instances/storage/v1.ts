import { AbstractModule } from '../../../core/abstract-module'
import type { RequestOptions } from '../../../types/request'
import { SharedInstancesApiError } from '../errors'
import type { SharedInstances } from '../types'

/**
 * The signed-in account's shared storage: how much of its allowance on other people's Cores its
 * shares use, how many versions it keeps, and the versions themselves. Deleting a version is
 * `instances_v1.deleteVersion`.
 */
export class SharedInstancesStorageV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'sharedinstances_storage_v1'
	}

	private async request(options: RequestOptions) {
		try {
			return await this.client.request<SharedInstances.Storage.v1.Storage>('/storage', options)
		} catch (error) {
			throw SharedInstancesApiError.from(error)
		}
	}

	public async get() {
		return this.request({ api: 'sharedinstances', version: 1, method: 'GET' })
	}

	/** Lowering `kept_versions` drops older versions at once. Returns the storage afterwards. */
	public async change(settings: SharedInstances.Storage.v1.StorageSettings) {
		return this.request({ api: 'sharedinstances', version: 1, method: 'PATCH', body: settings })
	}
}
