import { provideFileDrop } from '@modrinth/ui'

import { listenNativeDragDrop } from '@/helpers/native-drag-drop'
import { readFile, statFile } from '@/platform/files'
import { isNativeWindow } from '@/platform/local'

function getFileName(path: string) {
	return path.split(/[\\/]/).pop() || 'file'
}

/** A tab has no native drops; `packages/ui` reads the browser's own drop events there. */
export function setupFileDropProvider() {
	if (!isNativeWindow) return
	let nativeFileDropPaths: string[] = []

	provideFileDrop({
		async listenNativeFileDrop(handler) {
			return await listenNativeDragDrop((event) => {
				if (event.type === 'leave') {
					nativeFileDropPaths = []
					void handler({
						type: 'leave',
						paths: [],
						position: { x: 0, y: 0 },
					})
					return
				}

				if (event.type === 'enter' || event.type === 'drop') {
					nativeFileDropPaths = event.paths
				}

				void handler({
					type: event.type,
					paths: nativeFileDropPaths,
					position: event.position,
				})

				if (event.type === 'drop') {
					nativeFileDropPaths = []
				}
			})
		},
		async createFilesFromNativePaths(paths) {
			return await Promise.all(
				paths.map(async (path) => new File([await readFile(path)], getFileName(path))),
			)
		},
		async resolveLocalFiles(paths) {
			return await Promise.all(
				paths.map(async (path) => ({
					kind: 'local' as const,
					path,
					...(await statFile(path)),
				})),
			)
		},
	})
}
