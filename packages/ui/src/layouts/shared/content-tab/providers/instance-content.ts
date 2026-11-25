import {
	type CacheBehaviour,
	type ContentItem as EngineContentItem,
	instanceContent,
	type LinkedModpackInfo,
} from '@amberite/contracts/engine'
import type { ContractClient } from '@modrinth/api-client'
import { type QueryKey, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import type { ContentSide } from '../types'
import { type ContentItemMapping, toContentItem } from '../utils/content-item'

/** An instance's own content files and the modpack it is linked to, as the engine lists them. */
export interface InstanceContent {
	items: EngineContentItem[]
	modpack: LinkedModpackInfo | null
}

export interface InstanceContentQueryOptions {
	/** `client.local` for an instance on this computer, `client.core(id)` for a server. */
	client: MaybeRefOrGetter<ContractClient>
	instanceId: MaybeRefOrGetter<string>
	queryKeys: {
		content: (instanceId: string) => QueryKey
		packContent: (instanceId: string) => QueryKey
	}
}

/** The queries of an instance's content, for a host that reads them outside the content tab. */
export function instanceContentQueries(options: InstanceContentQueryOptions) {
	const client = toValue(options.client)
	const id = toValue(options.instanceId)
	const read = (cacheBehaviour?: CacheBehaviour) => ({
		params: { id },
		query: { cacheBehaviour },
	})
	return {
		content: (cacheBehaviour?: CacheBehaviour) => ({
			queryKey: options.queryKeys.content(id),
			queryFn: async (): Promise<InstanceContent> => {
				const [items, modpack] = await Promise.all([
					client.call(instanceContent.ListContentItems, read(cacheBehaviour)),
					client.call(instanceContent.GetLinkedModpack, read(cacheBehaviour)),
				])
				return { items, modpack }
			},
			networkMode: 'always' as const,
			staleTime: 30_000,
		}),
		/** The files of the modpack (or shared instance) the instance follows. */
		packContent: (cacheBehaviour?: CacheBehaviour) => ({
			queryKey: options.queryKeys.packContent(id),
			queryFn: () => client.call(instanceContent.ListLinkedModpackContent, read(cacheBehaviour)),
			networkMode: 'always' as const,
		}),
	}
}

export interface InstanceContentOptions extends InstanceContentQueryOptions {
	mapping?: ContentItemMapping
	/** Whether to read the linked pack's files; by default when the instance has a modpack. */
	packContentEnabled?: (modpack: LinkedModpackInfo | null | undefined) => boolean
}

/**
 * An instance's content over the engine's content routes, which an App and a Core both serve: the
 * Content tab's provider for either, given the client that reaches the instance. `items` and
 * `packItems` are the content tab's items; the queries hold the engine's own. The host keeps
 * what only it has, such as synced packs on this computer or a server's followed instance.
 */
export function useInstanceContent(options: InstanceContentOptions) {
	const queryClient = useQueryClient()
	const queries = () => instanceContentQueries(options)
	const client = () => toValue(options.client)
	const params = () => ({ id: toValue(options.instanceId) })

	const content = useQuery(
		computed(() => ({ ...queries().content(), enabled: !!toValue(options.instanceId) })),
	)
	const packContent = useQuery(
		computed(() => ({
			...queries().packContent(),
			enabled: (options.packContentEnabled ?? ((modpack) => !!modpack))(
				content.data.value?.modpack,
			),
		})),
	)
	const map = (items: EngineContentItem[] | undefined, fromPack: boolean) =>
		items?.map((item) => toContentItem(item, options.mapping, fromPack))

	return {
		content,
		packContent,
		items: computed(() => map(content.data.value?.items, false)),
		packItems: computed(() => map(packContent.data.value, true)),
		toContentItem: (item: EngineContentItem, fromPack = false) =>
			toContentItem(item, options.mapping, fromPack),
		/** Reads the content again; `cacheBehaviour` says whether to ask Modrinth again too. */
		fetch: (cacheBehaviour?: CacheBehaviour, staleTime = 0) =>
			queryClient.fetchQuery({ ...queries().content(cacheBehaviour), staleTime }),
		fetchPackContent: (cacheBehaviour?: CacheBehaviour) =>
			queryClient.fetchQuery({ ...queries().packContent(cacheBehaviour), staleTime: 0 }),
		/** Marks both lists stale, reading again the ones on screen. */
		refresh: async () => {
			const id = toValue(options.instanceId)
			await Promise.all([
				queryClient.invalidateQueries({ queryKey: options.queryKeys.content(id) }),
				queryClient.invalidateQueries({ queryKey: options.queryKeys.packContent(id) }),
			])
		},
		/** Turns a file on or off, which renames it; resolves to its new path. */
		toggle: (path: string, enabled?: boolean) =>
			client().call(instanceContent.ToggleProject, {
				params: params(),
				body: { project_path: path, desired_enabled: enabled },
			}),
		setSide: (path: string, side: ContentSide, enabled: boolean) =>
			client().call(instanceContent.SetSide, {
				params: params(),
				body: { project_path: path, side, enabled },
			}),
		remove: (path: string) =>
			client().call(instanceContent.RemoveProject, {
				params: params(),
				body: { project_path: path },
			}),
		/** Installs another version of a file's project, with its dependencies; resolves to its path. */
		switchVersion: (path: string, versionId: string) =>
			client().call(instanceContent.SwitchProjectVersion, {
				params: params(),
				body: { project_path: path, version_id: versionId },
			}),
		setLocked: (path: string, locked: boolean) =>
			client().call(instanceContent.SetProjectLocked, {
				params: params(),
				body: { project_path: path, locked },
			}),
		updateModpack: (versionId: string) =>
			client().call(instanceContent.UpdateModpackVersion, {
				params: params(),
				body: { version_id: versionId },
			}),
		unlinkModpack: () => client().call(instanceContent.UnlinkModpack, { params: params() }),
	}
}
