import type { LocalEvent } from '@modrinth/api-client'
import { createContext } from '@modrinth/ui'

export type AppEventType = LocalEvent['type']
export type AppEventPayload<Type extends AppEventType> =
	Extract<LocalEvent, { type: Type }> extends { payload: infer Payload } ? Payload : undefined
export type AppEventHandler<Type extends AppEventType> = (
	payload: AppEventPayload<Type>,
) => void | Promise<void>

export interface AppEvents {
	on<Type extends AppEventType>(type: Type, handler: AppEventHandler<Type>): () => void
	once<Type extends AppEventType>(type: Type, handler: AppEventHandler<Type>): () => void
}

export const [injectAppEvents, provideAppEvents] = createContext<AppEvents>('root', 'appEvents')
