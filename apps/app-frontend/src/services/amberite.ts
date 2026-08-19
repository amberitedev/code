import {
	AmberiteApiClient,
	AmberiteAuthClient,
	CoreApiClient,
	ConvexAmberiteTransport,
} from '@modrinth/api-client'

import { createDesktopAdapter } from '@/adapters/desktop'

const adapter = createDesktopAdapter()
const transport = new ConvexAmberiteTransport(adapter)

export const amberite = {
	adapter,
	transport,
	auth: new AmberiteAuthClient({ adapter, transport }),
	api: new AmberiteApiClient(transport),
	core: new CoreApiClient(adapter),
}
