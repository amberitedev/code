import { AbstractModule } from '../../../core/abstract-module'
import type { SharedInstances } from '../types'

export class SharedInstancesInstancesV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'sharedinstances_instances_v1'
	}

	public async get(instanceId: string): Promise<SharedInstances.Instances.v1.Instance> {
		return this.client.request<SharedInstances.Instances.v1.Instance>(
			`/instances/${encodeURIComponent(instanceId)}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}

	public async create(name: string): Promise<{ id: string }> {
		return this.client.request('/instances', {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: { name },
		})
	}

	public async update(instanceId: string, name: string): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}`, {
			api: 'sharedinstances',
			version: 1,
			method: 'PATCH',
			body: { name },
		})
	}

	public async delete(instanceId: string): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}`, {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
		})
	}

	public async getForUser(userId: string): Promise<string[]> {
		return this.client.request<string[]>('/instances', {
			api: 'sharedinstances',
			version: 1,
			method: 'GET',
			params: { user: userId },
		})
	}

	public async getUsers(instanceId: string): Promise<SharedInstances.Instances.v1.InstanceUsers> {
		return this.client.request<SharedInstances.Instances.v1.InstanceUsers>(
			`/instances/${encodeURIComponent(instanceId)}/users`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}

	public async inviteUsers(instanceId: string, userIds: string[]): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/users`, {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: { user_ids: [...new Set(userIds)] },
		})
	}

	public async removeUsers(instanceId: string, userIds: string[]): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/users`, {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
			body: { user_ids: [...new Set(userIds)] },
		})
	}

	public async createInvite(
		instanceId: string,
		options: { max_age?: number; max_uses?: number } = {},
	): Promise<{ id: string }> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/invites`, {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: options,
		})
	}

	public async getInvites(instanceId: string): Promise<SharedInstances.Instances.v1.LinkInvite[]> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/invites`, {
			api: 'sharedinstances',
			version: 1,
			method: 'GET',
		})
	}

	public async revokeInvite(instanceId: string, inviteId: string): Promise<void> {
		return this.client.request(
			`/instances/${encodeURIComponent(instanceId)}/invites/${encodeURIComponent(inviteId)}`,
			{ api: 'sharedinstances', version: 1, method: 'DELETE' },
		)
	}

	public async acceptPendingInvite(instanceId: string): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/invites/pending`, {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
		})
	}

	public async declinePendingInvite(instanceId: string): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/invites/pending`, {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
		})
	}

	public async acceptInvite(instanceId: string, inviteId: string): Promise<void> {
		return this.client.request(
			`/instances/${encodeURIComponent(instanceId)}/invites/${encodeURIComponent(inviteId)}`,
			{ api: 'sharedinstances', version: 1, method: 'POST' },
		)
	}

	public async createVersion(
		instanceId: string,
		request: SharedInstances.Instances.v1.CreateVersionRequest,
	): Promise<SharedInstances.Instances.v1.InstanceVersion> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/versions`, {
			api: 'sharedinstances',
			version: 1,
			method: 'POST',
			body: request,
		})
	}

	public async setIcon(instanceId: string, icon: Blob): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/icon`, {
			api: 'sharedinstances',
			version: 1,
			method: 'PUT',
			headers: { 'Content-Type': icon.type || 'application/octet-stream' },
			body: icon,
		})
	}

	public async deleteIcon(instanceId: string): Promise<void> {
		return this.client.request(`/instances/${encodeURIComponent(instanceId)}/icon`, {
			api: 'sharedinstances',
			version: 1,
			method: 'DELETE',
		})
	}

	public async uploadExternalFile(uploadToken: string, file: Blob): Promise<void> {
		return this.client.request(`/uploads/${encodeURIComponent(uploadToken)}`, {
			api: 'sharedinstances',
			version: 1,
			method: 'PUT',
			headers: { 'Content-Type': file.type || 'application/octet-stream' },
			body: file,
		})
	}

	public async getLatestVersion(
		instanceId: string,
	): Promise<SharedInstances.Instances.v1.InstanceVersion> {
		return this.client.request<SharedInstances.Instances.v1.InstanceVersion>(
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
		return this.client.request<SharedInstances.Instances.v1.InstanceVersion>(
			`/instances/${encodeURIComponent(instanceId)}/versions/${version}`,
			{
				api: 'sharedinstances',
				version: 1,
				method: 'GET',
			},
		)
	}
}
