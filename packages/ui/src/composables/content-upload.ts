import {
	ApiCallError,
	type ContractClient,
	instanceFiles,
	type UploadState,
} from '@modrinth/api-client'
import type { Ref } from 'vue'

import type { CancelUploadHandler } from '#ui/providers/server-context'

/**
 * Uploads files into an instance's content, mods or plugins by their names, on the App or a Core.
 * Progress goes to `uploadState`; `cancelUpload` aborts it while it runs.
 */
export function useContentUpload(options: {
	client: ContractClient
	instanceId: string
	uploadState: Ref<UploadState>
	cancelUpload: Ref<CancelUploadHandler | null>
}) {
	const { client, instanceId, uploadState, cancelUpload } = options

	async function uploadFiles(files: File[]): Promise<'completed' | 'cancelled'> {
		const { upload_id: upload } = await client.call(instanceFiles.OpenInstanceUpload, {
			params: { id: instanceId },
			body: { scope: 'content' },
		})
		const params = { id: instanceId, upload }
		const abort = new AbortController()
		cancelUpload.value = () => abort.abort()
		uploadState.value = {
			isUploading: true,
			currentFileName: files[0]?.name ?? null,
			currentFileProgress: 0,
			uploadedBytes: 0,
			totalBytes: files.reduce((sum, file) => sum + file.size, 0),
			completedFiles: 0,
			totalFiles: files.length,
		}
		try {
			for (const file of files) {
				const sent = uploadState.value.uploadedBytes
				uploadState.value.currentFileName = file.name
				uploadState.value.currentFileProgress = 0
				await client.call(
					instanceFiles.AddInstanceUploadFile,
					{ params, query: { path: file.name }, body: file },
					{
						signal: abort.signal,
						onUploadProgress: ({ loaded, progress }) => {
							uploadState.value.currentFileProgress = progress
							uploadState.value.uploadedBytes = sent + loaded
						},
					},
				)
				uploadState.value.completedFiles++
				uploadState.value.uploadedBytes = sent + file.size
			}
			await client.call(instanceFiles.FinalizeInstanceUpload, { params })
			return 'completed'
		} catch (error) {
			await client.call(instanceFiles.CancelInstanceUpload, { params }).catch(() => {})
			if (error instanceof ApiCallError && error.code === 'cancelled') return 'cancelled'
			throw error
		} finally {
			cancelUpload.value = null
			uploadState.value = { ...uploadState.value, isUploading: false }
		}
	}

	return { uploadFiles }
}
