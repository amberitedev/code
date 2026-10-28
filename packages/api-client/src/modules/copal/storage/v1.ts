import { AbstractModule } from '../../../core/abstract-module'
import type { Copal } from '../types'

/**
 * A Core's storage for the shared file pool: how much of other people's encrypted shared files it
 * holds, and the caps its owner sets. Owner only. Turning storage off never affects sharing.
 */
export class CopalStorageV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'copal_storage_v1'
	}

	public async get(coreId: string): Promise<Copal.StorageStatus> {
		return this.client.request<Copal.StorageStatus>('', {
			api: await this.client.archon.cores_v1.url(coreId),
			version: 'storage',
			method: 'GET',
			retry: false,
			skipAuth: true,
			ownAuthorization: true,
		})
	}

	/** Change what is given; the rest stays. Returns the storage as it is afterwards. */
	public async change(
		coreId: string,
		settings: Copal.StorageSettings,
	): Promise<Copal.StorageStatus> {
		return this.client.request<Copal.StorageStatus>('', {
			api: await this.client.archon.cores_v1.url(coreId),
			version: 'storage',
			method: 'PATCH',
			body: settings,
			retry: false,
			skipAuth: true,
			ownAuthorization: true,
		})
	}
}
