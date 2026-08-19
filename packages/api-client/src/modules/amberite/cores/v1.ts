import { AbstractModule } from '../../../core/abstract-module'
import type { Amberite } from '../types'

export class AmberiteCoresV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'amberite_cores_v1'
	}

	public async list(): Promise<Amberite.Cores.v1.CoreListEntry[]> {
		return this.client.request('/cores', { api: 'amberite', version: 1, method: 'GET' })
	}

	public async claimPairing(code: string): Promise<Amberite.Cores.v1.PairingClaim | null> {
		return this.client.request('/cores/pairing/claim', {
			api: 'amberite',
			version: 1,
			method: 'POST',
			body: { code },
		})
	}

	public async finalizePairing(args: {
		code: string
		coreId: string
		connectionUrl?: string
	}): Promise<{ coreId: string }> {
		return this.client.request('/cores/pairing/finalize', {
			api: 'amberite',
			version: 1,
			method: 'POST',
			body: args,
		})
	}

	public async releasePairing(code: string, coreId: string): Promise<void> {
		return this.client.request('/cores/pairing/release', {
			api: 'amberite',
			version: 1,
			method: 'POST',
			body: { code, coreId },
		})
	}
}
