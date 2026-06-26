<script setup lang="ts">
/**
 * App browse layout.
 * - `sortOptions` (56) and `maxResultsOptions` (63) adapt shared browse state to controls.
 * - `cardSlideEnabled` (94) and `loadingBarEnabled` (95) persist Discover page polish toggles.
 * - `activeProjectTypeTabIndex` (106) and `selectProjectTypeTab` (123) drive instant local tabs.
 * - `resultsTransitionKey` (102) and `browseCardsTransition` (103) drive discover card slides.
 * - Template result cards (309) render app-specific browse actions and server metadata.
 */
import type { Labrinth } from '@modrinth/api-client'
import { SearchIcon } from '@modrinth/assets'
import {
	BrowseInstallHeader,
	ButtonStyled,
	type CardAction,
	Combobox,
	type ComboboxOption,
	commonMessages,
	defineMessages,
	formatProjectTypeSentence,
	injectBrowseManager,
	LoadingIndicator,
	NavTabs,
	Pagination,
	ProjectCard,
	ProjectCardList,
	SearchFilterControl,
	SelectedProjectsFloatingBar,
	type SortType,
	StyledInput,
	Toggle,
	useStickyObserver,
	useVIntl,
} from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, ref, toValue, watch } from 'vue'
import { useRouter } from 'vue-router'

import AppJoinedButtons from '@/components/ui/AppJoinedButtons.vue'
import { getDiscoverProjectTypeFromHref } from '@/composables/useDiscoverContentPreload'

interface AppCardAction extends CardAction {
	joinedActions?: AppCardAction[]
}

const ctx = injectBrowseManager()
const { formatMessage } = useVIntl()
const router = useRouter()
const lockedMessages = computed(() => toValue(ctx.lockedFilterMessages))
const stickyInstallHeaderRef = ref<HTMLElement | null>(null)
const { isStuck: isInstallHeaderStuck } = useStickyObserver(
	stickyInstallHeaderRef,
	'BrowseInstallHeader',
)

const sortOptions = computed<ComboboxOption<SortType>[]>(() =>
	ctx.effectiveSortTypes.value.map((st) => ({
		value: st,
		label: st.display,
	})),
)

const maxResultsOptions = computed<ComboboxOption<number>[]>(() =>
	(ctx.maxResultsOptions?.value ?? [5, 10, 15, 20, 50, 100]).map((n) => ({
		value: n,
		label: String(n),
	})),
)

const messages = defineMessages({
	searchPlaceholder: {
		id: 'browse.search.placeholder',
		defaultMessage: 'Search {projectType}...',
	},
	viewPrefix: {
		id: 'browse.view-prefix',
		defaultMessage: 'View:',
	},
	filterResults: {
		id: 'browse.filter-results',
		defaultMessage: 'Filter results...',
	},
	offline: {
		id: 'browse.offline',
		defaultMessage: 'You are currently offline. Connect to the internet to browse Modrinth!',
	},
	noResults: {
		id: 'browse.no-results',
		defaultMessage: 'No results found for your query!',
	},
})

const BROWSE_LOADING_BAR_STORAGE_KEY = 'app-browse-loading-bar-enabled'
const cardSlideEnabled = useStorage('app-browse-card-slide-animation', true)
const loadingBarEnabled = useStorage(BROWSE_LOADING_BAR_STORAGE_KEY, false)
const slideDirection = ref<'forward' | 'backward'>('forward')
const optimisticProjectType = ref<string | null>(null)
const resultSignature = computed(() => {
	const hits = ctx.isServerType.value ? ctx.serverHits.value : ctx.projectHits.value
	return hits.map((hit) => hit.project_id).join('|')
})
const resultsTransitionKey = ref(`${ctx.projectType.value}:${resultSignature.value}`)
const browseCardsTransition = computed(() =>
	cardSlideEnabled.value ? `browse-cards-${slideDirection.value}` : undefined,
)
const activeProjectTypeTabIndex = computed(() =>
	ctx.selectableProjectTypes.value
		.filter((link) => link.shown ?? true)
		.findIndex(
			(link) =>
				getDiscoverProjectTypeFromHref(link.href) ===
				(optimisticProjectType.value ?? ctx.projectType.value),
		),
)

function getProjectTypeOrder() {
	return ctx.selectableProjectTypes.value
		.filter((link) => link.shown ?? true)
		.map((link) => link.href.match(/^\/browse\/([^?]+)/)?.[1])
		.filter((type): type is string => !!type)
}

function selectProjectTypeTab(_index: number, tab: { href: string; onHover?: () => void }) {
	tab.onHover?.()

	const nextProjectType = getDiscoverProjectTypeFromHref(tab.href)
	if (nextProjectType) {
		optimisticProjectType.value = nextProjectType
	}

	void router.push(tab.href).catch(() => undefined)
}

watch(
	() => ctx.projectType.value,
	(next, previous) => {
		optimisticProjectType.value = null

		const order = getProjectTypeOrder()
		const previousIndex = order.indexOf(previous)
		const nextIndex = order.indexOf(next)

		if (previousIndex !== -1 && nextIndex !== -1) {
			slideDirection.value = nextIndex > previousIndex ? 'forward' : 'backward'
		}
	},
)

watch(resultSignature, () => {
	resultsTransitionKey.value = `${ctx.projectType.value}:${resultSignature.value}`
})

function toJoinedButtonAction(action: CardAction) {
	return {
		id: action.key,
		label: action.label,
		icon: action.icon,
		action: action.onClick,
		color: action.color,
		disabled: action.disabled,
	}
}

function getJoinedCardActions(actions?: AppCardAction[]) {
	return actions?.filter((action) => action.joinedActions?.length) ?? []
}

function getStandardCardActions(actions?: AppCardAction[]) {
	return actions?.filter((action) => !action.joinedActions?.length) ?? []
}

function getJoinedButtonActions(action: AppCardAction) {
	return [toJoinedButtonAction(action), ...(action.joinedActions ?? []).map(toJoinedButtonAction)]
}
</script>

<template>
	<template v-if="ctx.installContext?.value && ctx.variant !== 'web'">
		<div
			ref="stickyInstallHeaderRef"
			class="sticky top-0 z-20 -mx-6 -mt-6 rounded-tl-[--radius-xl] border-0 border-b border-solid bg-surface-1 p-3 border-surface-5"
			:class="[isInstallHeaderStuck ? 'border-t' : '']"
		>
			<BrowseInstallHeader />
		</div>
	</template>
	<SelectedProjectsFloatingBar v-if="ctx.installContext?.value && ctx.variant !== 'web'" />

	<NavTabs
		v-if="ctx.showProjectTypeTabs.value"
		mode="local"
		:links="ctx.selectableProjectTypes.value"
		:active-index="activeProjectTypeTabIndex"
		transition-duration="95ms"
		transition-stagger-delay="35ms"
		opacity-transition-duration="120ms"
		@tab-click="selectProjectTypeTab"
	/>

	<StyledInput
		v-model="ctx.query.value"
		:icon="SearchIcon"
		type="text"
		autocomplete="off"
		:placeholder="
			formatMessage(messages.searchPlaceholder, {
				projectType: formatProjectTypeSentence(formatMessage, ctx.projectType.value, 2),
			})
		"
		clearable
		wrapper-class="w-full"
		:input-class="ctx.variant === 'web' ? '!h-12' : 'h-12'"
		@clear="ctx.clearSearch()"
	/>

	<div class="flex flex-wrap items-center gap-2">
		<Combobox
			:model-value="ctx.effectiveCurrentSortType.value"
			:options="sortOptions"
			:class="
				ctx.variant === 'web'
					? '!w-[16rem] min-w-max max-w-full flex-grow md:flex-grow-0'
					: '!w-[16rem] min-w-max max-w-full'
			"
			@update:model-value="(val: SortType) => (ctx.effectiveCurrentSortType.value = val)"
		>
			<template #prefix>
				<span class="font-semibold text-primary">{{
					formatMessage(commonMessages.sortByLabel)
				}}</span>
			</template>
		</Combobox>

		<Combobox
			:model-value="ctx.maxResults.value"
			:options="maxResultsOptions"
			:class="
				ctx.variant === 'web'
					? '!w-[9rem] min-w-max max-w-full flex-grow md:flex-grow-0'
					: '!w-[9rem] min-w-max max-w-full'
			"
			:placeholder="formatMessage(commonMessages.viewLabel)"
			@update:model-value="(val: number) => (ctx.maxResults.value = val)"
		>
			<template #prefix>
				<span class="font-semibold text-primary">{{ formatMessage(messages.viewPrefix) }}</span>
			</template>
		</Combobox>

		<div v-if="ctx.filtersMenuOpen && !ctx.filtersMenuOpen.value" class="lg:hidden">
			<ButtonStyled>
				<button @click="ctx.filtersMenuOpen.value = true">
					{{ formatMessage(messages.filterResults) }}
				</button>
			</ButtonStyled>
		</div>

		<ButtonStyled v-if="ctx.cycleDisplayMode" circular>
			<button @click="ctx.cycleDisplayMode!()">
				<slot name="display-mode-icon" />
			</button>
		</ButtonStyled>

		<Pagination
			:page="ctx.currentPage.value"
			:count="ctx.pageCount.value"
			:class="ctx.variant === 'web' ? 'mx-auto sm:ml-auto sm:mr-0' : 'ml-auto'"
			@switch-page="ctx.setPage"
		/>
	</div>

	<SearchFilterControl
		v-if="ctx.isServerType.value"
		v-model:selected-filters="ctx.serverCurrentFilters.value"
		:filters="ctx.serverFilterTypes.value"
		:provided-filters="[]"
		:overridden-provided-filter-types="[]"
	/>
	<SearchFilterControl
		v-else
		v-model:selected-filters="ctx.currentFilters.value"
		:filters="ctx.filters.value.filter((f) => f.display !== 'none')"
		:provided-filters="ctx.providedFilters?.value ?? []"
		:overridden-provided-filter-types="ctx.overriddenProvidedFilterTypes.value"
		:provided-message="lockedMessages?.providedBy"
	/>

	<div class="search">
		<section v-if="ctx.loading.value" class="offline">
			<component :is="ctx.loadingComponent ?? LoadingIndicator" />
		</section>
		<section v-else-if="ctx.offline?.value && ctx.totalHits.value === 0" class="offline">
			{{ formatMessage(messages.offline) }}
		</section>
		<section
			v-else-if="
				ctx.isServerType.value
					? ctx.serverHits.value.length === 0
					: ctx.projectHits.value.length === 0
			"
			class="offline"
		>
			<p>{{ formatMessage(messages.noResults) }}</p>
		</section>

		<Transition v-else :name="browseCardsTransition" mode="out-in">
			<ProjectCardList :key="resultsTransitionKey" :layout="ctx.effectiveLayout.value">
				<template v-if="ctx.isServerType.value">
					<ProjectCard
						v-for="result in ctx.serverHits.value"
						:key="`server-card-${result.project_id}`"
						:title="result.name"
						:icon-url="result.icon_url || undefined"
						:summary="result.summary"
						:tags="result.categories"
						:link="ctx.getServerProjectLink(result)"
						:server-online-players="result.minecraft_java_server?.ping?.data?.players_online ?? 0"
						:server-region="result.minecraft_server?.region"
						:server-recent-plays="result.minecraft_java_server?.verified_plays_2w ?? 0"
						:server-modpack-content="ctx.getServerModpackContent?.(result)"
						:server-ping="ctx.serverPings?.value?.[result.project_id]"
						:server-status-online="!!result.minecraft_java_server?.ping?.data"
						:hide-online-players-label="ctx.variant === 'app'"
						:hide-recent-plays-label="ctx.variant === 'app'"
						:layout="ctx.effectiveLayout.value"
						:max-tags="2"
						is-server-project
						exclude-loaders
						:color="result.color ?? undefined"
						:banner="result.featured_gallery ?? undefined"
						@contextmenu.prevent.stop="(event: MouseEvent) => ctx.onContextMenu?.(event, result)"
						@mouseenter="ctx.onServerProjectHover?.(result)"
						@mouseleave="ctx.onProjectHoverEnd?.()"
					>
						<template v-if="ctx.getCardActions?.(result, ctx.projectType.value)?.length" #actions>
							<div class="flex gap-2">
								<AppJoinedButtons
									v-for="action in getJoinedCardActions(
										ctx.getCardActions(result, ctx.projectType.value),
									)"
									:key="action.key"
									class="browse-card-joined-button"
									:actions="getJoinedButtonActions(action)"
									:color="action.color"
									:type="action.type"
									merged
									:disabled="action.disabled"
									:primary-tooltip="action.tooltip"
									@click.stop
								/>
								<ButtonStyled
									v-for="action in getStandardCardActions(
										ctx.getCardActions(result, ctx.projectType.value),
									)"
									:key="action.key"
									:color="action.color"
									:type="action.type"
									:circular="action.circular"
								>
									<button
										v-tooltip="action.tooltip"
										:disabled="action.disabled"
										@click.stop="action.onClick"
									>
										<component :is="action.icon" :class="action.iconClass" />
										<template v-if="!action.circular">{{ action.label }}</template>
									</button>
								</ButtonStyled>
							</div>
						</template>
					</ProjectCard>
				</template>
				<template v-else>
					<ProjectCard
						v-for="result in ctx.projectHits.value"
						:key="result.project_id"
						:link="ctx.getProjectLink(result)"
						:title="result.title"
						:icon-url="result.icon_url"
						:author="{
							name: result.organization == null ? result.author : result.organization,
							link:
								result.organization_id == null
									? ctx.variant === 'web'
										? `/user/${result.author_id ?? result.author}`
										: `https://modrinth.com/user/${result.author_id ?? result.author}`
									: ctx.variant === 'web'
										? `/organization/${result.organization_id}`
										: `https://modrinth.com/organization/${result.organization_id}`,
						}"
						:date-updated="result.date_modified"
						:date-published="result.date_created"
						:displayed-date="
							ctx.effectiveCurrentSortType.value.name === 'newest' ? 'published' : 'updated'
						"
						:downloads="result.downloads"
						:summary="result.description"
						:tags="result.display_categories"
						:all-tags="result.categories"
						:deprioritized-tags="ctx.deprioritizedTags.value"
						:exclude-loaders="ctx.excludeLoaders.value"
						:followers="result.follows"
						:banner="result.featured_gallery ?? undefined"
						:color="result.color ?? undefined"
						:environment="
							['mod', 'modpack'].includes(ctx.projectType.value)
								? {
										clientSide: result.client_side as Labrinth.Projects.v2.Environment,
										serverSide: result.server_side as Labrinth.Projects.v2.Environment,
									}
								: undefined
						"
						:layout="ctx.effectiveLayout.value"
						@contextmenu.prevent.stop="(event: MouseEvent) => ctx.onContextMenu?.(event, result)"
						@mouseenter="ctx.onProjectHover?.(result)"
						@mouseleave="ctx.onProjectHoverEnd?.()"
					>
						<template v-if="ctx.getCardActions?.(result, ctx.projectType.value)?.length" #actions>
							<div class="flex gap-2">
								<AppJoinedButtons
									v-for="action in getJoinedCardActions(
										ctx.getCardActions(result, ctx.projectType.value),
									)"
									:key="action.key"
									class="browse-card-joined-button"
									:actions="getJoinedButtonActions(action)"
									:color="action.color"
									:type="action.type"
									merged
									:disabled="action.disabled"
									:primary-tooltip="action.tooltip"
									@click.stop
								/>
								<ButtonStyled
									v-for="action in getStandardCardActions(
										ctx.getCardActions(result, ctx.projectType.value),
									)"
									:key="action.key"
									:color="action.color"
									:type="action.type"
									:circular="action.circular"
								>
									<button
										v-tooltip="action.tooltip"
										:disabled="action.disabled"
										@click.stop="action.onClick"
									>
										<component :is="action.icon" :class="action.iconClass" />
										<template v-if="!action.circular">{{ action.label }}</template>
									</button>
								</ButtonStyled>
							</div>
						</template>
					</ProjectCard>
				</template>
			</ProjectCardList>
		</Transition>

		<div :class="ctx.variant === 'web' ? 'pagination-after mt-3' : 'flex justify-end mt-3'">
			<Pagination
				:page="ctx.currentPage.value"
				:count="ctx.pageCount.value"
				:class="ctx.variant === 'web' ? 'justify-end' : 'pagination-after'"
				@switch-page="ctx.setPage"
			/>
		</div>
	</div>

	<slot name="after" />

	<Teleport to="body">
		<div class="fixed z-20 flex items-center gap-2" style="right: 1.25rem; bottom: 1.25rem">
			<div class="rounded-full border border-solid border-surface-5 bg-surface-3 p-2 shadow-lg">
				<Toggle
					id="browse-loading-bar-toggle"
					v-model="loadingBarEnabled"
					v-tooltip="'Show Discover loading bar'"
					small
					aria-label="Show Discover loading bar"
				/>
			</div>
			<div class="rounded-full border border-solid border-surface-5 bg-surface-3 p-2 shadow-lg">
				<Toggle
					id="browse-card-slide-toggle"
					v-model="cardSlideEnabled"
					v-tooltip="'Animate browse cards'"
					small
					aria-label="Animate browse cards"
				/>
			</div>
		</div>
	</Teleport>
</template>

<style scoped>
.browse-card-joined-button {
	position: relative;
}

.browse-card-joined-button::after {
	content: '';
	position: absolute;
	right: calc(1.75rem - 1px);
	top: 50%;
	z-index: 1;
	width: 1px;
	height: 1.8rem;
	transform: translateY(-50%);
	background: linear-gradient(
		to bottom,
		rgb(255 255 255 / 0),
		rgb(255 255 255 / 0.76) 18%,
		rgb(255 255 255 / 0.76) 82%,
		rgb(255 255 255 / 0)
	);
	pointer-events: none;
}

.browse-card-joined-button :deep(:is(button, a, .button-like):first-child:active) {
	scale: 1 !important;
}

.browse-card-joined-button:is(:hover, :has(:focus-visible), :has(:active))
	:deep(:is(button, a, .button-like):first-child) {
	filter: brightness(var(--hover-brightness));
}

.browse-card-joined-button :deep(.btn-dropdown-animation) {
	width: 1.75rem !important;
	padding-left: 0.25rem !important;
	padding-right: 0.25rem !important;
	border-left-color: var(--_bg) !important;
}

.browse-cards-forward-enter-active,
.browse-cards-forward-leave-active,
.browse-cards-backward-enter-active,
.browse-cards-backward-leave-active {
	transition:
		opacity 90ms cubic-bezier(0.2, 0, 0, 1),
		transform 90ms cubic-bezier(0.2, 0, 0, 1);
	will-change: opacity, transform;
}

.browse-cards-forward-enter-from,
.browse-cards-backward-leave-to {
	opacity: 0;
	transform: translateX(1rem);
}

.browse-cards-forward-leave-to,
.browse-cards-backward-enter-from {
	opacity: 0;
	transform: translateX(-1rem);
}

@media (prefers-reduced-motion: reduce) {
	.browse-cards-forward-enter-active,
	.browse-cards-forward-leave-active,
	.browse-cards-backward-enter-active,
	.browse-cards-backward-leave-active {
		transition: none;
	}
}
</style>
