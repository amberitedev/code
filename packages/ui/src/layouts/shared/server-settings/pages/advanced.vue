<template>
	<div class="relative h-full w-full">
		<div class="flex h-full w-full flex-col gap-4">
			<div class="flex flex-col gap-6">
				<!-- Java version section -->
				<div class="flex flex-col gap-2.5">
					<div class="flex flex-col gap-2">
						<span class="text-lg font-semibold text-contrast">Java version</span>
					</div>
					<div class="relative max-w-xs">
						<Combobox
							:id="'java-version-field'"
							v-model="javaVersion"
							name="java-version"
							:options="displayedJavaVersions"
							:display-value="javaVersionLabel ?? 'Java Version'"
							:disabled="isInstanceLoading || isPending"
						>
							<template #dropdown-footer>
								<button
									class="flex w-full cursor-pointer items-center justify-center gap-1.5 border-0 border-t border-solid border-surface-5 bg-transparent py-3 text-center text-sm font-semibold text-secondary transition-colors hover:text-contrast"
									@mousedown.prevent
									@click="showAllVersions = !showAllVersions"
								>
									<EyeOffIcon v-if="showAllVersions" class="size-4" />
									<EyeIcon v-else class="size-4" />
									{{ showAllVersions ? 'Hide extra versions' : 'Show all versions' }}
								</button>
							</template>
						</Combobox>
						<div
							v-if="isInstanceLoading || isPending"
							class="bg-bg/50 absolute inset-0 flex items-center justify-center rounded-xl"
						>
							<SpinnerIcon class="h-5 w-5 animate-spin text-secondary" />
						</div>
					</div>
					<span> The Java version your server runs on. </span>
				</div>
			</div>
		</div>
		<SaveBanner
			:is-visible="!!hasUnsavedChanges || isPending"
			:server-id="serverId"
			:is-updating="isPending"
			:save="() => saveJavaVersion()"
			:reset="resetJavaVersion"
		/>
	</div>
</template>

<script setup lang="ts">
import { EyeIcon, EyeOffIcon, SpinnerIcon } from '@modrinth/assets'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import { Combobox } from '#ui/components'
import SaveBanner from '#ui/components/servers/SaveBanner.vue'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'

const { addNotification } = injectNotificationManager()
const { serverId } = injectModrinthServerContext()
const coreClient = injectCoreClient()
const queryClient = useQueryClient()

const { data: coreInstance, isLoading: isInstanceLoading } = useQuery({
	queryKey: ['servers', 'detail', 'core', serverId] as const,
	queryFn: () => coreClient.getInstance(serverId),
})

const JAVA_VERSIONS = [
	{ value: 8, label: 'Java 8' },
	{ value: 11, label: 'Java 11' },
	{ value: 17, label: 'Java 17' },
	{ value: 21, label: 'Java 21' },
	{ value: 25, label: 'Java 25' },
]

const showAllVersions = ref(false)

type MinecraftReleaseVersion = {
	major: number
	minor: number
}

function parseMinecraftReleaseVersion(version: string): MinecraftReleaseVersion | null {
	const [majorPart, minorPart] = version.split('.')
	if (!majorPart || !minorPart) return null
	const major = Number(majorPart)
	const minor = Number(minorPart)
	if (!Number.isInteger(major) || !Number.isInteger(minor)) return null
	return { major, minor }
}

function filterJavaVersions(compatibleVersions: number[]) {
	return JAVA_VERSIONS.filter((version) => compatibleVersions.includes(version.value))
}

const displayedJavaVersions = computed(() => {
	if (showAllVersions.value) return JAVA_VERSIONS

	const mcVersion = coreInstance.value?.game_version ?? ''
	if (!mcVersion) return JAVA_VERSIONS

	const releaseVersion = parseMinecraftReleaseVersion(mcVersion)
	if (!releaseVersion) return JAVA_VERSIONS

	if (releaseVersion.major > 1) {
		if (releaseVersion.major >= 26) return filterJavaVersions([25])
		return JAVA_VERSIONS
	}

	if (releaseVersion.minor >= 20) return filterJavaVersions([21])
	if (releaseVersion.minor >= 17) return filterJavaVersions([17, 21])
	if (releaseVersion.minor >= 12) return filterJavaVersions([8, 11, 17, 21])
	if (releaseVersion.minor >= 6) return filterJavaVersions([8, 11])
	return filterJavaVersions([8])
})

const savedJavaVersion = computed(() => coreInstance.value?.java_version ?? undefined)
const javaVersion = ref<number>()

const javaVersionLabel = computed(
	() => JAVA_VERSIONS.find((v) => v.value === javaVersion.value)?.label,
)

function syncFormFromData() {
	javaVersion.value = savedJavaVersion.value
}

watch(
	coreInstance,
	(newData, oldData) => {
		if (newData && !oldData) {
			syncFormFromData()
		}
	},
	{ immediate: true },
)

const hasUnsavedChanges = computed(() => javaVersion.value !== savedJavaVersion.value)

const { mutate: saveJavaVersion, isPending } = useMutation({
	mutationFn: () => coreClient.updateJavaVersion(serverId, javaVersion.value ?? null),
	onSuccess: async () => {
		await queryClient.invalidateQueries({ queryKey: ['servers', 'detail', 'core', serverId] })
		syncFormFromData()
		addNotification({
			type: 'success',
			title: 'Server settings updated',
			text: 'Your server settings were successfully changed.',
		})
	},
	onError: (error) => {
		console.error(error)
		addNotification({
			type: 'error',
			title: 'Failed to update server settings',
			text: 'Please try again later.',
		})
	},
})

function resetJavaVersion() {
	syncFormFromData()
}
</script>
