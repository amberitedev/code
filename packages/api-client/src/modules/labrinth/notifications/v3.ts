import { AbstractModule } from '../../../core/abstract-module'
import type { Labrinth } from '../types'

export class LabrinthNotificationsV3Module extends AbstractModule {
	public getModuleID(): string {
		return 'labrinth_notifications_v3'
	}

	public async getUserNotifications(
		userId: string,
	): Promise<Labrinth.Notifications.v3.Notification[]> {
		return this.client.request(`/user/${encodeURIComponent(userId)}/notifications`, {
			api: 'labrinth',
			version: 3,
			method: 'GET',
		})
	}

	public async get(id: string): Promise<Labrinth.Notifications.v3.Notification> {
		return this.client.request(`/notification/${encodeURIComponent(id)}`, {
			api: 'labrinth',
			version: 3,
			method: 'GET',
		})
	}

	public async getMultiple(ids: string[]): Promise<Labrinth.Notifications.v3.Notification[]> {
		return this.client.request('/notifications', {
			api: 'labrinth',
			version: 3,
			method: 'GET',
			params: { ids: JSON.stringify([...new Set(ids)]) },
		})
	}

	public async markAsRead(id: string): Promise<void> {
		return this.client.request(`/notification/${encodeURIComponent(id)}`, {
			api: 'labrinth',
			version: 3,
			method: 'PATCH',
		})
	}

	public async markMultipleAsRead(ids: string[]): Promise<void> {
		return this.client.request('/notifications', {
			api: 'labrinth',
			version: 3,
			method: 'PATCH',
			params: { ids: JSON.stringify([...new Set(ids)]) },
		})
	}

	public async delete(id: string): Promise<void> {
		return this.client.request(`/notification/${encodeURIComponent(id)}`, {
			api: 'labrinth',
			version: 3,
			method: 'DELETE',
		})
	}

	public async deleteMultiple(ids: string[]): Promise<void> {
		return this.client.request('/notifications', {
			api: 'labrinth',
			version: 3,
			method: 'DELETE',
			params: { ids: JSON.stringify([...new Set(ids)]) },
		})
	}
}
