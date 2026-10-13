<script setup lang="ts">
import { SkinButton } from '@modrinth/ui'
import { useIntersectionObserver } from '@vueuse/core'
import { type ComponentPublicInstance, computed, ref, watch } from 'vue'

import { acquireSkinPreview, getSkinPreviewKey } from '@/helpers/rendering/skin-previews'
import type { Cape, Skin } from '@/helpers/skins'

const props = defineProps<{
	skin: Skin
	capes: Cape[]
	selected: boolean
}>()
const button = ref<ComponentPublicInstance>()
const visible = ref(false)
const url = ref<string>()
const key = computed(() => getSkinPreviewKey(props.skin))

useIntersectionObserver(
	button,
	([entry]) => {
		visible.value = entry?.isIntersecting ?? false
	},
	{ rootMargin: '200px' },
)

watch(
	[visible, key, () => props.capes],
	([isVisible], _, onCleanup) => {
		if (!isVisible) {
			url.value = undefined
			return
		}
		const preview = acquireSkinPreview(props.skin, props.capes)
		url.value = preview.result?.forwards
		let active = true
		onCleanup(() => {
			active = false
			preview.release()
		})
		void preview.promise
			.then((result) => {
				if (active) url.value = result?.forwards
			})
			.catch((error) => console.warn('Could not load skin preview', error))
	},
	{ immediate: true },
)
</script>

<template>
	<SkinButton ref="button" :selected="selected" :forward-image-src="url">
		<template v-for="(_, name) in $slots" #[name]="slotProps">
			<slot :name="name" v-bind="slotProps ?? {}" />
		</template>
	</SkinButton>
</template>
