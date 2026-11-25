import type { Server } from '@amberite/contracts/core'
import { instanceContent } from '@amberite/contracts/engine'
import type { ContractClient } from '@modrinth/api-client'

import { createContext } from '#ui/providers/create-context'

export interface ShareUnlinkPrompt {
	/** Asks the owner to unlink the server's uploaded modpack. Resolves true once they confirm. */
	ask: (server: Server) => Promise<boolean>
}

/** Provided by the server panel and server setup, which each show the App's unlink confirm. */
export const [injectShareUnlinkPrompt, provideShareUnlinkPrompt] = createContext<ShareUnlinkPrompt>(
	'root',
	'shareUnlinkPrompt',
)

/** The owner kept the uploaded modpack, so the server was not shared. */
export class ShareUnlinkDeclined extends Error {}

/**
 * Unlinks the `.mrpack` a server was set up from by upload once its owner confirms, since a share
 * cannot carry one. Every way to share a server calls it first. Resolves whether it unlinked one;
 * throws {@link ShareUnlinkDeclined} when the owner keeps the pack.
 */
export async function unlinkBeforeShare(
	core: ContractClient,
	server: Server,
	prompt: ShareUnlinkPrompt | null,
) {
	if (!server.uploaded_modpack) return false
	if (!(await prompt?.ask(server))) throw new ShareUnlinkDeclined()
	await core.call(instanceContent.UnlinkModpack, { params: { id: server.server_id } })
	return true
}
