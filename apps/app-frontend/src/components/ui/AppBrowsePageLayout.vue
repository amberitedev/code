<script setup lang="ts">
/**
 * App browse layout.
 * - sortOptions (90) and projectTypeTabController (134) adapt shared browse state to controls and tab motion.
 * - createBrowseFreezeFrame (259) captures real outgoing card DOM for tab switch visuals.
 * - prepareIncomingBrowseContent (366) limits incoming rendering to the first card slice during motion.
 * - AppFreezeFrameTransition (568) wraps the result list; ProjectCard rendering starts at 606 and 673.
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
	type UiMotionDirection,
	StyledInput,
	TextMorph,
	useNavTabContentController,
	useStickyObserver,
	useVIntl,
} from '@modrinth/ui'
import type { ComponentPublicInstance } from 'vue'
import { computed, nextTick, onUnmounted, ref, toValue, watch } from 'vue'
import { useRouter } from 'vue-router'

import AppBrowseFreezeFrame from '@/components/ui/AppBrowseFreezeFrame.vue'
import AppFreezeFrameTransition from '@/components/ui/AppFreezeFrameTransition.vue'
import AppJoinedButtons from '@/components/ui/AppJoinedButtons.vue'
import { getDiscoverProjectTypeFromHref } from '@/composables/useDiscoverContentPreload'

import type {
	BrowseDomFreezeFrameSnapshot,
	BrowseFreezeFrameSource,
} from './browse-freeze-frame'
import {
	createBrowseDomFreezeFrameSnapshot,
	decodeImagesInElement,
} from './browse-freeze-frame'

interface AppCardAction extends CardAction {
	joinedActions?: AppCardAction[]
}

const ctx = injectBrowseManager()
const { formatMessage } = useVIntl()
const router = useRouter()
const lockedMessages = computed(() => toValue(ctx.lockedFilterMessages))
const stickyInstallHeaderRef = ref<HTMLElement | null>(null)
const stickyControlsRef = ref<HTMLElement | null>(null)
const stickyInstallHeaderHeight = ref(0)
const hasAppInstallHeader = computed(() => !!ctx.installContext?.value && ctx.variant !== 'web')
const controlsFlushWithPageTop = computed(() => ctx.variant !== 'web' && !hasAppInstallHeader.value)
const { isStuck: isInstallHeaderStuck } = useStickyObserver(
	stickyInstallHeaderRef,
	'BrowseInstallHeader',
)
const { isStuck: isControlsStuck } = useStickyObserver(stickyControlsRef, 'BrowseControls')
const cardFrameElements = new Map<string, { element: HTMLElement; index: number }>()
const transitionActive = ref(false)
const renderedCardLimit = ref<number | null>(null)

const BROWSE_FREEZE_FRAME_OVERSCAN_PX = 160
const BROWSE_FREEZE_FRAME_MAX_CARDS = 14
const BROWSE_INCOMING_PREVIEW_CARD_COUNT = 6
const BROWSE_PROGRESSIVE_CARD_BATCH_SIZE = 12
const BROWSE_PROGRESSIVE_CARD_BATCH_DELAY_MS = 16

const emit = defineEmits<{
	(e: 'update:transitioning', transitioning: boolean): void
	(e: 'update:transitionDirection', direction: UiMotionDirection): void
}>()

let stickyHeaderResizeObserver: ResizeObserver | undefined
let progressiveRenderTimer: ReturnType<typeof setTimeout> | undefined
let imageDecodeFrame: ReturnType<typeof requestAnimationFrame> | undefined

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

const displayedProjectType = computed(() => ctx.visibleProjectType?.value ?? ctx.projectType.value)
const searchPlaceholderText = computed(() =>
	formatMessage(messages.searchPlaceholder, {
		projectType: formatProjectTypeSentence(formatMessage, ctx.projectType.value, 2),
	}),
)
const browseContentKey = computed(() => ctx.projectType.value)
const routeProjectTypeTabIndex = computed(() =>
	ctx.selectableProjectTypes.value
		.filter((link) => link.shown ?? true)
		.findIndex((link) => getDiscoverProjectTypeFromHref(link.href) === ctx.projectType.value),
)
const projectTypeTabController = useNavTabContentController({
	activeIndex: routeProjectTypeTabIndex,
	router,
})
const activeProjectTypeTabIndex = projectTypeTabController.activeIndex
const slideDirection = projectTypeTabController.direction
const contentVisible = projectTypeTabController.visible
const selectProjectTypeTab = projectTypeTabController.selectTab
const stickyControlsStyle = computed(() => ({
	top: hasAppInstallHeader.value ? `${stickyInstallHeaderHeight.value}px` : '0px',
}))
const contentProjectTypePending = computed(
	() => displayedProjectType.value !== ctx.projectType.value,
)
const visibleHitsEmpty = computed(() =>
	displayedProjectType.value === 'server'
		? ctx.serverHits.value.length === 0
		: ctx.projectHits.value.length === 0,
)
const showLoadingState = computed(
	() => contentProjectTypePending.value || (ctx.loading.value && visibleHitsEmpty.value),
)

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

function getCardActionsForResult(
	result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
) {
	return (ctx.getCardActions?.(result, displayedProjectType.value) ?? []) as AppCardAction[]
}

const serverCards = computed(() =>
	ctx.serverHits.value.map((result) => ({
		result,
		actions: getCardActionsForResult(result),
		modpackContent: ctx.getServerModpackContent?.(result),
		ping: ctx.serverPings?.value?.[result.project_id],
	})),
)

const projectCards = computed(() =>
	ctx.projectHits.value.map((result) => ({
		result,
		actions: getCardActionsForResult(result),
	})),
)

const renderedServerCards = computed(() => getRenderedCardSlice(serverCards.value))
const renderedProjectCards = computed(() => getRenderedCardSlice(projectCards.value))

type BrowseProjectTypeTab = Parameters<typeof selectProjectTypeTab>[1]

function handleProjectTypeTabClick(index: number, tab: BrowseProjectTypeTab) {
	selectProjectTypeTab(index, { ...tab, onHover: undefined })
}

function getScrollContainer() {
	return stickyControlsRef.value?.closest('.app-viewport') as HTMLElement | null | undefined
}

function resolveCardFrameElement(target: Element | ComponentPublicInstance | null) {
	if (target instanceof HTMLElement) return target

	const element = target?.$el
	return element instanceof HTMLElement ? element : null
}

function setCardFrameRef(
	key: string,
	index: number,
	target: Element | ComponentPublicInstance | null,
) {
	const element = resolveCardFrameElement(target)
	if (!element) {
		cardFrameElements.delete(key)
		return
	}

	cardFrameElements.set(key, { element, index })
}

function getProjectCardKey(result: Labrinth.Search.v2.ResultSearchProject) {
	return `project:${result.project_id}`
}

function getServerCardKey(result: Labrinth.Search.v3.ResultSearchProject) {
	return `server:${result.project_id}`
}

function getRenderedCardSlice<T>(cards: T[]) {
	const limit = renderedCardLimit.value
	return limit === null ? cards : cards.slice(0, limit)
}

function getCardFrameSources(): BrowseFreezeFrameSource[] {
	return Array.from(cardFrameElements.entries())
		.map(([key, frame]) => ({
			key,
			element: frame.element,
			index: frame.index,
		}))
		.sort((a, b) => a.index - b.index)
}

function createBrowseFreezeFrame(): BrowseDomFreezeFrameSnapshot | null {
	if (typeof window === 'undefined') return null

	return createBrowseDomFreezeFrameSnapshot(getCardFrameSources(), {
		viewport: getScrollContainer() ?? undefined,
		overscan: BROWSE_FREEZE_FRAME_OVERSCAN_PX,
		maxItems: BROWSE_FREEZE_FRAME_MAX_CARDS,
	})
}

function resetBrowseScroll() {
	const scrollContainer = getScrollContainer()
	if (scrollContainer) {
		scrollContainer.scrollTo({ top: 0, left: 0, behavior: 'auto' })
		return
	}

	if (typeof window !== 'undefined') {
		window.scrollTo({ top: 0, left: 0, behavior: 'auto' })
	}
}

function clearProgressiveRenderTimer() {
	if (!progressiveRenderTimer) return

	clearTimeout(progressiveRenderTimer)
	progressiveRenderTimer = undefined
}

function clearImageDecodeFrame() {
	if (!imageDecodeFrame) return

	cancelAnimationFrame(imageDecodeFrame)
	imageDecodeFrame = undefined
}

function prepareIncomingCardSlice() {
	clearProgressiveRenderTimer()
	renderedCardLimit.value = BROWSE_INCOMING_PREVIEW_CARD_COUNT
}

function getActiveRenderedCardCount() {
	return displayedProjectType.value === 'server' ? serverCards.value.length : projectCards.value.length
}

function renderAllIncomingCards() {
	clearProgressiveRenderTimer()
	renderedCardLimit.value = null
}

function renderNextIncomingCardBatch() {
	if (renderedCardLimit.value === null) return

	const total = getActiveRenderedCardCount()
	if (total === 0) {
		if (!contentProjectTypePending.value && !showLoadingState.value) {
			renderAllIncomingCards()
		}
		return
	}

	const nextLimit = Math.min(
		total,
		renderedCardLimit.value + BROWSE_PROGRESSIVE_CARD_BATCH_SIZE,
	)
	renderedCardLimit.value = nextLimit

	if (nextLimit >= total) {
		renderAllIncomingCards()
		return
	}

	scheduleProgressiveCardRender()
}

function scheduleProgressiveCardRender() {
	clearProgressiveRenderTimer()
	progressiveRenderTimer = setTimeout(
		renderNextIncomingCardBatch,
		BROWSE_PROGRESSIVE_CARD_BATCH_DELAY_MS,
	)
}

function startProgressiveCardRender() {
	if (renderedCardLimit.value === null) return

	void nextTick().then(() => {
		scheduleProgressiveCardRender()
	})
}

function scheduleIncomingImageDecode() {
	if (typeof requestAnimationFrame === 'undefined') return

	clearImageDecodeFrame()
	imageDecodeFrame = requestAnimationFrame(() => {
		imageDecodeFrame = undefined
		for (const source of getCardFrameSources().slice(0, BROWSE_INCOMING_PREVIEW_CARD_COUNT)) {
			decodeImagesInElement(source.element, 2)
		}
	})
}

function shouldEagerLoadIncomingCard(index: number) {
	return renderedCardLimit.value !== null && index < BROWSE_INCOMING_PREVIEW_CARD_COUNT
}

async function prepareIncomingBrowseContent(el?: Element) {
	void el
	prepareIncomingCardSlice()
	await projectTypeTabController.handleBeforeLeave()
	resetBrowseScroll()
	await nextTick()
	scheduleIncomingImageDecode()
}

function isIncomingBrowseFirstSliceReady() {
	return !contentProjectTypePending.value && !showLoadingState.value
}

function handleAfterEnter() {
	projectTypeTabController.handleAfterEnter()
	startProgressiveCardRender()
}

function handleEnterCancelled() {
	renderAllIncomingCards()
	projectTypeTabController.handleEnterCancelled()
}

function handleLeaveCancelled() {
	renderAllIncomingCards()
	projectTypeTabController.handleLeaveCancelled()
}

watch(
	stickyInstallHeaderRef,
	(el) => {
		stickyHeaderResizeObserver?.disconnect()
		stickyHeaderResizeObserver = undefined
		stickyInstallHeaderHeight.value = el?.offsetHeight ?? 0

		if (el && typeof ResizeObserver !== 'undefined') {
			stickyHeaderResizeObserver = new ResizeObserver(() => {
				stickyInstallHeaderHeight.value = el.offsetHeight
			})
			stickyHeaderResizeObserver.observe(el)
		}
	},
	{ flush: 'post' },
)

watch(transitionActive, (transitioning) => emit('update:transitioning', transitioning))
watch(slideDirection, (direction) => emit('update:transitionDirection', direction), {
	immediate: true,
})
watch(
	[
		() => displayedProjectType.value,
		() => projectCards.value.length,
		() => serverCards.value.length,
		() => contentProjectTypePending.value,
		() => showLoadingState.value,
	],
	() => {
		if (renderedCardLimit.value === null) return

		scheduleIncomingImageDecode()
		if (!transitionActive.value) {
			startProgressiveCardRender()
		}
	},
	{ flush: 'post' },
)

onUnmounted(() => {
	stickyHeaderResizeObserver?.disconnect()
	clearProgressiveRenderTimer()
	clearImageDecodeFrame()
	cardFrameElements.clear()
})
</script>

<template>
	<div class="contents">
		<template v-if="hasAppInstallHeader">
			<div
				ref="stickyInstallHeaderRef"
				class="sticky top-0 z-20 -mx-6 -mt-6 rounded-tl-[--radius-xl] border-0 border-b border-solid bg-surface-1 p-3 border-surface-5"
				:class="[isInstallHeaderStuck ? 'border-t' : '']"
			>
				<BrowseInstallHeader />
			</div>
		</template>
		<SelectedProjectsFloatingBar v-if="hasAppInstallHeader" />

		<div
			ref="stickyControlsRef"
			class="browse-sticky-controls sticky z-20 -mx-6 flex min-w-0 flex-col gap-3 border-0 border-b border-solid bg-surface-1 px-6 py-3"
			:class="[
				isControlsStuck ? 'border-surface-5 shadow-sm' : 'border-transparent',
				controlsFlushWithPageTop ? '-mt-6' : '',
				transitionActive ? 'browse-sticky-controls--transitioning' : '',
			]"
			:style="stickyControlsStyle"
		>
			<NavTabs
				v-if="ctx.showProjectTypeTabs.value"
				mode="local"
				:links="ctx.selectableProjectTypes.value"
				:active-index="activeProjectTypeTabIndex"
				@tab-click="handleProjectTypeTabClick"
			/>

			<StyledInput
				v-model="ctx.query.value"
				:icon="SearchIcon"
				type="text"
				autocomplete="off"
				:placeholder="searchPlaceholderText"
				clearable
				wrapper-class="browse-search-field w-full"
				:input-class="ctx.variant === 'web' ? '!h-12' : 'h-12'"
				@clear="ctx.clearSearch()"
			>
				<template #right>
					<TextMorph
						v-if="!ctx.query.value"
						as="span"
						class-name="browse-search-placeholder pointer-events-none absolute left-10 top-1/2 max-w-[calc(100%-3.25rem)] -translate-y-1/2 truncate text-base font-medium text-secondary"
						:children="searchPlaceholderText"
						aria-hidden="true"
					/>
				</template>
			</StyledInput>

			<div class="browse-controls-row flex flex-wrap items-center gap-2">
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
					:class="
						ctx.variant === 'web'
							? 'browse-pagination mx-auto sm:ml-auto sm:mr-0'
							: 'browse-pagination ml-auto'
					"
					@switch-page="ctx.setPage"
				/>
			</div>

			<SearchFilterControl
				v-if="ctx.isServerType.value"
				v-model:selected-filters="ctx.serverCurrentFilters.value"
				class="browse-filter-chips"
				:filters="ctx.serverFilterTypes.value"
				:provided-filters="[]"
				:overridden-provided-filter-types="[]"
			/>
			<SearchFilterControl
				v-else
				v-model:selected-filters="ctx.currentFilters.value"
				class="browse-filter-chips"
				:filters="ctx.filters.value.filter((f) => f.display !== 'none')"
				:provided-filters="ctx.providedFilters?.value ?? []"
				:overridden-provided-filter-types="ctx.overriddenProvidedFilterTypes.value"
				:provided-message="lockedMessages?.providedBy"
			/>
		</div>

		<AppFreezeFrameTransition
			class="-mt-3"
			v-model:transitioning="transitionActive"
			:content-key="browseContentKey"
			:direction="slideDirection"
			:visible="contentVisible"
			:create-freeze-frame="createBrowseFreezeFrame"
			:prepare-incoming-freeze-frame="prepareIncomingBrowseContent"
			:is-incoming-ready="isIncomingBrowseFirstSliceReady"
			@after-leave="projectTypeTabController.handleAfterLeave"
			@after-enter="handleAfterEnter"
			@enter-cancelled="handleEnterCancelled"
			@leave-cancelled="handleLeaveCancelled"
		>
			<template #freeze-frame="{ snapshot }">
				<AppBrowseFreezeFrame :snapshot="snapshot as BrowseDomFreezeFrameSnapshot" />
			</template>
			<div class="flex min-w-0 flex-col gap-3">
				<div class="search">
					<section v-if="showLoadingState" class="offline">
						<component :is="ctx.loadingComponent ?? LoadingIndicator" />
					</section>
					<section v-else-if="ctx.offline?.value && ctx.totalHits.value === 0" class="offline">
						{{ formatMessage(messages.offline) }}
					</section>
					<section
						v-else-if="
							displayedProjectType === 'server'
								? ctx.serverHits.value.length === 0
								: ctx.projectHits.value.length === 0
						"
						class="offline"
					>
						<p>{{ formatMessage(messages.noResults) }}</p>
					</section>

					<ProjectCardList v-else :layout="ctx.effectiveLayout.value">
						<template v-if="displayedProjectType === 'server'">
							<ProjectCard
								v-for="({ result, actions, modpackContent, ping }, index) in renderedServerCards"
								:key="result.project_id"
								:ref="(target) => setCardFrameRef(getServerCardKey(result), index, target)"
								:title="result.name"
								:icon-url="result.icon_url || undefined"
								:summary="result.summary"
								:tags="result.categories"
								:link="ctx.getServerProjectLink(result)"
								:server-online-players="
									result.minecraft_java_server?.ping?.data?.players_online ?? 0
								"
								:server-region="result.minecraft_server?.region"
								:server-recent-plays="result.minecraft_java_server?.verified_plays_2w ?? 0"
								:server-modpack-content="modpackContent"
								:server-ping="ping"
								:server-status-online="!!result.minecraft_java_server?.ping?.data"
								:hide-online-players-label="ctx.variant === 'app'"
								:hide-recent-plays-label="ctx.variant === 'app'"
								:layout="ctx.effectiveLayout.value"
								:max-tags="2"
								is-server-project
								exclude-loaders
								:color="result.color ?? undefined"
								:banner="result.featured_gallery ?? undefined"
								:image-loading="shouldEagerLoadIncomingCard(index) ? 'eager' : 'lazy'"
								@contextmenu.prevent.stop="
									(event: MouseEvent) => ctx.onContextMenu?.(event, result)
								"
								@mouseenter="ctx.onServerProjectHover?.(result)"
								@mouseleave="ctx.onProjectHoverEnd?.()"
							>
								<template v-if="actions.length" #actions>
									<div class="flex gap-2">
										<AppJoinedButtons
											v-for="action in getJoinedCardActions(actions)"
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
											v-for="action in getStandardCardActions(actions)"
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
								v-for="({ result, actions }, index) in renderedProjectCards"
								:key="result.project_id"
								:ref="(target) => setCardFrameRef(getProjectCardKey(result), index, target)"
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
									['mod', 'modpack'].includes(displayedProjectType)
										? {
												clientSide: result.client_side as Labrinth.Projects.v2.Environment,
												serverSide: result.server_side as Labrinth.Projects.v2.Environment,
											}
										: undefined
								"
								:layout="ctx.effectiveLayout.value"
								:image-loading="shouldEagerLoadIncomingCard(index) ? 'eager' : 'lazy'"
								@contextmenu.prevent.stop="
									(event: MouseEvent) => ctx.onContextMenu?.(event, result)
								"
								@mouseenter="ctx.onProjectHover?.(result)"
								@mouseleave="ctx.onProjectHoverEnd?.()"
							>
								<template v-if="actions.length" #actions>
									<div class="flex gap-2">
										<AppJoinedButtons
											v-for="action in getJoinedCardActions(actions)"
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
											v-for="action in getStandardCardActions(actions)"
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

					<div :class="ctx.variant === 'web' ? 'pagination-after mt-3' : 'flex justify-end mt-3'">
						<Pagination
							:page="ctx.currentPage.value"
							:count="ctx.pageCount.value"
							:class="
								ctx.variant === 'web'
									? 'browse-pagination justify-end'
									: 'browse-pagination pagination-after'
							"
							@switch-page="ctx.setPage"
						/>
					</div>
				</div>
			</div>
		</AppFreezeFrameTransition>

		<slot name="after" />
	</div>
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

.search {
	position: relative;
}

.browse-sticky-controls :deep(.browse-search-field input::placeholder) {
	color: transparent;
	opacity: 0;
}

.browse-sticky-controls :deep(.browse-search-placeholder),
.browse-sticky-controls :deep(.browse-pagination),
.browse-sticky-controls :deep(.browse-filter-chips) {
	transition:
		opacity 120ms cubic-bezier(0.4, 0, 0.2, 1),
		color 120ms cubic-bezier(0.4, 0, 0.2, 1);
}

.browse-sticky-controls--transitioning :deep(.browse-pagination),
.browse-sticky-controls--transitioning :deep(.browse-filter-chips) {
	opacity: 0.45;
}

@media (prefers-reduced-motion: reduce) {
	.browse-sticky-controls :deep(.browse-search-placeholder),
	.browse-sticky-controls :deep(.browse-pagination),
	.browse-sticky-controls :deep(.browse-filter-chips) {
		transition-duration: 1ms;
	}
}
</style>
