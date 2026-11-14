import {
	ContractClient,
	type ExternalSave,
	type ExtractDryRunResult,
	type InstanceFileItem,
	type InstanceFileSave,
	instanceFiles,
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

/** Paths are inside the instance's folder; `""` lists the folder itself. */
export function listInstanceFiles(id: string, path: string): Promise<InstanceFileItem[]> {
	return local.call(instanceFiles.ListInstanceFiles, { params: { id }, query: { path } })
}

export function readInstanceFile(id: string, path: string): Promise<Blob> {
	return local.call(instanceFiles.ReadInstanceFile, { params: { id }, query: { path } })
}

export function writeInstanceFile(
	id: string,
	path: string,
	contents: Blob,
	createOnly = false,
): Promise<void> {
	return local.call(instanceFiles.WriteInstanceFile, {
		params: { id },
		query: { path, create_only: createOnly },
		body: contents,
	})
}

export function createInstanceDirectory(id: string, path: string): Promise<void> {
	return local.call(instanceFiles.CreateInstanceDirectory, { params: { id }, query: { path } })
}

export function moveInstanceFile(id: string, source: string, destination: string): Promise<void> {
	return local.call(instanceFiles.MoveInstanceFile, {
		params: { id },
		body: { source, destination },
	})
}

export function deleteInstanceFile(id: string, path: string, recursive: boolean): Promise<void> {
	return local.call(instanceFiles.DeleteInstanceFile, {
		params: { id },
		query: { path, recursive },
	})
}

/** A dry run answers the files it would replace; a real run answers nothing. */
export async function extractInstanceZip(
	id: string,
	path: string,
	overrideConflicts: boolean,
	dryRun: boolean,
): Promise<ExtractDryRunResult | undefined> {
	const result = await local.call(instanceFiles.ExtractInstanceZip, {
		params: { id },
		body: { path, override_conflicts: overrideConflicts, dry_run: dryRun },
	})
	return result ?? undefined
}
