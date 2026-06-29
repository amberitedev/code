<script setup>
import {
	ClipboardCopyIcon,
	EyeIcon,
	FolderOpenIcon,
	PlayIcon,
	PlusIcon,
	SearchIcon,
	StopCircleIcon,
	TrashIcon,
} from '@modrinth/assets'
import {
	Accordion,
	DropdownSelect,
	formatLoader,
	injectNotificationManager,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import dayjs from 'dayjs'
import { computed, ref } from 'vue'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import Instance from '@/components/ui/Instance.vue'
import ConfirmDeleteInstanceModal from '@/components/ui/modal/ConfirmDeleteInstanceModal.vue'
import { duplicate, remove } from '@/helpers/profile.js'

const { handleError } = injectNotificationManager()

const { formatMessage } = useVIntl()

const props = defineProps({
	instances: {
		type: Array,
		default() {
			return []
		},
	},
	label: {
		type: String,
		default: '',
	},
	contentKey: {
		type: [String, Number],
		default: 'grid-display-static',
	},
	contentDirection: {
		type: String,
		default: 'forward',
	},
	contentVisible: {
		type: Boolean,
		default: true,
	},
	animationMode: {
		type: String,
		default: 'subtle-page',
	},
})
const instanceOptions = ref(null)
const instanceComponents = ref(null)

const currentDeleteInstance = ref(null)
const confirmModal = ref(null)

async function deleteProfile() {
	if (currentDeleteInstance.value) {
		instanceComponents.value = instanceComponents.value.filter(
			(x) => x.instance.path !== currentDeleteInstance.value,
		)
		await remove(currentDeleteInstance.value).catch(handleError)
	}
}

async function duplicateProfile(p) {
	await duplicate(p).catch(handleError)
}

const handleRightClick = (event, profilePathId) => {
	const item = instanceComponents.value.find((x) => x.instance.path === profilePathId)
	const baseOptions = [
		{ name: 'add_content' },
		{ type: 'divider' },
		{ name: 'edit' },
		{ name: 'duplicate' },
		{ name: 'open' },
		{ name: 'copy' },
		{ type: 'divider' },
		{
			name: 'delete',
			color: 'danger',
		},
	]

	instanceOptions.value.showMenu(
		event,
		item,
		item.playing
			? [
					{
						name: 'stop',
						color: 'danger',
					},
					...baseOptions,
				]
			: [
					{
						name: 'play',
						color: 'primary',
					},
					...baseOptions,
				],
	)
}

const handleOptionsClick = async (args) => {
	switch (args.option) {
		case 'play':
			args.item.play(null, 'InstanceGridContextMenu')
			break
		case 'stop':
			args.item.stop(null, 'InstanceGridContextMenu')
			break
		case 'add_content':
			await args.item.addContent()
			break
		case 'edit':
			await args.item.seeInstance()
			break
		case 'duplicate':
			if (args.item.instance.install_stage == 'installed')
				await duplicateProfile(args.item.instance.path)
			break
		case 'open':
			await args.item.openFolder()
			break
		case 'copy':
			await navigator.clipboard.writeText(args.item.instance.path)
			break
		case 'delete':
			currentDeleteInstance.value = args.item.instance.path
			confirmModal.value.show()
			break
	}
}

const state = useStorage(
	`${props.label}-grid-display-state`,
	{
		group: 'Group',
		sortBy: 'Name',
		collapsedGroups: [],
	},
	localStorage,
	{ mergeDefaults: true },
)

const search = ref('')
const collapsedSectionKeys = computed(() => new Set(state.value.collapsedGroups ?? []))

const getSectionKey = (sectionName) => `${state.value.group}:${sectionName}`

const isSectionCollapsed = (sectionName) => {
	return collapsedSectionKeys.value.has(getSectionKey(sectionName))
}

const setSectionCollapsed = (sectionName, collapsed) => {
	const sectionKey = getSectionKey(sectionName)
	const collapsedSections = new Set(state.value.collapsedGroups ?? [])

	if (collapsed) {
		collapsedSections.add(sectionKey)
	} else {
		collapsedSections.delete(sectionKey)
	}

	state.value.collapsedGroups = [...collapsedSections]
}

const filteredResults = computed(() => {
	const { group = 'Group', sortBy = 'Name' } = state.value

	const instances = props.instances.filter((instance) => {
		return instance.name.toLowerCase().includes(search.value.toLowerCase())
	})

	if (sortBy === 'Name') {
		instances.sort((a, b) => {
			return a.name.localeCompare(b.name)
		})
	}

	if (sortBy === 'Game version') {
		instances.sort((a, b) => {
			return a.game_version.localeCompare(b.game_version, undefined, { numeric: true })
		})
	}

	if (sortBy === 'Last played') {
		instances.sort((a, b) => {
			return dayjs(b.last_played ?? 0).diff(dayjs(a.last_played ?? 0))
		})
	}

	if (sortBy === 'Date created') {
		instances.sort((a, b) => {
			return dayjs(b.date_created).diff(dayjs(a.date_created))
		})
	}

	if (sortBy === 'Date modified') {
		instances.sort((a, b) => {
			return dayjs(b.date_modified).diff(dayjs(a.date_modified))
		})
	}

	const instanceMap = new Map()

	if (group === 'Loader') {
		instances.forEach((instance) => {
			const loader = formatLoader(formatMessage, instance.loader)
			if (!instanceMap.has(loader)) {
				instanceMap.set(loader, [])
			}

			instanceMap.get(loader).push(instance)
		})
	} else if (group === 'Game version') {
		instances.forEach((instance) => {
			if (!instanceMap.has(instance.game_version)) {
				instanceMap.set(instance.game_version, [])
			}

			instanceMap.get(instance.game_version).push(instance)
		})
	} else if (group === 'Group') {
		instances.forEach((instance) => {
			const categories = instance.groups.length === 0 ? ['None'] : instance.groups

			for (const category of categories) {
				if (!instanceMap.has(category)) {
					instanceMap.set(category, [])
				}

				instanceMap.get(category).push(instance)
			}
		})
	} else {
		return instanceMap.set('None', instances)
	}

	// For 'name', we intuitively expect the sorting to apply to the name of the group first, not just the name of the instance
	// ie: Category A should come before B, even if the first instance in B comes before the first instance in A
	if (sortBy === 'Name') {
		const sortedEntries = [...instanceMap.entries()].sort((a, b) => {
			// None should always be first
			if (a[0] === 'None' && b[0] !== 'None') {
				return -1
			}
			if (a[0] !== 'None' && b[0] === 'None') {
				return 1
			}
			return a[0].localeCompare(b[0])
		})
		instanceMap.clear()
		sortedEntries.forEach((entry) => {
			instanceMap.set(entry[0], entry[1])
		})
	}
	// default sorting would do 1.20.4 < 1.8.9 because 2 < 8
	// localeCompare with numeric=true puts 1.8.9 < 1.20.4 because 8 < 20
	if (group === 'Game version') {
		const sortedEntries = [...instanceMap.entries()].sort((a, b) => {
			return a[0].localeCompare(b[0], undefined, { numeric: true })
		})
		instanceMap.clear()
		sortedEntries.forEach((entry) => {
			instanceMap.set(entry[0], entry[1])
		})
	}

	return instanceMap
})

const contentTransitionName = computed(() =>
	[
		'grid-display',
		props.animationMode,
		props.contentDirection === 'backward' || props.contentDirection === 'left'
			? 'backward'
			: 'forward',
	].join('-'),
)
const usesCardTransition = computed(() => props.animationMode === 'card-changes')
const usesPageTransition = computed(() =>
	['subtle-page', 'in-only', 'push'].includes(props.animationMode),
)
const pageContentKey = computed(() =>
	usesPageTransition.value ? props.contentKey : 'grid-display-stable-content',
)
</script>
<template>
	<div class="grid-display">
		<div class="grid-display-controls-stage">
			<Transition :name="contentTransitionName">
				<div
					v-if="contentVisible"
					:key="`${pageContentKey}:controls`"
					class="grid-display-controls-frame"
				>
					<div class="grid-display-controls">
						<StyledInput
							v-model="search"
							:icon="SearchIcon"
							type="text"
							placeholder="Search"
							clearable
							wrapper-class="flex-1"
						/>
						<DropdownSelect
							v-slot="{ selected }"
							v-model="state.sortBy"
							name="Sort Dropdown"
							class="max-w-[16rem]"
							:options="['Name', 'Last played', 'Date created', 'Date modified', 'Game version']"
							placeholder="Select..."
						>
							<span class="font-semibold text-primary">Sort by: </span>
							<span class="font-semibold text-secondary">{{ selected }}</span>
						</DropdownSelect>
						<DropdownSelect
							v-slot="{ selected }"
							v-model="state.group"
							class="max-w-[16rem]"
							name="Group Dropdown"
							:options="['Group', 'Loader', 'Game version', 'None']"
							placeholder="Select..."
						>
							<span class="font-semibold text-primary">Group by: </span>
							<span class="font-semibold text-secondary">{{ selected }}</span>
						</DropdownSelect>
					</div>
				</div>
			</Transition>
		</div>
		<div class="grid-display-results-stage">
			<Transition :name="contentTransitionName">
				<div v-if="contentVisible" :key="pageContentKey" class="grid-display-results-frame">
					<div class="grid-display-results">
						<Accordion
							v-for="instanceSection in Array.from(filteredResults, ([key, value]) => ({
								key,
								value,
							}))"
							:key="instanceSection.key"
							:divider="instanceSection.key !== 'None'"
							:open-by-default="!isSectionCollapsed(instanceSection.key)"
							class="row"
							@on-open="setSectionCollapsed(instanceSection.key, false)"
							@on-close="setSectionCollapsed(instanceSection.key, true)"
						>
							<template v-if="instanceSection.key !== 'None'" #title>
								<span class="text-base">{{ instanceSection.key }}</span>
							</template>
							<TransitionGroup
								v-if="usesCardTransition"
								name="grid-display-card"
								tag="section"
								class="instances"
							>
								<Instance
									v-for="instance in instanceSection.value"
									ref="instanceComponents"
									:key="instance.path + instance.install_stage"
									:instance="instance"
									@contextmenu.prevent.stop="(event) => handleRightClick(event, instance.path)"
								/>
							</TransitionGroup>
							<section v-else class="instances">
								<Instance
									v-for="instance in instanceSection.value"
									ref="instanceComponents"
									:key="instance.path + instance.install_stage"
									:instance="instance"
									@contextmenu.prevent.stop="(event) => handleRightClick(event, instance.path)"
								/>
							</section>
						</Accordion>
					</div>
				</div>
			</Transition>
		</div>
		<ConfirmDeleteInstanceModal ref="confirmModal" @delete="deleteProfile" />
		<ContextMenu ref="instanceOptions" @option-clicked="handleOptionsClick">
			<template #play> <PlayIcon /> Play </template>
			<template #stop> <StopCircleIcon /> Stop </template>
			<template #add_content> <PlusIcon /> Add content </template>
			<template #edit> <EyeIcon /> View instance </template>
			<template #duplicate> <ClipboardCopyIcon /> Duplicate instance</template>
			<template #delete> <TrashIcon /> Delete </template>
			<template #open> <FolderOpenIcon /> Open folder </template>
			<template #copy> <ClipboardCopyIcon /> Copy path </template>
		</ContextMenu>
	</div>
</template>
<style lang="scss" scoped>
.grid-display {
	display: flex;
	flex-direction: column;
	gap: 0.75rem;
}

.grid-display-results {
	display: flex;
	flex-direction: column;
	gap: 0.75rem;
}

.grid-display-controls-stage {
	display: grid;
	min-width: 0;
	overflow: visible;
}

.grid-display-controls-frame {
	grid-area: 1 / 1;
	min-width: 0;
	width: 100%;
	background: var(--color-bg);
}

.grid-display-controls {
	display: flex;
	gap: 0.5rem;
}

.grid-display-results-stage {
	display: grid;
	min-width: 0;
	overflow: hidden;
}

.grid-display-results-frame {
	grid-area: 1 / 1;
	min-width: 0;
	width: 100%;
	background: var(--color-bg);
}

.grid-display-tab-forward-enter-active,
.grid-display-tab-forward-leave-active,
.grid-display-tab-backward-enter-active,
.grid-display-tab-backward-leave-active,
.grid-display-subtle-page-forward-enter-active,
.grid-display-subtle-page-forward-leave-active,
.grid-display-subtle-page-backward-enter-active,
.grid-display-subtle-page-backward-leave-active {
	transition:
		opacity 170ms cubic-bezier(0.2, 0, 0, 1),
		transform 170ms cubic-bezier(0.2, 0, 0, 1);
	will-change: opacity, transform;
}

.grid-display-tab-forward-enter-active,
.grid-display-tab-backward-enter-active,
.grid-display-subtle-page-forward-enter-active,
.grid-display-subtle-page-backward-enter-active {
	z-index: 2;
}

.grid-display-tab-forward-leave-active,
.grid-display-tab-backward-leave-active,
.grid-display-subtle-page-forward-leave-active,
.grid-display-subtle-page-backward-leave-active {
	pointer-events: none;
	z-index: 1;
}

.grid-display-tab-forward-enter-from,
.grid-display-subtle-page-forward-enter-from {
	opacity: 0;
	transform: translate3d(1.5rem, 0, 0);
}

.grid-display-tab-forward-leave-to,
.grid-display-subtle-page-forward-leave-to {
	opacity: 0;
	transform: translate3d(-0.75rem, 0, 0);
}

.grid-display-tab-backward-enter-from,
.grid-display-subtle-page-backward-enter-from {
	opacity: 0;
	transform: translate3d(-1.5rem, 0, 0);
}

.grid-display-tab-backward-leave-to,
.grid-display-subtle-page-backward-leave-to {
	opacity: 0;
	transform: translate3d(0.75rem, 0, 0);
}

.grid-display-in-only-forward-enter-active,
.grid-display-in-only-backward-enter-active {
	z-index: 2;
	transition:
		opacity 150ms cubic-bezier(0.2, 0, 0, 1),
		transform 150ms cubic-bezier(0.2, 0, 0, 1);
	will-change: opacity, transform;
}

.grid-display-in-only-forward-leave-active,
.grid-display-in-only-backward-leave-active {
	display: none;
}

.grid-display-in-only-forward-enter-from {
	opacity: 0;
	transform: translate3d(0.75rem, 0, 0);
}

.grid-display-in-only-backward-enter-from {
	opacity: 0;
	transform: translate3d(-0.75rem, 0, 0);
}

.grid-display-push-forward-enter-active,
.grid-display-push-forward-leave-active,
.grid-display-push-backward-enter-active,
.grid-display-push-backward-leave-active {
	transition: transform 240ms cubic-bezier(0.2, 0, 0, 1);
	will-change: transform;
}

.grid-display-push-forward-leave-active,
.grid-display-push-backward-leave-active {
	pointer-events: none;
}

.grid-display-push-forward-enter-from {
	transform: translate3d(100%, 0, 0);
}

.grid-display-push-forward-leave-to {
	transform: translate3d(-100%, 0, 0);
}

.grid-display-push-backward-enter-from {
	transform: translate3d(-100%, 0, 0);
}

.grid-display-push-backward-leave-to {
	transform: translate3d(100%, 0, 0);
}

.grid-display-card-move,
.grid-display-card-enter-active,
.grid-display-card-leave-active {
	transition:
		opacity 160ms cubic-bezier(0.2, 0, 0, 1),
		transform 160ms cubic-bezier(0.2, 0, 0, 1);
}

.grid-display-card-enter-from {
	opacity: 0;
	transform: translate3d(0, 0.25rem, 0) scale(0.985);
}

.grid-display-card-leave-to {
	opacity: 0;
	transform: scale(0.985);
}

.grid-display-card-leave-active {
	position: absolute;
	pointer-events: none;
}

.row {
	width: 100%;
}

.instances {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
	width: 100%;
	gap: 0.75rem;
	margin-right: auto;
	scroll-behavior: smooth;
	overflow-y: auto;
	position: relative;
}
</style>
