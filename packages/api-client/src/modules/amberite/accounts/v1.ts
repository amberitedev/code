import { AbstractModule } from '../../../core/abstract-module'
import type { Amberite } from '../types'

export class AmberiteAccountsV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'amberite_accounts_v1'
	}

	public async getLinkedModrinth(): Promise<Amberite.Accounts.v1.LinkedModrinthAccount | null> {
		return this.client.request('/account/modrinth', { api: 'amberite', version: 1, method: 'GET' })
	}

	public async storeLinkedModrinth(
		request: Amberite.Accounts.v1.StoreModrinthTokensRequest,
	): Promise<Amberite.Accounts.v1.LinkedModrinthAccount> {
		return this.client.request('/account/modrinth', {
			api: 'amberite',
			version: 1,
			method: 'PUT',
			body: request,
		})
	}

	public async refreshLinkedModrinth(): Promise<Amberite.Accounts.v1.LinkedModrinthAccount | null> {
		return this.client.request('/account/modrinth/refresh', {
			api: 'amberite',
			version: 1,
			method: 'POST',
		})
	}

	public async disconnectLinkedModrinth(): Promise<void> {
		return this.client.request('/account/modrinth', {
			api: 'amberite',
			version: 1,
			method: 'DELETE',
		})
	}
}
