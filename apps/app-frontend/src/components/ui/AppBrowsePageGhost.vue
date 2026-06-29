<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import {
	Combobox,
	type ComboboxOption,
	commonMessages,
	defineMessages,
	GhostBlock,
	GhostMedia,
	NavTabs,
	Pagination,
	StyledInput,
	type SortType,
	useVIntl,
} from '@modrinth/ui'
import { ref } from 'vue'

defineOptions({
	name: 'AppBrowsePageGhost',
})

const { formatMessage } = useVIntl()
const ghostSearch = ref('')
const ghostSort = ref<SortType>({ display: 'Relevance', name: 'relevance' })
const ghostView = ref(20)
const sortOptions: ComboboxOption<SortType>[] = [{ value: ghostSort.value, label: 'Relevance' }]
const viewOptions: ComboboxOption<number>[] = [{ value: 20, label: '20' }]
const tabs = [
	{ label: 'Modpacks', href: 'modpack' },
	{ label: 'Mods', href: 'mod' },
	{ label: 'Resource Packs', href: 'resourcepack' },
	{ label: 'Data Packs', href: 'datapack' },
	{ label: 'Shaders', href: 'shader' },
	{ label: 'Servers', href: 'server' },
]
const ghostProjects = [
	{
		title: 'Fabulously Optimized',
		author: 'Fabulously Optimized',
		descriptionWidth: '35rem',
		displayCategories: ['lightweight', 'multiplayer', 'optimization'],
		overflowCount: 1,
		downloads: '13.63M',
		downloadWidth: '55px',
		follows: '4,482',
		followWidth: '45px',
		updated: '6 days ago',
		updatedWidth: '83px',
		installJoined: false,
	},
	{
		title: 'Zombie Invade 100 Days',
		author: 'FlameFire',
		descriptionWidth: '34rem',
		displayCategories: ['challenging', 'combat'],
		overflowCount: 2,
		downloads: '10.85M',
		downloadWidth: '55px',
		follows: '546',
		followWidth: '30px',
		updated: '6 months ago',
		updatedWidth: '104px',
		installJoined: true,
	},
	{
		title: 'Cobblemon Official Modpack [Fabric]',
		author: 'Cobbled Studios',
		descriptionWidth: '20.5rem',
		displayCategories: ['adventure', 'lightweight', 'multiplayer'],
		overflowCount: 2,
		downloads: '8.2M',
		downloadWidth: '40px',
		follows: '2,429',
		followWidth: '45px',
		updated: '5 months ago',
		updatedWidth: '104px',
		installJoined: true,
	},
	{
		title: 'COBBLEVERSE - Pokemon Adventure [Cobblemon]',
		author: 'LUMYVERSE',
		descriptionWidth: '54rem',
		displayCategories: ['adventure', 'multiplayer', 'optimization'],
		overflowCount: 3,
		downloads: '4.54M',
		downloadWidth: '47px',
		follows: '1,643',
		followWidth: '45px',
		updated: '3 months ago',
		updatedWidth: '104px',
		installJoined: true,
	},
	{
		title: 'Better MC [FABRIC] - BMC2',
		author: 'Luna Pixel Studios',
		descriptionWidth: '27.5rem',
		displayCategories: ['adventure', 'combat', 'optimization'],
		overflowCount: 4,
		downloads: '2.46M',
		downloadWidth: '47px',
		follows: '1,265',
		followWidth: '45px',
		updated: '1 year ago',
		updatedWidth: '76px',
		installJoined: true,
	},
	{
		title: 'Sodium Plus',
		author: 'HappyRedstone Modding',
		descriptionWidth: '22rem',
		displayCategories: ['lightweight', 'multiplayer', 'optimization'],
		overflowCount: 2,
		downloads: '2.15M',
		downloadWidth: '47px',
		follows: '561',
		followWidth: '30px',
		updated: '13 days ago',
		updatedWidth: '92px',
		installJoined: false,
	},
	{
		title: 'Vanilla Perfected',
		author: 'demonjoeTV',
		descriptionWidth: '48.5rem',
		displayCategories: ['adventure', 'lightweight', 'optimization'],
		overflowCount: 1,
		downloads: '1.94M',
		downloadWidth: '47px',
		follows: '2,237',
		followWidth: '45px',
		updated: '12 days ago',
		updatedWidth: '92px',
		installJoined: false,
	},
] as const
const messages = defineMessages({
	viewPrefix: {
		id: 'browse.view-prefix',
		defaultMessage: 'View:',
	},
})

function remWidth(text: string, min: number, max: number, ratio: number) {
	return `${Math.min(max, Math.max(min, Number((text.length * ratio).toFixed(2))))}rem`
}

function titleWidth(project: (typeof ghostProjects)[number]) {
	return remWidth(project.title, 7, 28, 0.55)
}

function authorWidth(project: (typeof ghostProjects)[number]) {
	return remWidth(`by ${project.author}`, 5, 16, 0.43)
}

function tagWidth(tag: string) {
	return remWidth(tag, 3.5, 8.5, 0.55)
}

function overflowWidth(count: number) {
	return remWidth(`+${count}`, 2.25, 3.25, 0.7)
}
</script>

<template>
	<div class="app-browse-ghost" inert aria-hidden="true">
		<NavTabs
			mode="local"
			:links="tabs"
			:active-index="0"
			class="app-browse-ghost-tabs"
			@tab-click="() => {}"
		/>

		<StyledInput
			v-model="ghostSearch"
			:icon="SearchIcon"
			type="text"
			autocomplete="off"
			placeholder="Search modpacks..."
			readonly
			class="app-browse-ghost-search"
			wrapper-class="w-full"
			input-class="h-12"
		/>

		<div class="app-browse-ghost-controls">
			<div class="app-browse-ghost-control-group">
				<div class="pointer-events-none">
					<Combobox
						:model-value="ghostSort"
						:options="sortOptions"
						class="!w-[16rem] min-w-max max-w-full"
					>
						<template #prefix>
							<span class="font-semibold text-primary">{{
								formatMessage(commonMessages.sortByLabel)
							}}</span>
						</template>
					</Combobox>
				</div>

				<div class="pointer-events-none">
					<Combobox
						:model-value="ghostView"
						:options="viewOptions"
						class="!w-[9rem] min-w-max max-w-full"
						:placeholder="formatMessage(commonMessages.viewLabel)"
					>
						<template #prefix>
							<span class="font-semibold text-primary">{{ formatMessage(messages.viewPrefix) }}</span>
						</template>
					</Combobox>
				</div>
			</div>

			<div class="pointer-events-none ml-auto">
				<Pagination :page="1" :count="861" @switch-page="() => {}" />
			</div>
		</div>

		<div class="app-browse-ghost-list">
			<div
				v-for="project in ghostProjects"
				:key="project.title"
				class="app-browse-ghost-card"
			>
				<GhostMedia kind="rounded" class="app-browse-ghost-icon" />

				<div class="app-browse-ghost-main">
					<div class="app-browse-ghost-title-row">
						<GhostBlock
							shape="text"
							class="app-browse-ghost-title"
							:style="{ width: titleWidth(project) }"
						/>
						<GhostBlock
							shape="text"
							class="app-browse-ghost-author"
							:style="{ width: authorWidth(project) }"
						/>
					</div>
					<GhostBlock
						shape="text"
						class="app-browse-ghost-description"
						:style="{ width: project.descriptionWidth }"
					/>
				</div>

				<div class="app-browse-ghost-actions">
					<div v-if="project.installJoined" class="app-browse-ghost-install-joined">
						<GhostBlock
							shape="control"
							class="app-browse-ghost-install app-browse-ghost-install--main"
						/>
						<GhostBlock
							shape="control"
							class="app-browse-ghost-install app-browse-ghost-install--dropdown"
						/>
					</div>
					<GhostBlock
						v-else
						shape="control"
						class="app-browse-ghost-install app-browse-ghost-install--single"
					/>
				</div>
				<div class="app-browse-ghost-side">
					<div class="flex items-center gap-3">
						<div class="app-browse-ghost-metric">
							<span
								class="app-browse-ghost-metric-icon app-browse-ghost-metric-icon--download"
							/>
							<GhostBlock
								shape="text"
								class="app-browse-ghost-stat"
								:style="{ width: project.downloadWidth }"
							/>
						</div>
						<div class="app-browse-ghost-metric">
							<span class="app-browse-ghost-metric-icon app-browse-ghost-metric-icon--heart" />
							<GhostBlock
								shape="text"
								class="app-browse-ghost-stat"
								:style="{ width: project.followWidth }"
							/>
						</div>
					</div>
					<div class="app-browse-ghost-metric">
						<span class="app-browse-ghost-metric-icon app-browse-ghost-metric-icon--history" />
						<GhostBlock
							shape="text"
							class="app-browse-ghost-stat"
							:style="{ width: project.updatedWidth }"
						/>
					</div>
				</div>

				<div class="app-browse-ghost-tags">
					<GhostBlock
						v-for="tag in project.displayCategories"
						:key="`${project.title}-${tag}`"
						shape="pill"
						class="h-[22px]"
						:style="{ width: tagWidth(tag) }"
					/>
					<GhostBlock
						v-if="project.overflowCount > 0"
						shape="pill"
						class="h-[22px]"
						:style="{ width: overflowWidth(project.overflowCount) }"
					/>
				</div>
			</div>
		</div>
	</div>
</template>

<style scoped>
.app-browse-ghost {
	display: flex;
	height: 100%;
	min-height: 0;
	flex-direction: column;
	gap: 0.75rem;
}

.app-browse-ghost-tabs {
	min-height: 40px;
	flex: 0 0 auto;
	overflow: hidden;
}

.app-browse-ghost-search,
.app-browse-ghost-controls {
	flex: 0 0 auto;
}

.app-browse-ghost-controls,
.app-browse-ghost-control-group,
.app-browse-ghost-title-row,
.app-browse-ghost-tags {
	display: flex;
	align-items: center;
	gap: 0.5rem;
}

.app-browse-ghost-controls,
.app-browse-ghost-tags {
	flex-wrap: wrap;
}

.app-browse-ghost-list {
	display: flex;
	min-height: 0;
	flex: 1 1 auto;
	flex-direction: column;
	gap: 0.75rem;
	overflow: hidden;
}

.app-browse-ghost-card {
	display: grid;
	flex: 0 0 142px;
	grid-template:
		'icon main actions actions'
		'icon main dummy stats'
		'icon tags tags stats';
	grid-template-columns: auto minmax(0, 1fr) auto auto;
	height: 142px;
	gap: 8px 12px;
	overflow: hidden;
	border: 1px solid var(--color-button-bg);
	border-radius: var(--radius-xl);
	background: var(--color-raised-bg);
	padding: 16px;
}

.app-browse-ghost-icon {
	grid-area: icon;
	width: 100px;
	height: 100px;
	flex: 0 0 auto;
	border-radius: var(--radius-lg);
}

.app-browse-ghost-main {
	grid-area: main;
	display: flex;
	min-width: 0;
	flex-direction: column;
	gap: 0.5rem;
}

.app-browse-ghost-title-row {
	min-width: 0;
}

.app-browse-ghost-title {
	height: 22px;
	max-width: min(28rem, 100%);
	flex: 0 1 auto;
}

.app-browse-ghost-author {
	height: 16px;
	max-width: 16rem;
	flex: 0 1 auto;
}

.app-browse-ghost-description,
.app-browse-ghost-stat {
	height: 16px;
	max-width: 100%;
}

.app-browse-ghost-metric {
	display: flex;
	align-items: center;
	gap: 8px;
}

.app-browse-ghost-metric-icon {
	display: block;
	width: 20px;
	height: 20px;
	flex: 0 0 auto;
	background-color: color-mix(in srgb, var(--surface-4) 72%, var(--surface-5) 28%);
	background-image:
		repeating-linear-gradient(
			45deg,
			transparent 0rem,
			transparent 15rem,
			color-mix(in srgb, var(--surface-5) 8%, transparent) 20rem,
			color-mix(in srgb, var(--surface-1) 18%, transparent) 25rem,
			color-mix(in srgb, var(--surface-5) 16%, transparent) 30rem,
			transparent 39rem,
			transparent 64rem
		),
		repeating-linear-gradient(
			45deg,
			transparent 0rem,
			transparent 34rem,
			color-mix(in srgb, var(--surface-1) 10%, transparent) 42rem,
			color-mix(in srgb, var(--surface-5) 12%, transparent) 50rem,
			transparent 64rem
		);
	background-attachment: fixed, fixed;
	background-position:
		0 0,
		0 0;
	background-repeat: repeat;
	background-size:
		64rem 64rem,
		64rem 64rem;
	-webkit-mask-position: center;
	-webkit-mask-repeat: no-repeat;
	-webkit-mask-size: 100% 100%;
	mask-position: center;
	mask-repeat: no-repeat;
	mask-size: 100% 100%;
	animation: app-browse-ghost-icon-sweep 3200ms linear infinite;
}

.app-browse-ghost-metric-icon--download {
	-webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4'/%3E%3Cpath d='M7 10l5 5 5-5'/%3E%3Cpath d='M12 15V3'/%3E%3C/svg%3E");
	mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4'/%3E%3Cpath d='M7 10l5 5 5-5'/%3E%3Cpath d='M12 15V3'/%3E%3C/svg%3E");
}

.app-browse-ghost-metric-icon--heart {
	-webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z'/%3E%3C/svg%3E");
	mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z'/%3E%3C/svg%3E");
}

.app-browse-ghost-metric-icon--history {
	-webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M3 12a9 9 0 1 0 3-6.7L3 8'/%3E%3Cpath d='M3 3v5h5'/%3E%3Cpath d='M12 7v5l4 2'/%3E%3C/svg%3E");
	mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M3 12a9 9 0 1 0 3-6.7L3 8'/%3E%3Cpath d='M3 3v5h5'/%3E%3Cpath d='M12 7v5l4 2'/%3E%3C/svg%3E");
}

@keyframes app-browse-ghost-icon-sweep {
	to {
		background-position:
			64rem -64rem,
			64rem -64rem;
	}
}

.app-browse-ghost-side {
	grid-area: stats;
	display: flex;
	min-width: 168px;
	flex-direction: column;
	align-items: flex-end;
	justify-content: flex-start;
	gap: 12px;
	margin-top: 12px;
	color: var(--color-secondary);
}

.app-browse-ghost-actions {
	grid-area: actions;
	display: flex;
	justify-content: flex-end;
}

.app-browse-ghost-install,
.app-browse-ghost-install-joined {
	height: 36px;
}

.app-browse-ghost-install {
	border-radius: 12px;
}

.app-browse-ghost-install-joined {
	display: flex;
	gap: 1px;
}

.app-browse-ghost-install--single,
.app-browse-ghost-install--main {
	width: 95px;
}

.app-browse-ghost-install--dropdown {
	width: 27px;
}

.app-browse-ghost-install--main {
	border-top-right-radius: 0;
	border-bottom-right-radius: 0;
}

.app-browse-ghost-install--dropdown {
	border-top-left-radius: 0;
	border-bottom-left-radius: 0;
}

.app-browse-ghost-tags {
	grid-area: tags;
	margin-top: auto;
}

@media (max-width: 900px) {
	.app-browse-ghost-card {
		grid-template:
			'icon main actions'
			'icon main stats'
			'tags tags stats';
		grid-template-columns: auto minmax(0, 1fr) auto;
	}

	.app-browse-ghost-side {
		display: none;
	}

	.app-browse-ghost-card {
		grid-template:
			'icon main actions'
			'icon main actions'
			'tags tags tags';
	}
}
</style>
