import { createContext } from './create-context'

/**
 * A file of a server to save on this computer: `url` is the Core's download route, through the
 * App's Core proxy, which adds this device's session (`client.cores.downloadUrl`).
 */
export type FileDownload = {
	type: 'server-file' | 'server-backup' | 'server-world' | 'server-mrpack'
	url: string
	filename: string
	serverId: string
	serverName?: string
}

export interface FileDownloadProvider {
	download: (file: FileDownload) => Promise<boolean>
}

export const [injectFileDownload, provideFileDownload] = createContext<FileDownloadProvider>(
	'FileDownloadProvider',
	'fileDownload',
)
