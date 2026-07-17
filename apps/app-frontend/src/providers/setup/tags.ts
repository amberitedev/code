import { provideTags } from '@modrinth/ui'
import { ref } from 'vue'

import { get_game_versions, get_loaders } from '@/helpers/tags'

export function setupTagsProvider() {
	const gameVersions = ref([])
	const loaders = ref([])
	provideTags({ gameVersions, loaders })

	return async () => {
		const [versions, loaderTypes] = await Promise.allSettled([get_game_versions(), get_loaders()])
		if (versions.status === 'fulfilled') gameVersions.value = versions.value
		else console.warn('Game version metadata is unavailable.')
		if (loaderTypes.status === 'fulfilled') loaders.value = loaderTypes.value
		else console.warn('Loader metadata is unavailable.')
	}
}
