import { SharedInstancesModule } from '../module'
import type { SharedInstances } from '../types'

export class SharedInstancesUsersV1Module extends SharedInstancesModule {
	public getModuleID(): string {
		return 'sharedinstances_users_v1'
	}

	public async getBlacklistStatus(
		userId: string,
	): Promise<SharedInstances.Users.v1.BlacklistStatus> {
		return this.request<SharedInstances.Users.v1.BlacklistStatus>(
			`/blacklist/${encodeURIComponent(userId)}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}
}
