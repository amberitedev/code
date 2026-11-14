import { boot as bootRoutes, type Boot, ContractClient } from '@modrinth/api-client'

import { localBase, localTransport } from '@/platform/local'

let boot: Boot | undefined

/** `main.js` awaits this before anything else loads, so the readers below can stay synchronous. */
export async function loadBoot(): Promise<Boot> {
	if (!localBase) {
		throw new Error(
			"The App's local address is missing. A tab needs the URL the dev runner prints, with #local=",
		)
	}
	boot ??= await new ContractClient(localTransport).call(bootRoutes.getBoot)
	return boot
}

export function getBoot(): Boot {
	if (!boot) throw new Error('Boot info was read before main.js loaded it')
	return boot
}

export async function getVersion(): Promise<string> {
	return getBoot().version
}

// These keep `@tauri-apps/plugin-os`'s names: components read them during setup.
export const platform = () => getBoot().platform
export const type = () => getBoot().platform
export const version = () => getBoot().os_version
