<template>
	<div class="relative h-full w-full">
		<div class="flex h-full w-full flex-col gap-4">
			<div class="flex flex-col gap-6">
				<!-- SFTP section (Core has no SFTP yet — markup kept for later) -->
				<div v-if="SFTP_ENABLED" class="flex flex-col gap-2">
					<div class="flex flex-col items-center justify-between gap-0.5 sm:flex-row">
						<span class="text-lg font-semibold text-contrast">SFTP</span>
						<ButtonStyled>
							<a
								v-tooltip="'This button only works with compatible SFTP clients (e.g. WinSCP)'"
								class="!w-full sm:!w-auto"
								:href="sftpUrl"
								target="_blank"
							>
								<ExternalIcon class="h-5 w-5" />
								Launch SFTP
							</a>
						</ButtonStyled>
					</div>

					<div class="flex flex-col gap-2.5 rounded-2xl bg-surface-2 p-4">
						<span class="text-lg font-semibold text-contrast">Server Address</span>
						<div
							v-tooltip="'Copy SFTP server address'"
							class="copy-field hover:bg-button-bg-hover"
							@click="copyToClipboard('Server address', server?.sftp_host)"
						>
							<span class="cursor-pointer font-semibold text-primary">
								{{ server?.sftp_host }}
							</span>
							<div class="grid h-10 w-10 place-content-center">
								<CopyIcon class="h-5 w-5" />
							</div>
						</div>
						<div class="flex flex-col gap-2 sm:mt-0 sm:flex-row">
							<div class="flex w-full flex-col justify-center gap-2">
								<span class="text-lg font-semibold text-contrast">Username</span>
								<div
									v-tooltip="'Copy SFTP username'"
									class="copy-field hover:bg-button-bg-hover"
									@click="copyToClipboard('Username', server?.sftp_username)"
								>
									<div class="truncate font-semibold">
										{{ server?.sftp_username }}
									</div>
									<div class="grid h-10 w-9 place-content-center">
										<CopyIcon class="h-5 w-5" />
									</div>
								</div>
							</div>
							<div class="flex w-full flex-col justify-center gap-2">
								<span class="text-lg font-semibold text-contrast">Password</span>
								<div
									class="copy-field-has-button [&:hover:not(:has(button:hover))]:bg-button-bg-hover"
									@click="copyToClipboard('Password', server?.sftp_password)"
								>
									<div class="flex items-center gap-1.5 h-full w-full">
										<div
											v-tooltip="'Copy SFTP Password'"
											class="h-full flex justify-between grow items-center"
										>
											<div class="truncate font-semibold">
												{{
													showPassword
														? server?.sftp_password
														: '*'.repeat(server?.sftp_password?.length ?? 0)
												}}
											</div>
											<CopyIcon class="h-5 w-5" />
										</div>

										<ButtonStyled type="transparent" circular>
											<button
												v-tooltip="showPassword ? 'Hide password' : 'Show password'"
												class="hover:bg-button-bg-hover grid h-10 w-10 place-content-center rounded-lg"
												@click.stop="showPassword = !showPassword"
											>
												<EyeIcon v-if="showPassword" class="h-5 w-5" />
												<EyeOffIcon v-else class="h-5 w-5" />
											</button>
										</ButtonStyled>
									</div>
								</div>
							</div>
						</div>
					</div>
				</div>

				<!-- Startup command section (bound to Core JVM arguments) -->
				<div class="flex flex-col gap-2.5">
					<div class="flex h-10 flex-col items-end justify-between gap-4 sm:flex-row">
						<label for="startup-command-field" class="mb-0.5 flex flex-col gap-2">
							<span class="text-lg font-semibold text-contrast">JVM arguments</span>
						</label>
						<ButtonStyled v-if="startupCommand !== defaultStartupCommand" type="transparent">
							<button
								:disabled="isStartupLoading || startupCommand === defaultStartupCommand"
								class="relative !w-full sm:!w-auto"
								@click="resetToDefault"
							>
								<UpdatedIcon class="h-5 w-5" />
								Default
							</button>
						</ButtonStyled>
					</div>
					<div class="relative">
						<StyledInput
							id="startup-command-field"
							v-model="startupCommand"
							multiline
							resize="vertical"
							input-class="font-mono field-sizing-content"
							:disabled="isStartupLoading"
						/>
						<div
							v-if="isStartupLoading"
							class="bg-bg/50 absolute inset-0 flex items-center justify-center rounded-xl"
						>
							<SpinnerIcon class="h-6 w-6 animate-spin text-secondary" />
						</div>
					</div>
					<span> Extra JVM arguments passed to Java when your server starts. </span>
				</div>

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
							:disabled="isStartupLoading"
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
							v-if="isStartupLoading"
							class="bg-bg/50 absolute inset-0 flex items-center justify-center rounded-xl"
						>
							<SpinnerIcon class="h-5 w-5 animate-spin text-secondary" />
						</div>
					</div>
					<span> The Java version your server runs on. </span>
				</div>

				<!-- Java runtime section (Core picks the runtime — markup kept for later) -->
				<div v-if="JRE_VENDOR_ENABLED" class="flex flex-col gap-2.5">
					<div class="flex flex-col gap-2">
						<span class="text-lg font-semibold text-contrast">Java runtime</span>
					</div>
					<div class="relative max-w-xs">
						<Combobox
							:id="'runtime-field'"
							v-model="jreVendor"
							name="runtime"
							:options="JRE_VENDORS"
							:display-value="jreVendorLabel ?? 'Runtime'"
							:disabled="isStartupLoading"
						/>
						<div
							v-if="isStartupLoading"
							class="bg-bg/50 absolute inset-0 flex items-center justify-center rounded-xl"
						>
							<SpinnerIcon class="h-5 w-5 animate-spin text-secondary" />
						</div>
					</div>
					<span> The Java runtime your server will use. </span>
				</div>
			</div>
		</div>
		<SaveBanner
			:is-visible="!!hasUnsavedChanges || isPending"
			:server-id="serverId"
			:is-updating="isPending"
			:save="() => saveStartup()"
			:reset="resetStartup"
		/>
	</div>
</template>

<script setup lang="ts">
import {
	CopyIcon,
	ExternalIcon,
	EyeIcon,
	EyeOffIcon,
	SpinnerIcon,
	UpdatedIcon,
} from '@modrinth/assets'
import {
	ButtonStyled,
	Combobox,
	injectModrinthServerContext,
	injectNotificationManager,
	StyledInput,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

import { injectCoreServerContext } from '../../core-server-instance'
import SaveBanner from '../SaveBanner.vue'

/**
 * Forked verbatim from the hosting server-settings Advanced page. Backend swaps:
 * the freeform "startup command" field maps to Core's `jvm_args` override (the
 * only round-trippable freeform launch field — memory has its own control and
 * Core renders the full command itself), and Java version is persisted through
 * `updateJavaVersion`. SFTP and the JRE-vendor picker have no Core equivalent yet,
 * so their markup is retained but gated behind disabled flags for the future.
 */
const SFTP_ENABLED = false
const JRE_VENDOR_ENABLED = false

const { addNotification } = injectNotificationManager()
const { server, serverId, worldId } = injectModrinthServerContext()
const core = useCoreClient()
const { refreshServer } = injectCoreServerContext()
const queryClient = useQueryClient()

// SFTP state
const showPassword = ref(false)
const sftpUrl = computed(() => `sftp://${server.value?.sftp_username}@${server.value?.sftp_host}`)

const copyToClipboard = (name: string, textToCopy?: string) => {
	navigator.clipboard.writeText(textToCopy || '')
	addNotification({
		type: 'success',
		title: `${name} copied to clipboard!`,
	})
}

// Startup state
const startupQueryKey = computed(() => ['servers', 'startup', 'v1', serverId, worldId.value])

const { data: startupData, isLoading: isStartupLoading } = useQuery({
	queryKey: startupQueryKey,
	queryFn: () => core.getStartup(serverId),
	enabled: computed(() => worldId.value !== null),
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

	const mcVersion = server.value?.mc_version ?? ''
	if (!mcVersion) return JAVA_VERSIONS

	const releaseVersion = parseMinecraftReleaseVersion(mcVersion)
	if (!releaseVersion) return JAVA_VERSIONS

	if (releaseVersion.major > 1) {
		if (releaseVersion.major >= 26) {
			return filterJavaVersions([25])
		}

		return JAVA_VERSIONS
	}

	if (releaseVersion.minor >= 20) return filterJavaVersions([21])
	if (releaseVersion.minor >= 17) return filterJavaVersions([17, 21])
	if (releaseVersion.minor >= 12) return filterJavaVersions([8, 11, 17, 21])
	if (releaseVersion.minor >= 6) return filterJavaVersions([8, 11])
	return filterJavaVersions([8])
})

type JreVendor = 'corretto' | 'temurin' | 'graal'
const JRE_VENDORS: { value: JreVendor; label: string }[] = [
	{ value: 'corretto', label: 'Corretto' },
	{ value: 'temurin', label: 'Temurin' },
	{ value: 'graal', label: 'GraalVM' },
]

const savedStartupCommand = computed(() => startupData.value?.jvm_args ?? '')
const savedJavaVersion = computed(() => startupData.value?.java_version ?? undefined)
const savedJreVendor = computed<JreVendor | undefined>(() => undefined)
const defaultStartupCommand = computed(() => '')

const startupCommand = ref('')
const javaVersion = ref<number>()
const jreVendor = ref<JreVendor>()

const javaVersionLabel = computed(
	() => JAVA_VERSIONS.find((v) => v.value === javaVersion.value)?.label,
)
const jreVendorLabel = computed(() => JRE_VENDORS.find((v) => v.value === jreVendor.value)?.label)

function syncFormFromData() {
	startupCommand.value = savedStartupCommand.value
	javaVersion.value = savedJavaVersion.value
	jreVendor.value = savedJreVendor.value
}

watch(
	startupData,
	(newData, oldData) => {
		if (newData && !oldData) {
			syncFormFromData()
		}
	},
	{ immediate: true },
)

const hasUnsavedChanges = computed(
	() =>
		startupCommand.value !== savedStartupCommand.value ||
		javaVersion.value !== savedJavaVersion.value ||
		jreVendor.value !== savedJreVendor.value,
)

const { mutate: saveStartup, isPending } = useMutation({
	mutationFn: async () => {
		if (javaVersion.value !== savedJavaVersion.value) {
			await core.updateJavaVersion(serverId, javaVersion.value ?? null)
		}
		await core.updateStartup(serverId, { jvm_args: startupCommand.value || null })
	},
	onSuccess: async () => {
		await queryClient.invalidateQueries({ queryKey: startupQueryKey.value })
		await refreshServer()
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
			title: 'Failed to update server arguments',
			text: 'Please try again later.',
		})
	},
})

function resetStartup() {
	syncFormFromData()
}

function resetToDefault() {
	startupCommand.value = defaultStartupCommand.value
}
</script>

<style scoped>
.copy-field {
	@apply flex h-10 cursor-pointer items-center justify-between gap-2 rounded-lg bg-button-bg px-3 pr-1.5 transition-all;
	@apply hover:brightness-125 active:scale-95;
}

.copy-field-has-button {
	@apply flex h-10 cursor-pointer items-center justify-between gap-2 rounded-lg bg-button-bg px-3 pr-1.5 transition-all;
	@apply [&:hover:not(:has(button:hover))]:brightness-125 [&:active:not(:has(button:active))]:scale-95;
}
</style>
