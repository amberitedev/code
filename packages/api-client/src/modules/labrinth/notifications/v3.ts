import { type Notification, social } from '@amberite/contracts'

import { AbstractModule } from '../../../core/abstract-module'

export class LabrinthNotificationsV3Module extends AbstractModule {
	public getModuleID(): string {
		return 'labrinth_notifications_v3'
	}

	public async getUserNotifications(userId: string): Promise<Notification[]> {
		return this.client.backend.call(social.ListUserNotifications, { params: { id: userId } })
	}

	public async get(id: string): Promise<Notification> {
		return this.client.backend.call(social.GetNotification, { params: { id } })
	}

	public async getMultiple(ids: string[]): Promise<Notification[]> {
		return this.client.backend.call(social.GetNotifications, { query: idList(ids) })
	}

	public async markAsRead(id: string): Promise<void> {
		return this.client.backend.call(social.ReadNotification, { params: { id } })
	}

	public async markMultipleAsRead(ids: string[]): Promise<void> {
		return this.client.backend.call(social.ReadNotifications, { query: idList(ids) })
	}

	public async delete(id: string): Promise<void> {
		return this.client.backend.call(social.DeleteNotification, { params: { id } })
	}

	public async deleteMultiple(ids: string[]): Promise<void> {
		return this.client.backend.call(social.DeleteNotifications, { query: idList(ids) })
	}
}

function idList(ids: string[]) {
	return { ids: JSON.stringify([...new Set(ids)]) }
}
