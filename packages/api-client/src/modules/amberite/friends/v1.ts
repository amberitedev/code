import { AbstractModule } from '../../../core/abstract-module'

export class AmberiteFriendsV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'amberite_friends_v1'
	}

	public async addByCode(code: string): Promise<void> {
		return this.client.request(`/friends/code/${encodeURIComponent(code)}`, {
			api: 'amberite',
			version: 1,
			method: 'POST',
		})
	}
}
