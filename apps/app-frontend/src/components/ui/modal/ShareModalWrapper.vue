<!-- @deprecated Use ShareModal from @modrinth/ui directly. Ads/noblur now handled by injectModalBehavior. -->
<script setup lang="ts">
import { ShareModal } from '@modrinth/ui'
import { ref } from 'vue'

defineProps({
	header: {
		type: String,
		default: 'Share',
	},
	shareTitle: {
		type: String,
		default: 'Modrinth',
	},
	shareText: {
		type: String,
		default: null,
	},
	link: {
		type: Boolean,
		default: false,
	},
	openInNewTab: {
		type: Boolean,
		default: true,
	},
})

const modal = ref<InstanceType<typeof ShareModal>>(null!)

defineExpose({
	show: (passedContent: unknown) => {
		modal.value.show(passedContent)
	},
	hide: () => {
		// @ts-expect-error ShareModal exposes no hide, so upstream's call throws
		modal.value.hide()
	},
})
</script>

<template>
	<ShareModal
		ref="modal"
		:header="header"
		:share-title="shareTitle"
		:share-text="shareText"
		:link="link"
		:open-in-new-tab="openInNewTab"
	/>
</template>
