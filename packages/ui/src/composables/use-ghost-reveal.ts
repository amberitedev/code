import { computed, ref, watchEffect } from 'vue'

/**
 * Class for the container that replaces ghost placeholders, so it fades in once instead of
 * popping. Empty until `ghosted` has been true, which keeps cached content rendering with no fade.
 *
 * Bind it on the element that mounts when the ghosts go away: `:class="reveal"`.
 */
export function useGhostReveal(ghosted: () => boolean) {
	const hadGhosts = ref(false)
	watchEffect(() => {
		if (ghosted()) hadGhosts.value = true
	})
	return computed(() =>
		hadGhosts.value
			? 'transition-opacity duration-150 motion-reduce:transition-none [@starting-style]:opacity-0'
			: '',
	)
}
