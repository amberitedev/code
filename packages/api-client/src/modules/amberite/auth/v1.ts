import { AbstractModule } from '../../../core/abstract-module'
import type { Amberite } from '../types'

export class AmberiteAuthV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'amberite_auth_v1'
	}

	public async signIn(request: Amberite.Auth.v1.SignInRequest): Promise<Amberite.Auth.v1.Session> {
		return this.client.request('/auth/sign-in', {
			api: 'amberite',
			version: 1,
			method: 'POST',
			body: request,
			skipAuth: true,
		})
	}

	public async restore(): Promise<Amberite.Auth.v1.Session | null> {
		return this.client.request('/auth/session', {
			api: 'amberite',
			version: 1,
			method: 'GET',
			skipAuth: true,
		})
	}

	public async signOut(): Promise<void> {
		return this.client.request('/auth/session', {
			api: 'amberite',
			version: 1,
			method: 'DELETE',
		})
	}
}
