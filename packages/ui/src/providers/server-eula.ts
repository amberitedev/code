import { createContext } from './create-context'

/** Shows the Minecraft EULA at `url` to a Core's owner. Resolves true when they accept it. */
export const [injectServerEula, provideServerEula] = createContext<{
	ask: (url: string) => Promise<boolean>
}>('root.vue', 'serverEula')
