import type { CoreApiClient } from '@amberite/amberite-api'

import { createContext } from './create-context'

export const [injectCoreClient, provideCoreClient] = createContext<CoreApiClient>(
	'Index.vue',
	'coreClient',
)
