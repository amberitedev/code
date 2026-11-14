import { type LocalFileUploader, ModrinthApiError } from '@modrinth/api-client'

import { uploadLocal } from '@/platform/app-lib/files/commands'
import { cancelOp } from '@/platform/ops'

/** Uploads local files from the Rust side, streaming them from disk. */
export function createLocalFileUploader(): LocalFileUploader {
	return async ({ url, headers, file, multipartField, signal, onProgress }) => {
		const uploadId = crypto.randomUUID()
		const cancel = () => void cancelOp(uploadId).catch(() => undefined)
		signal.addEventListener('abort', cancel)
		try {
			const { error, response } = await uploadLocal(
				{
					op: uploadId,
					url,
					headers,
					path: file.path,
					file_name: file.name,
					multipart_field: multipartField ?? null,
				},
				({ loaded, total }) =>
					onProgress({ loaded, total, progress: total > 0 ? loaded / total : 1 }),
			)
			if (error || !response) throw new ModrinthApiError(error?.description ?? 'Upload failed')
			return response
		} finally {
			signal.removeEventListener('abort', cancel)
		}
	}
}
