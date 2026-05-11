import type { CoreApiClient } from '@amberite/core-client'

import { createContext } from './create-context'

export const [injectCoreClient, provideCoreClient] = createContext<CoreApiClient>(
	'Index.vue',
	'coreClient',
)
