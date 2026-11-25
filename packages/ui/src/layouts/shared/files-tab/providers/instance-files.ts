import { instanceFiles, type InstanceZipRequest } from '@amberite/contracts/engine'
import { ApiCallError, type ContractClient, type UploadState } from '@modrinth/api-client'
import { type QueryKey, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, type Ref, ref, toValue, watch } from 'vue'

import type { EditingFile, FileItem } from '../types'
import type { FileManagerContext } from './file-manager'

/** What went wrong, as the host tells the user. */
export type FilesFailure =
	| 'create'
	| 'rename'
	| 'move'
	| 'delete'
	| 'upload'
	| 'extract'
	| 'write'
	| 'zip'

export interface InstanceFilesOptions {
	/** `client.local` for an instance on this computer, `client.core(id)` for a server. */
	client: MaybeRefOrGetter<ContractClient>
	instanceId: MaybeRefOrGetter<string>
	/** The folder shown, from the instance's folder, without a leading `/`. The host keeps it. */
	currentPath: Ref<string>
	/** Each folder's query key, under the one prefix that every listing of the instance shares. */
	queryKey: (path: string) => QueryKey
	/** The prefix of every listing's key, to mark them all stale. */
	queryKeyPrefix: MaybeRefOrGetter<QueryKey>
	onFailure: (failure: FilesFailure, error: unknown) => void
	/** Called after a file was written from the editor or uploaded. */
	onWritten?: (path: string) => void
	/** Where an upload's progress goes, when the host shows it outside the tab. */
	uploadState?: Ref<UploadState>
}

/** `path` without a leading `/`, as the engine names paths in an instance. */
export function instancePath(path: string): string {
	return path.replace(/^\/+/, '')
}

function join(parent: string, name: string) {
	return parent ? `${parent}/${name}` : name
}

/**
 * An instance's files over the engine's file routes, which an App and a Core both serve: the
 * Files tab's provider for either, given the client that reaches the instance. The host adds what
 * only it has, such as opening a folder or saving a file on this computer.
 */
export function useInstanceFiles(options: InstanceFilesOptions) {
	const queryClient = useQueryClient()
	const client = () => toValue(options.client)
	const params = () => ({ id: toValue(options.instanceId) })

	function directoryQueryOptions(path: string) {
		return {
			queryKey: options.queryKey(path),
			queryFn: () =>
				client().call(instanceFiles.ListInstanceFiles, { params: params(), query: { path } }),
			staleTime: 30_000,
		}
	}

	// A cached listing shows at once. Opening the tab always reads it again in the background,
	// because files change outside the tab.
	const directory = useQuery(
		computed(() => ({
			...directoryQueryOptions(options.currentPath.value),
			enabled: !!toValue(options.instanceId),
			refetchOnMount: 'always' as const,
		})),
	)
	const items = ref<FileItem[]>([])
	watch(
		directory.data,
		(data) => {
			if (data) items.value = data
		},
		{ immediate: true },
	)
	watch(directory.error, (error) => {
		if (error) items.value = []
	})
	/** True until the first listing is shown. */
	const firstPaintPending = computed(
		() => directory.data.value === undefined && !directory.error.value,
	)

	/** Marks every listing of the instance stale and reads the open one again. */
	async function refresh() {
		await queryClient.invalidateQueries({ queryKey: toValue(options.queryKeyPrefix) })
	}

	async function attempt(failure: FilesFailure, change: () => Promise<unknown>) {
		try {
			await change()
		} catch (error) {
			options.onFailure(failure, error)
		} finally {
			await refresh()
		}
	}

	const editingFile = ref<EditingFile | null>(null)

	const readFileAsBlob = async (path: string) => {
		const response = await client().call(instanceFiles.ReadInstanceFile, {
			params: params(),
			query: { path: instancePath(path) },
		})
		return response.blob()
	}

	const uploadState =
		options.uploadState ??
		ref<UploadState>({
			isUploading: false,
			currentFileName: null,
			currentFileProgress: 0,
			uploadedBytes: 0,
			totalBytes: 0,
			completedFiles: 0,
			totalFiles: 0,
		})
	/** Aborts the upload running now, if any. */
	let uploading: AbortController | undefined

	async function uploadFiles(files: File[]) {
		if (files.length === 0) return
		const abort = (uploading = new AbortController())
		uploadState.value = {
			isUploading: true,
			currentFileName: '',
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
				const path = join(options.currentPath.value, file.name)
				await client().call(
					instanceFiles.WriteInstanceFile,
					{ params: params(), query: { path, create_only: false }, body: file },
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
				uploadState.value.currentFileProgress = 1
				options.onWritten?.(path)
			}
		} catch (error) {
			if (!(error instanceof ApiCallError && error.code === 'cancelled'))
				options.onFailure('upload', error)
		} finally {
			if (uploading === abort) uploading = undefined
			uploadState.value.isUploading = false
			await refresh()
		}
	}

	async function zip(body: InstanceZipRequest) {
		await attempt('zip', () =>
			client().call(instanceFiles.ZipInstanceFiles, { params: params(), body }),
		)
	}

	return {
		items,
		loading: directory.isFetching,
		error: computed(() => directory.error.value ?? null),
		firstPaintPending,
		currentPath: options.currentPath,
		navigateTo: (path: string) => {
			options.currentPath.value = instancePath(path)
		},
		prefetchDirectory: (path: string) =>
			void queryClient.prefetchQuery(directoryQueryOptions(instancePath(path))),
		editingFile,
		startEditing: (file: EditingFile) => {
			editingFile.value = file
		},
		stopEditing: () => {
			editingFile.value = null
		},
		createItem: (name: string, type: 'file' | 'directory') =>
			attempt('create', () => {
				const path = join(options.currentPath.value, name)
				return type === 'directory'
					? client().call(instanceFiles.CreateInstanceDirectory, {
							params: params(),
							query: { path },
						})
					: client().call(instanceFiles.WriteInstanceFile, {
							params: params(),
							query: { path, create_only: true },
							body: new Blob([]),
						})
			}),
		renameItem: (path: string, newName: string) =>
			attempt('rename', () => {
				const source = instancePath(path)
				const parent = source.includes('/') ? source.slice(0, source.lastIndexOf('/')) : ''
				return client().call(instanceFiles.MoveInstanceFile, {
					params: params(),
					body: { source, destination: join(parent, newName) },
				})
			}),
		moveItem: (source: string, destination: string) =>
			attempt('move', () =>
				client().call(instanceFiles.MoveInstanceFile, {
					params: params(),
					body: { source: instancePath(source), destination: instancePath(destination) },
				}),
			),
		deleteItem: (path: string, recursive: boolean) =>
			attempt('delete', () =>
				client().call(instanceFiles.DeleteInstanceFile, {
					params: params(),
					query: { path: instancePath(path), recursive },
				}),
			),
		readFile: async (path: string) => (await readFileAsBlob(path)).text(),
		readFileAsBlob,
		writeFile: async (path: string, content: string) => {
			await client().call(instanceFiles.WriteInstanceFile, {
				params: params(),
				query: { path: instancePath(path), create_only: false },
				body: new Blob([content]),
			})
			options.onWritten?.(instancePath(path))
		},
		statFile: (path: string) =>
			client().call(instanceFiles.StatInstanceFile, {
				params: params(),
				query: { path: instancePath(path) },
			}),
		zipFolder: (path: string) => zip({ target_type: 'Directory', path: instancePath(path) }),
		zipPaths: (parent: string, include: string[], target: string) =>
			zip({
				target_type: 'ManyPaths',
				parent: instancePath(parent),
				include,
				target: instancePath(target),
			}),
		uploadFiles,
		cancelUpload: () => uploading?.abort(),
		uploadState,
		extractFile: async (path: string, override: boolean, dry: boolean) => {
			try {
				const result = await client().call(instanceFiles.ExtractInstanceZip, {
					params: params(),
					body: { path: instancePath(path), override_conflicts: override, dry_run: dry },
				})
				return result ?? undefined
			} catch (error) {
				options.onFailure('extract', error)
			} finally {
				if (!dry) await refresh()
			}
		},
		refresh,
	} satisfies Partial<FileManagerContext> & Record<string, unknown>
}
