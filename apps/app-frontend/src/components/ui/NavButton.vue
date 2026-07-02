<template>
	<RouterLink
		v-if="typeof to === 'string'"
		:to="to"
		v-bind="$attrs"
		:active-class="isSubpage ? '' : undefined"
		:class="{
			'router-link-active': isPrimary && isPrimary(route),
			'subpage-active': isSubpage && isSubpage(route),
			disabled: disabled,
		}"
		class="app-nav-button relative z-[1] w-12 h-12 text-primary rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-button-bgHover hover:text-contrast"
		@mouseenter="preloadRoute"
		@focus="preloadRoute"
	>
		<slot />
	</RouterLink>
	<button
		v-else
		v-bind="$attrs"
		class="app-nav-button button-animation relative z-[1] border-none text-primary cursor-pointer w-12 h-12 rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-button-bgHover hover:text-contrast"
		:disabled="disabled"
		@click="to"
	>
		<slot />
	</button>
</template>

<script setup lang="ts">
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import { RouterLink, useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

type RouteFunction = (route: RouteLocationNormalizedLoaded) => boolean

const props = withDefaults(
	defineProps<{
		to: (() => void) | string
		isPrimary?: RouteFunction
		isSubpage?: RouteFunction
		highlightOverride?: boolean
		disabled?: boolean
	}>(),
	{
		disabled: false,
		isPrimary: undefined,
		isSubpage: undefined,
		highlightOverride: false,
	},
)

let preloaded = false

async function preloadRoute() {
	if (preloaded || props.disabled || typeof props.to !== 'string') return
	preloaded = true
	const resolved = router.resolve(props.to)
	await Promise.all(
		resolved.matched.flatMap((record) =>
			Object.values(record.components ?? {}).map((component) =>
				typeof component === 'function' ? component() : Promise.resolve(),
			),
		),
	).catch(() => {
		preloaded = false
	})
}

defineOptions({
	inheritAttrs: false,
})
</script>

<style lang="scss" scoped>
.router-link-active,
.subpage-active {
	svg {
		filter: drop-shadow(0 0 0.5rem black);
	}
}

.router-link-active {
	@apply text-[--color-button-text-selected];
	background: var(--nav-button-active-bg, var(--color-button-bg-selected));
	box-shadow: var(
		--nav-button-active-shadow,
		0 0 0 1px color-mix(in srgb, var(--color-brand) 38%, transparent),
		0 0 18px color-mix(in srgb, var(--color-brand) 24%, transparent)
	);
}

.router-link-active:hover {
	@apply text-[--color-button-text-selected];
	background: var(--nav-button-active-bg, var(--color-button-bg-selected));
}

.subpage-active {
	@apply text-contrast;
	background: var(--nav-button-subpage-bg, var(--color-button-bg));
}

.subpage-active:hover {
	@apply text-contrast;
	background: var(--nav-button-subpage-bg, var(--color-button-bg));
}
</style>
