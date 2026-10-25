<script setup lang="ts">
import { defineMessages, useScrollIndicator, useVIntl } from '@modrinth/ui'
import { OverlayScrollbars } from 'overlayscrollbars'
import { computed, onMounted, onScopeDispose, ref } from 'vue'

import { useTheme } from '@/composables/use-theme'
import PrideFundraiserBanner from '@/features/campaigns/pride-fundraiser-banner.vue'
import FriendsList from '@/features/friends/friends-list/index.vue'
import AccountsCard from '@/features/minecraft-accounts/accounts-card.vue'
import type { MinecraftAccountsActions } from '@/features/minecraft-accounts/context'
import NewsSidebar from '@/features/news/news-sidebar.vue'
import OnboardingChecklist from '@/features/onboarding/checklist.vue'
import type { ModrinthCredentials } from '@/helpers/mr_auth'
import { injectOnboardingChecklist } from '@/providers/onboarding-checklist'

defineProps<{
	credentials: ModrinthCredentials | null | undefined
	visible: boolean
	requestSignIn: () => void
}>()

const emit = defineEmits<{
	'create-instance': []
	'login-modrinth': []
}>()

const { formatMessage } = useVIntl()
const appTheme = useTheme()
const { hasLoggedIntoMinecraft, hasLoggedIntoModrinth, showChecklist } = injectOnboardingChecklist()
const showFriendsList = computed(() => !showChecklist.value || hasLoggedIntoModrinth.value)
const accounts = ref<InstanceType<typeof AccountsCard> | null>(null)
const friends = ref<InstanceType<typeof FriendsList> | null>(null)
const scrollContainer = ref<HTMLElement | null>(null)
const scrollViewport = ref<HTMLElement | null>(null)
const { showTopFade, showBottomFade, checkScrollState } = useScrollIndicator(scrollViewport)

let removeScrollbarListener: (() => void) | undefined

onMounted(() => {
	if (!scrollContainer.value) return
	const scrollbars = OverlayScrollbars(scrollContainer.value)
	if (!scrollbars) return
	scrollViewport.value = scrollbars.elements().viewport
	removeScrollbarListener = scrollbars.on('updated', checkScrollState)
})

onScopeDispose(() => removeScrollbarListener?.())

const minecraftAccounts = computed<MinecraftAccountsActions | null>(() => {
	const card = accounts.value
	if (!card) return null
	return {
		loginDisabled: card.loginDisabled,
		refreshValues: card.refreshValues,
		setEquippedSkin: card.setEquippedSkin,
		setLoginDisabled: card.setLoginDisabled,
		login: card.login,
	}
})

function showAddFriendModal() {
	friends.value?.showAddFriendModal()
}

defineExpose({ minecraftAccounts, showAddFriendModal })

const scrollbarOptions = Object.freeze({
	overflow: {
		x: 'hidden',
		y: 'scroll',
	},
})

const messages = defineMessages({
	playingAs: {
		id: 'app.sidebar.playing-as',
		defaultMessage: 'Playing as',
	},
})
</script>

<template>
	<div
		class="app-sidebar mt-px relative flex h-[calc(100vh-var(--top-bar-height))] w-[300px] shrink-0 flex-col overflow-visible border-0 border-l-[1px] border-solid border-[--brand-gradient-border] before:pointer-events-none before:absolute before:inset-y-0 before:-left-8 before:w-8 before:content-['']"
		:class="{
			'before:shadow-[-15px_0_15px_-15px_rgba(0,0,0,0.1)_inset]': appTheme.advancedRendering,
		}"
	>
		<div class="sidebar-scroll-area relative flex min-h-0 shrink flex-col">
			<div
				ref="scrollContainer"
				v-overlay-scrollbars="scrollbarOptions"
				class="app-sidebar-scrollable relative min-h-0 grow shrink"
				data-overlayscrollbars-initialize
			>
				<OnboardingChecklist
					@create-instance="emit('create-instance')"
					@login-minecraft="accounts?.login()"
					@login-modrinth="emit('login-modrinth')"
				/>
				<div id="sidebar-teleport-target" class="sidebar-teleport-content empty:hidden"></div>
				<div class="sidebar-default-content hidden" :class="{ 'sidebar-enabled': visible }">
					<div
						v-show="hasLoggedIntoMinecraft"
						class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid"
					>
						<h3 class="text-base text-primary font-medium m-0">
							{{ formatMessage(messages.playingAs) }}
						</h3>
						<Suspense>
							<AccountsCard ref="accounts" />
						</Suspense>
					</div>
					<div
						v-show="showFriendsList"
						class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid"
					>
						<Suspense>
							<FriendsList
								ref="friends"
								:credentials="credentials ?? null"
								:sign-in="requestSignIn"
							/>
						</Suspense>
					</div>
					<PrideFundraiserBanner
						class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid"
					/>
				</div>
			</div>
			<div
				class="sidebar-scroll-fade pointer-events-none absolute inset-x-0 top-0 z-10 h-8 rotate-180 transition-opacity duration-200 motion-reduce:transition-none"
				:class="showTopFade ? 'opacity-100' : 'opacity-0'"
				aria-hidden="true"
			/>
			<div
				class="sidebar-scroll-fade pointer-events-none absolute inset-x-0 bottom-0 z-10 h-8 transition-opacity duration-200 motion-reduce:transition-none"
				:class="showBottomFade ? 'opacity-100' : 'opacity-0'"
				aria-hidden="true"
			/>
		</div>
		<NewsSidebar v-if="visible" />
	</div>
</template>

<style scoped>
.app-sidebar {
	background: var(--brand-gradient-bg);
	--color-button-bg: var(--brand-gradient-button);
	--surface-4: var(--brand-gradient-button);
	--color-button-bg-hover: var(--brand-gradient-border);
	--surface-5: var(--brand-gradient-border);
	--color-divider: var(--brand-gradient-border);
	--color-divider-dark: var(--brand-gradient-border);
}

.sidebar-teleport-content:empty + .sidebar-default-content.sidebar-enabled {
	display: contents;
}

.sidebar-scroll-fade {
	background: var(--brand-gradient-fade-out-color);
}
</style>

<style>
/* News belongs to the default sidebar, so it hides when a page fills the sidebar itself. */
.app-sidebar:has(.sidebar-teleport-content:not(:empty)) .sidebar-news {
	display: none;
}

/*
 * The sidebar's free space belongs to the scroll area while news is closed and to the news body
 * while it is open. Swapping flex-grow between them animates the news open and closed and leaves
 * no gap above it. The minimum height keeps news visible when the sidebar content is long.
 */
.sidebar-scroll-area {
	flex-grow: 1;
}

.sidebar-news-body {
	display: flex;
	flex-direction: column;
	flex: 0 1 0;
	min-height: 0;
	overflow: hidden;
}

.sidebar-news-body.open {
	flex-grow: 1;
	min-height: 40%;
}

.app-sidebar:not(:has(.sidebar-teleport-content:not(:empty))):has(.sidebar-news-body.open)
	.sidebar-scroll-area {
	flex-grow: 0;
}

@media (prefers-reduced-motion: no-preference) {
	.sidebar-scroll-area,
	.sidebar-news-body {
		transition:
			flex-grow 220ms cubic-bezier(0.16, 1, 0.3, 1),
			min-height 220ms cubic-bezier(0.16, 1, 0.3, 1);
	}
}
</style>
