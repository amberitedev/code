import type { Server } from '@amberite/contracts/core'
import { ContractClient, type UploadState } from '@modrinth/api-client'
import { computed, ref } from 'vue'

import type { CancelUploadHandler, ModrinthServerContext } from '../../providers/server-context'

/** A running server on a Core that answers nothing, for stories to override. */
export function storyServerContext(
	overrides: Partial<ModrinthServerContext> & { server?: Partial<Server> } = {},
): ModrinthServerContext {
	const { server, ...rest } = overrides
	const core = new ContractClient({
		base: 'http://story.invalid',
		headers: async () => ({}),
		fetch: async () => new Response(null, { status: 503 }),
	})
	return {
		serverId: 'story-server',
		coreId: 'story-core',
		core,
		server: ref<Server>({
			server_id: 'story-server',
			name: 'Story server',
			owner_id: null,
			net: { ip: '127.0.0.1', port: 25565, outside: null, game_port: 25565 },
			status: 'available',
			loader: 'Fabric',
			loader_version: '0.16.0',
			mc_version: '1.21.1',
			upstream: null,
			flows: { intro: false },
			icon: null,
			power_state: 'running',
			linked_instance: null,
			shared_instance_id: null,
			reachability: null,
			players: null,
			memory_mb: 4096,
			created_at: '2026-01-01T00:00:00Z',
			update_channel: 'release',
			eula_accepted: true,
			...server,
		}),
		isConnected: ref(true),
		powerState: ref('running'),
		isServerRunning: computed(() => true),
		uptimeSeconds: ref(0),
		installation: computed(() => null),
		beginInstallation: () => {},
		cancelOptimisticInstallation: () => {},
		dismissInstallation: () => {},
		busyReasons: computed(() => []),
		uploadState: ref<UploadState>({
			isUploading: false,
			currentFileName: null,
			currentFileProgress: 0,
			uploadedBytes: 0,
			totalBytes: 0,
			completedFiles: 0,
			totalFiles: 0,
		}),
		cancelUpload: ref<CancelUploadHandler | null>(null),
		activeOperations: computed(() => []),
		dismissOperation: async () => {},
		...rest,
	}
}
