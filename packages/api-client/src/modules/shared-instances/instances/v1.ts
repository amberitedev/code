import { AbstractModule } from '../../../core/abstract-module'
import type { RequestOptions } from '../../../types/request'
import { SharedInstancesApiError } from '../errors'
import type { SharedInstances } from '../types'

export class SharedInstancesInstancesV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'sharedinstances_instances_v1'
	}

	/** Every request goes through here so a refused one is thrown as a `SharedInstancesApiError`. */
	private async request<T>(path: string, options: RequestOptions): Promise<T> {
		try {
			return await this.client.request<T>(path, options)
		} catch (error) {
			throw SharedInstancesApiError.from(error)
		}
	}

	public async get(instanceId: string): Promise<SharedInstances.Instances.v1.Instance> {
		return this.request<SharedInstances.Instances.v1.Instance>(
			`/instances/${encodeURIComponent(instanceId)}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}

	public async getForUser(userId: string): Promise<string[]> {
		return this.request<string[]>('/instances', {
			api: 'sharedinstances',
			version: 1,
			method: 'GET',
			params: { user: userId },
		})
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

	public async declinePendingInvite(instanceId: string): Promise<void> {
		return this.request(`/instances/${encodeURIComponent(instanceId)}/invites/pending`, {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
		})
	}

	public async getLatestVersion(
		instanceId: string,
	): Promise<SharedInstances.Instances.v1.InstanceVersion> {
		return this.request<SharedInstances.Instances.v1.InstanceVersion>(
			`/instances/${encodeURIComponent(instanceId)}/versions`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}

	public async getVersion(
		instanceId: string,
		version: number,
	): Promise<SharedInstances.Instances.v1.InstanceVersion> {
		return this.request<SharedInstances.Instances.v1.InstanceVersion>(
			`/instances/${encodeURIComponent(instanceId)}/versions/${version}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}
}
