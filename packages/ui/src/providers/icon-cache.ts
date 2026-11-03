import { createContext } from './create-context'

export interface IconCacheContext {
<<<<<<< HEAD
	cacheIcon: (source: string | Blob) => Promise<string>
	peekIcon?: (source: string | Blob) => string | undefined
=======
	cacheIcon: (source: string) => Promise<string>
	peekIcon: (source: string) => string | undefined
>>>>>>> modrinth/main
}

export const [injectIconCache, provideIconCache] = createContext<IconCacheContext>('IconCache')
