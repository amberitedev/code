<script setup lang="ts">
defineOptions({
	name: 'AppPageSkeleton',
})

withDefaults(
	defineProps<{
		variant?: 'page' | 'list' | 'detail'
	}>(),
	{
		variant: 'page',
	},
)
</script>

<template>
	<div class="app-page-skeleton p-6" :class="`app-page-skeleton-${variant}`" aria-hidden="true">
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
	</div>
</template>

<style scoped>
.app-page-skeleton {
	display: flex;
	flex-direction: column;
	gap: 1rem;
}

.app-page-skeleton-header,
.app-page-skeleton-card {
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

.app-page-skeleton-line {
	border-radius: var(--radius-md);
}

.app-page-skeleton-icon::before,
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
.app-page-skeleton-grid {
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

.app-page-skeleton-grid {
	display: grid;
	grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
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
	.app-page-skeleton-line,
	.app-page-skeleton-icon::before,
	.app-page-skeleton-line::before {
		animation: none;
	}
}
</style>
