import {
	type ImageViewerEditorData,
	type ImageViewerEditorSource,
	provideImageViewerEditor,
} from '@modrinth/ui'

import { readFile } from '@/platform/files'

export function setupImageViewerEditorProvider() {
	provideImageViewerEditor({
		async loadEditorData(source: ImageViewerEditorSource): Promise<ImageViewerEditorData> {
			return {
				source: new Blob([await readFile(source.path)], { type: 'image/png' }),
			}
		},
	})
}
