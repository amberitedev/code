import { type FileStat, localFiles } from '@amberite/contracts/app'

import { apiClient } from '@/platform/modrinth-client'

/** Keeps `@tauri-apps/plugin-fs`'s name. Reads any file on the App's computer. */
export async function readFile(path: string): Promise<Uint8Array<ArrayBuffer>> {
	const response = await apiClient.local.call(localFiles.readLocalFile, { query: { path } })
	return new Uint8Array(await response.arrayBuffer())
}

export async function statFile(path: string): Promise<FileStat> {
	return await apiClient.local.call(localFiles.statLocalFile, { query: { path } })
}
