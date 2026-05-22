import type { CoreInstanceStateManager } from '@amberite/amberite-api'

import { createContext } from './create-context'

export const [injectCoreInstanceState, provideCoreInstanceState] =
	createContext<CoreInstanceStateManager>('App.vue', 'coreInstanceState')
