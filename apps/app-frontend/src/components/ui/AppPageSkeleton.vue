<script setup lang="ts">
defineOptions({
	name: 'AppPageSkeleton',
})

withDefaults(
	defineProps<{
		variant?: 'page' | 'list' | 'detail' | 'home' | 'browse' | 'library' | 'core-overview'
	}>(),
	{
		variant: 'page',
	},
)
</script>

<template>
	<div class="app-page-skeleton p-6" :class="`app-page-skeleton-${variant}`" aria-hidden="true">
		<template v-if="variant === 'core-overview'">
			<div class="app-page-skeleton-tabs">
				<div v-for="i in 2" :key="i" class="app-page-skeleton-line h-9 w-28 rounded-full" />
			</div>
			<div class="app-page-skeleton-toolbar">
				<div class="app-page-skeleton-line h-10 min-w-60 flex-1 rounded-xl" />
				<div class="app-page-skeleton-line h-10 w-56 rounded-xl" />
				<div class="app-page-skeleton-line h-10 w-36 rounded-xl" />
			</div>
			<div class="app-page-skeleton-table">
				<div v-for="i in 6" :key="i" class="app-page-skeleton-user-row">
					<div class="app-page-skeleton-avatar" />
					<div class="app-page-skeleton-copy">
						<div class="app-page-skeleton-line h-4 w-40" />
						<div class="app-page-skeleton-line h-3 w-28 opacity-75" />
					</div>
					<div class="app-page-skeleton-line ml-auto h-7 w-24 rounded-full" />
					<div class="app-page-skeleton-line hidden h-7 w-28 rounded-full md:block" />
					<div class="app-page-skeleton-line h-9 w-9 rounded-full" />
				</div>
			</div>
		</template>

		<template v-else-if="variant === 'browse'">
			<div class="app-page-skeleton-browse-tabs">
				<div
					v-for="width in ['6.5rem', '4.25rem', '8.25rem', '6.75rem', '5.25rem', '5.5rem']"
					:key="width"
					class="app-page-skeleton-line h-8 rounded-full"
					:style="{ width }"
				/>
			</div>
			<div class="app-page-skeleton-line h-12 w-full rounded-xl opacity-75" />
			<div class="app-page-skeleton-browse-controls">
				<div class="app-page-skeleton-control-group">
					<div class="app-page-skeleton-line h-9 w-64 rounded-xl" />
					<div class="app-page-skeleton-line h-9 w-36 rounded-xl" />
				</div>
				<div class="app-page-skeleton-pagination">
					<div class="app-page-skeleton-line h-9 w-9 rounded-full" />
					<div class="app-page-skeleton-line h-8 w-8 rounded-full" />
					<div class="app-page-skeleton-line h-8 w-10 rounded-full" />
					<div class="app-page-skeleton-line h-8 w-12 rounded-full" />
					<div class="app-page-skeleton-line h-8 w-8 rounded-full" />
				</div>
			</div>
			<div class="app-page-skeleton-browse-list">
				<div v-for="i in 7" :key="i" class="app-page-skeleton-browse-row">
					<div class="app-page-skeleton-browse-icon" />
					<div class="app-page-skeleton-browse-copy">
						<div class="app-page-skeleton-heading-row">
							<div class="app-page-skeleton-line h-5 w-72 max-w-full" />
							<div class="app-page-skeleton-line h-4 w-32 opacity-60" />
						</div>
						<div class="app-page-skeleton-line h-4 w-full max-w-[48rem] opacity-75" />
						<div class="app-page-skeleton-browse-tags">
							<div class="app-page-skeleton-line h-6 w-24 rounded-full opacity-70" />
							<div class="app-page-skeleton-line h-6 w-28 rounded-full opacity-70" />
							<div class="app-page-skeleton-line h-6 w-24 rounded-full opacity-70" />
							<div class="app-page-skeleton-line h-6 w-10 rounded-full opacity-70" />
						</div>
					</div>
					<div class="app-page-skeleton-browse-actions">
						<div class="app-page-skeleton-line h-9 w-28 rounded-xl" />
						<div class="app-page-skeleton-line h-4 w-32 opacity-75" />
						<div class="app-page-skeleton-line h-4 w-28 opacity-75" />
						<div class="app-page-skeleton-line h-4 w-24 opacity-60" />
					</div>
				</div>
			</div>
		</template>

		<template v-else-if="variant === 'home' || variant === 'library'">
			<div class="app-page-skeleton-header">
				<div class="app-page-skeleton-icon" />
				<div class="app-page-skeleton-copy">
					<div class="app-page-skeleton-line h-6 w-56" />
					<div class="app-page-skeleton-line h-4 w-80 opacity-75" />
				</div>
			</div>
			<div v-if="variant === 'library'" class="app-page-skeleton-tabs">
				<div v-for="i in 4" :key="i" class="app-page-skeleton-line h-9 w-32 rounded-full" />
			</div>
			<div class="app-page-skeleton-grid">
				<div v-for="i in variant === 'home' ? 6 : 8" :key="i" class="app-page-skeleton-card">
					<div class="app-page-skeleton-line h-5 w-2/3" />
					<div class="app-page-skeleton-line h-4 w-full opacity-75" />
					<div class="app-page-skeleton-line h-4 w-4/5 opacity-60" />
				</div>
			</div>
		</template>

		<template v-else>
			<div class="app-page-skeleton-header">
				<div class="app-page-skeleton-icon" />
				<div class="app-page-skeleton-copy">
					<div class="app-page-skeleton-line h-6 w-56" />
					<div class="app-page-skeleton-line h-4 w-80 opacity-75" />
				</div>
				<div class="ml-auto hidden gap-2 sm:flex">
					<div class="app-page-skeleton-line h-10 w-28 rounded-xl" />
					<div class="app-page-skeleton-line h-10 w-10 rounded-xl" />
				</div>
			</div>

			<div class="app-page-skeleton-tabs">
				<div v-for="i in 4" :key="i" class="app-page-skeleton-line h-8 w-24 rounded-lg" />
			</div>

			<div class="app-page-skeleton-grid">
				<div v-for="i in variant === 'detail' ? 2 : 4" :key="i" class="app-page-skeleton-card">
					<div class="app-page-skeleton-line h-5 w-2/3" />
					<div class="app-page-skeleton-line h-4 w-full opacity-75" />
					<div class="app-page-skeleton-line h-4 w-4/5 opacity-60" />
				</div>
			</div>
		</template>
	</div>
</template>

<style scoped>
.app-page-skeleton {
	display: flex;
	flex-direction: column;
	gap: 1rem;
}

.app-page-skeleton-header,
.app-page-skeleton-card,
.app-page-skeleton-project-row,
.app-page-skeleton-user-row,
.app-page-skeleton-browse-row {
	display: flex;
	gap: 1rem;
	overflow: hidden;
	border: 1px solid var(--color-button-bg);
	border-radius: 1rem;
	background: var(--color-raised-bg);
	padding: 1rem;
}

.app-page-skeleton-card {
	min-height: 8rem;
	flex-direction: column;
}

.app-page-skeleton-icon,
.app-page-skeleton-avatar,
.app-page-skeleton-browse-icon,
.app-page-skeleton-line {
	position: relative;
	overflow: hidden;
	background: var(--color-button-bg);
	animation: app-page-skeleton-pulse 4s ease-in-out infinite;
}

.app-page-skeleton-icon {
	width: 4rem;
	height: 4rem;
	flex: 0 0 auto;
	border-radius: var(--radius-lg);
}

.app-page-skeleton-avatar {
	width: 2.5rem;
	height: 2.5rem;
	flex: 0 0 auto;
	border-radius: 9999px;
}

.app-page-skeleton-browse-icon {
	width: 6.25rem;
	height: 6.25rem;
	flex: 0 0 auto;
	border-radius: var(--radius-lg);
}

.app-page-skeleton-line {
	border-radius: var(--radius-md);
}

.app-page-skeleton-icon::before,
.app-page-skeleton-avatar::before,
.app-page-skeleton-browse-icon::before,
.app-page-skeleton-line::before {
	content: '';
	position: absolute;
	inset: 0;
	background-image: linear-gradient(
		-45deg,
		transparent 30%,
		rgba(196, 217, 237, 0.075) 50%,
		transparent 70%
	);
	animation: app-page-skeleton-shimmer 4s ease-in-out infinite;
}

.app-page-skeleton-copy,
.app-page-skeleton-tabs,
.app-page-skeleton-browse-tabs,
.app-page-skeleton-grid,
.app-page-skeleton-toolbar,
.app-page-skeleton-list,
.app-page-skeleton-table,
.app-page-skeleton-bottom-bar,
.app-page-skeleton-browse-controls,
.app-page-skeleton-control-group,
.app-page-skeleton-pagination,
.app-page-skeleton-browse-list,
.app-page-skeleton-browse-copy,
.app-page-skeleton-heading-row,
.app-page-skeleton-browse-tags,
.app-page-skeleton-browse-actions {
	display: flex;
	gap: 0.75rem;
}

.app-page-skeleton-copy {
	min-width: 0;
	flex: 1;
	flex-direction: column;
	justify-content: center;
}

.app-page-skeleton-tabs {
	flex-wrap: wrap;
}

.app-page-skeleton-browse-tabs {
	width: fit-content;
	max-width: 100%;
	flex-wrap: wrap;
	gap: 0.5rem;
	border-radius: 9999px;
	background: var(--color-raised-bg);
	padding: 0.25rem;
}

.app-page-skeleton-toolbar {
	flex-wrap: wrap;
	align-items: center;
}

.app-page-skeleton-grid {
	display: grid;
	grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
}

.app-page-skeleton-list,
.app-page-skeleton-table,
.app-page-skeleton-browse-list,
.app-page-skeleton-browse-copy,
.app-page-skeleton-browse-actions {
	flex-direction: column;
}

.app-page-skeleton-project-row,
.app-page-skeleton-user-row,
.app-page-skeleton-browse-row {
	align-items: center;
	padding: 0.875rem;
}

.app-page-skeleton-browse-row {
	min-height: 7.75rem;
	align-items: stretch;
}

.app-page-skeleton-browse-copy {
	min-width: 0;
	flex: 1;
	justify-content: space-between;
}

.app-page-skeleton-heading-row,
.app-page-skeleton-browse-controls,
.app-page-skeleton-control-group,
.app-page-skeleton-pagination,
.app-page-skeleton-browse-tags {
	align-items: center;
	flex-wrap: wrap;
}

.app-page-skeleton-browse-controls {
	justify-content: space-between;
}

.app-page-skeleton-browse-actions {
	width: 12rem;
	flex: 0 0 auto;
	align-items: flex-end;
	justify-content: space-between;
}

.app-page-skeleton-bottom-bar {
	justify-content: flex-end;
}

.app-page-skeleton-browse {
	min-height: calc(100vh - 7rem);
}

.app-page-skeleton-list .app-page-skeleton-grid,
.app-page-skeleton-detail .app-page-skeleton-grid {
	grid-template-columns: 1fr;
}

@keyframes app-page-skeleton-pulse {
	0%,
	100% {
		opacity: 0.45;
	}

	50% {
		opacity: 0.85;
	}
}

@keyframes app-page-skeleton-shimmer {
	0% {
		transform: translateX(-80%);
	}

	50%,
	100% {
		transform: translateX(80%);
	}
}

@media (prefers-reduced-motion: reduce) {
	.app-page-skeleton-icon,
	.app-page-skeleton-avatar,
	.app-page-skeleton-browse-icon,
	.app-page-skeleton-line,
	.app-page-skeleton-icon::before,
	.app-page-skeleton-avatar::before,
	.app-page-skeleton-browse-icon::before,
	.app-page-skeleton-line::before {
		animation: none;
	}
}

@media (max-width: 900px) {
	.app-page-skeleton-browse-actions {
		display: none;
	}
}
</style>
