import {
	ContractClient,
	type ExternalSave,
	type InstanceFileSave,
	type LocalUpload,
	ops,
} from '@modrinth/api-client'

import { localTransport } from '@/platform/local'
import { type OpEnd, runOp } from '@/platform/ops'

const local = new ContractClient(localTransport)

export interface ExternalFileProgress {
	stage: 'waiting' | 'downloading' | 'saving'
	downloadedBytes: number
	totalBytes: number | null
}

export interface LocalUploadProgress {
	loaded: number
	total: number
}

export function saveExternal(
	save: ExternalSave,
	onProgress: (progress: ExternalFileProgress) => void,
): Promise<OpEnd> {
	return runOp(
		save.op,
		() => local.call(ops.saveExternalFile, { body: save }),
		({ stage, done, total }) =>
			onProgress({
				stage: stage === 'downloading' || stage === 'saving' ? stage : 'waiting',
				downloadedBytes: done,
				totalBytes: total,
			}),
	)
}

export function uploadLocal(
	upload: LocalUpload,
	onProgress: (progress: LocalUploadProgress) => void,
): Promise<OpEnd> {
	return runOp(
		upload.op,
		() => local.call(ops.uploadLocalFile, { body: upload }),
		({ done, total }) => onProgress({ loaded: done, total: total ?? done }),
	)
}

export function saveInstanceFileAs(save: InstanceFileSave): Promise<OpEnd> {
	return runOp(save.op, () => local.call(ops.saveInstanceFileAs, { body: save }))
}
