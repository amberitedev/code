<script setup lang="ts">
import { ChevronDownIcon, NewspaperIcon } from '@modrinth/assets'
import { ButtonLink, defineMessages, NewsArticleCard, useVIntl } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { useStorage } from '@vueuse/core'
import { watch } from 'vue'

import { newsArticlesQueryOptions } from './queries'

/**
 * Modrinth news, pinned to the bottom of the sidebar. Collapsing it leaves only its header. The
 * sidebar shell gives the news body the free space while it is open (see `.sidebar-news-body`).
 */
const { formatMessage } = useVIntl()
const messages = defineMessages({
	news: {
		id: 'app.news.modrinth-title',
		defaultMessage: 'Modrinth News',
	},
	viewAllNews: {
		id: 'app.news.view-all',
		defaultMessage: 'View all news',
	},
})

const collapsed = useStorage('sidebar-news-collapsed', false)

const { data: news, error } = useQuery({
	...newsArticlesQueryOptions(),
	select: (articles) => articles.slice(0, 4),
})

watch(error, (error) => {
	if (error) console.error('Failed to fetch news articles', error)
})
</script>

<template>
	<div v-if="news && news.length > 0" class="sidebar-news contents">
		<button
			type="button"
			class="-mt-px flex w-full shrink-0 cursor-pointer items-center justify-between border-0 border-t-[1px] border-solid border-[--brand-gradient-border] bg-transparent px-4 py-3 text-base font-medium text-primary hover:text-contrast"
			:aria-expanded="!collapsed"
			@click="collapsed = !collapsed"
		>
			{{ formatMessage(messages.news) }}
			<ChevronDownIcon
				class="size-5 text-secondary transition-transform duration-[220ms] ease-[cubic-bezier(0.16,1,0.3,1)]"
				:class="{ 'rotate-180': collapsed }"
			/>
		</button>
		<div class="sidebar-news-body" :class="{ open: !collapsed }" :inert="collapsed">
			<div class="flex min-h-0 flex-1 flex-col items-center space-y-4 overflow-y-auto px-4 pb-4">
				<NewsArticleCard v-for="item in news" :key="item.path" :article="item" />
				<ButtonLink
					type="colored"
					color="brand"
					size="xl"
					href="https://modrinth.com/news"
					target="_blank"
				>
					<NewspaperIcon />
					{{ formatMessage(messages.viewAllNews) }}
				</ButtonLink>
			</div>
		</div>
	</div>
</template>
