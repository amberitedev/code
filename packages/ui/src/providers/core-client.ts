import type { CoreApiClient } from '@amberite/api-lib'

import { createContext } from './create-context'

export const [injectCoreClient, provideCoreClient] = createContext<CoreApiClient>(
	'Index.vue',
	'coreClient',
)
