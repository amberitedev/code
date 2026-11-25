import { core as coreRoutes, type Endpoint, type LocationOf } from '@modrinth/api-client'

import { injectModrinthClient } from '../providers/api-client'
import { type FileDownload, injectFileDownload } from '../providers/file-download'
import { injectModrinthServerContext } from '../providers/server-context'
import { injectNotificationManager } from '../providers/web-notifications'
import { defineMessages, useVIntl } from './i18n'

const messages = defineMessages({
	downloadFailedTitle: {
		id: 'servers.download.failed.title',
		defaultMessage: 'Download failed',
	},
	downloadFailedDescription: {
		id: 'servers.download.failed.description',
		defaultMessage: "An error occurred while trying to download the world's files.",
	},
})

/**
 * Saves files of the open server from its Core: a route of the Core, saved through the App's
 * Core proxy, which adds this device's session. Without a host that saves files, the browser
 * downloads it.
 */
export function useServerDownload() {
	const client = injectModrinthClient()
	const fileDownload = injectFileDownload(null)
	const { serverId, coreId, server } = injectModrinthServerContext()
	const { addNotification } = injectNotificationManager()
	const { formatMessage } = useVIntl()

	async function download<E extends Endpoint>(
		type: FileDownload['type'],
		filename: string,
		endpoint: E,
		location: LocationOf<E>,
	) {
		try {
			const url = await client.cores.downloadUrl(coreId, endpoint, location)
			if (fileDownload) {
				await fileDownload.download({
					type,
					url,
					filename,
					serverId,
					serverName: server.value?.name,
				})
				return
			}
			const link = document.createElement('a')
			link.href = url
			link.download = filename
			link.click()
		} catch (error) {
			addNotification({
				title: formatMessage(messages.downloadFailedTitle),
				text:
					error instanceof Error
						? error.message
						: formatMessage(messages.downloadFailedDescription),
				type: 'error',
			})
		}
	}

	return {
		download,
		/** The world and settings as they are now, zipped. */
		downloadWorld: () =>
			download('server-world', `${server.value?.name ?? serverId}.zip`, coreRoutes.DownloadWorld, {
				params: { id: serverId },
			}),
	}
}
