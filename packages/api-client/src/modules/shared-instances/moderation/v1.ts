import { SharedInstancesModule } from '../module'
import type { SharedInstances } from '../types'

export class SharedInstancesModerationV1Module extends SharedInstancesModule {
	public getModuleID(): string {
		return 'sharedinstances_moderation_v1'
	}

	public async blacklistUsers(
		request: SharedInstances.Moderation.v1.BlacklistUserRequest,
	): Promise<void> {
		return this.request('/moderation/blacklist', {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: request,
		})
	}

	public async unblacklistUsers(
		request: SharedInstances.Moderation.v1.BlacklistUserRequest,
	): Promise<void> {
		return this.request('/moderation/blacklist', {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
			body: request,
		})
	}

	public async deleteFile(instanceId: string, version: number, fileName: string): Promise<void> {
		return this.request(
			`/moderation/instances/${encodeURIComponent(instanceId)}/versions/${version}/files/${encodeURIComponent(fileName)}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'DELETE',
			},
		)
	}
}
