import type { UploadState } from '@modrinth/api-client'
import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import ServerPanelAdmonitions from '../../components/servers/admonitions/ServerPanelAdmonitions.vue'
import { defineMessage } from '../../composables/i18n'
import type { FileOperation } from '../../layouts/shared/files-tab/types'
import { provideModrinthServerContext } from '../../providers'
import type { CancelUploadHandler } from '../../providers/server-context'
import { storyServerContext } from './story-server-context'

const meta = {
	title: 'Servers/ServerPanelAdmonitions',
	component: ServerPanelAdmonitions,
	parameters: {
		layout: 'padded',
	},
	decorators: [
		(story) => ({
			components: { story },
			setup() {
				const router = useRouter()
				onMounted(() => {
					router.replace('/hosting/manage/demo-server/content')
				})

				const uploadState = ref<UploadState>({
					isUploading: true,
					currentFileName: 'resourcepack.zip',
					currentFileProgress: 0.2,
					uploadedBytes: 20_000,
					totalBytes: 100_000,
					completedFiles: 1,
					totalFiles: 3,
				})

				const fileOp = ref<FileOperation[]>([
					{
						id: 'fs-op-1',
						op: 'extract',
						src: 'story-modpack.mrpack',
						state: 'running',
						progress: 0.35,
						bytes_processed: 2_000_000,
					},
					{
						id: 'fs-op-2',
						op: 'zip',
						src: 'world.zip',
						state: 'failure-error',
						progress: 0,
						cancellable: false,
						error: 'ZIP operation ended before completion',
					},
				])

				provideModrinthServerContext(
					storyServerContext({
						busyReasons: computed(() => [
							{ reason: defineMessage({ id: 's.bg', defaultMessage: 'Background task running' }) },
						]),
						uploadState,
						cancelUpload: ref<CancelUploadHandler | null>(async () => {
							await new Promise((resolve) => setTimeout(resolve, 1200))
							uploadState.value = { ...uploadState.value, isUploading: false }
						}),
						activeOperations: computed(() => fileOp.value),
						dismissOperation: async (id) => {
							fileOp.value = fileOp.value.filter((o) => o.id !== id)
						},
					}),
				)
				return {}
			},
			template: '<div style="max-width: 720px"><story /></div>',
		}),
	],
} satisfies Meta<typeof ServerPanelAdmonitions>

export default meta

type Story = StoryObj<typeof meta>

export const WithUploadFileOpAndBusy: Story = {}
