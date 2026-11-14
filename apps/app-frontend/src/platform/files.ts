import { ContractClient, type FileStat, localFiles } from '@modrinth/api-client'

import { localTransport } from '@/platform/local'

const local = new ContractClient(localTransport)

/** Keeps `@tauri-apps/plugin-fs`'s name. Reads any file on the App's computer. */
export async function readFile(path: string): Promise<Uint8Array<ArrayBuffer>> {
	const response = await local.call(localFiles.readLocalFile, { query: { path } })
	return new Uint8Array(await response.arrayBuffer())
}

export async function statFile(path: string): Promise<FileStat> {
	return await local.call(localFiles.statLocalFile, { query: { path } })
}
