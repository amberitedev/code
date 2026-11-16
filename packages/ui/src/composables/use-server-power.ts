import { ApiCallError } from '@modrinth/api-client'
import { CancelledError } from '@tanstack/vue-query'

import type { PowerAction } from '#ui/components/servers/server-header/use-server-power-action'
import { injectModrinthClient, injectModrinthServerContext, injectServerEula } from '#ui/providers'

import { defineMessages, useVIntl } from './i18n'

const messages = defineMessages({
	ownerOnly: {
		id: 'servers.eula.owner-only',
		defaultMessage: "This Core's owner has to accept the Minecraft EULA before its servers start.",
	},
})

const isCode = (error: unknown, code: ApiCallError['code']) =>
	error instanceof ApiCallError && error.code === code

/**
 * Changes the current server's power. A Core starts nothing until its owner accepts the Minecraft
 * EULA, so a start that needs it asks the owner once and then starts.
 */
export function useServerPower() {
	const client = injectModrinthClient()
	const { serverId } = injectModrinthServerContext()
	const { ask } = injectServerEula()
	const { formatMessage } = useVIntl()
	const cores = client.archon.cores_v1

	return async function power(action: PowerAction) {
		try {
			await client.archon.servers_v0.power(serverId, action)
		} catch (error) {
			if (!isCode(error, 'eula_not_accepted')) throw error
			const eula = await cores.eula(serverId).catch((cause: unknown) => {
				throw isCode(cause, 'forbidden')
					? new ApiCallError(403, 'forbidden', formatMessage(messages.ownerOnly))
					: cause
			})
			if (!(await ask(eula.url))) throw new CancelledError()
			await cores.acceptEula(serverId)
			await client.archon.servers_v0.power(serverId, action)
		}
	}
}
