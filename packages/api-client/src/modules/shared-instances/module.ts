import { AbstractModule } from '../../core/abstract-module'
import type { RequestOptions } from '../../types/request'
import { SharedInstancesApiError } from './errors'

/** The base of every sharing backend module. A refused request throws `SharedInstancesApiError`. */
export abstract class SharedInstancesModule extends AbstractModule {
	protected async request<T>(path: string, options: RequestOptions): Promise<T> {
		try {
			return await this.client.request<T>(path, options)
		} catch (error) {
			throw SharedInstancesApiError.from(error)
		}
	}
}
