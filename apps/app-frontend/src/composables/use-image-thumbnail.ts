import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { localThumbnailUrl } from '@/platform/local'

export function useImageThumbnail(
	path: MaybeRefOrGetter<string | null | undefined>,
	size: MaybeRefOrGetter<number>,
	revision: MaybeRefOrGetter<string | number | undefined> = undefined,
) {
	return computed(() => {
		const source = toValue(path)
		return source ? localThumbnailUrl(source, toValue(size), toValue(revision)) : undefined
	})
}
