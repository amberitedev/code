import { SharedInstancesModule } from '../module'
import type { SharedInstances } from '../types'

export class SharedInstancesInstancesV1Module extends SharedInstancesModule {
	public getModuleID(): string {
		return 'sharedinstances_instances_v1'
	}

	public async get(
		instanceId: string,
		params?: { query_linked_server?: boolean },
	): Promise<SharedInstances.Instances.v1.Instance> {
		return this.request<SharedInstances.Instances.v1.Instance>(
			`/instances/${encodeURIComponent(instanceId)}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
				params,
			},
		)
	}

	public async getUsers(instanceId: string): Promise<SharedInstances.Instances.v1.InstanceUsers> {
		return this.request<SharedInstances.Instances.v1.InstanceUsers>(
			`/instances/${encodeURIComponent(instanceId)}/users`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}

	/** The servers other people host for this shared instance and show to its members. */
	public async getServers(
		instanceId: string,
	): Promise<SharedInstances.Instances.v1.HostedServer[]> {
		return this.request(`/instances/${encodeURIComponent(instanceId)}/servers`, {
			api: 'sharedinstances',
			version: 1,
			method: 'GET',
		})
	}

	/**
	 * Owner or member. A ticket for Core `coreId` to read this share for the signed-in user, sent
	 * with an `instance` install so the Core reads only what that user can.
	 */
	public async getServerTicket(
		instanceId: string,
		coreId: string,
	): Promise<{ ticket: string; expires: string }> {
		return this.request(`/instances/${encodeURIComponent(instanceId)}/server-ticket`, {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: { core_id: coreId },
		})
	}

	public async inviteUsers(instanceId: string, userIds: string[]): Promise<{ failed: string[] }> {
		return this.request(`/instances/${encodeURIComponent(instanceId)}/users`, {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: { user_ids: userIds },
			retry: false,
		})
	}

	public async removeUsers(instanceId: string, userIds: string[]): Promise<void> {
		return this.request(`/instances/${encodeURIComponent(instanceId)}/users`, {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
			body: { user_ids: userIds },
		})
	}
}
