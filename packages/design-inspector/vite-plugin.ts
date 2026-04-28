import { fileURLToPath } from 'node:url'
import type { Plugin } from 'vite'
import Inspector from 'vite-plugin-vue-inspector'

// virtual module ID that the inspector's AppInstaller imports for the overlay
const OVERLAY_VIRTUAL = 'virtual:vue-inspector-path:Overlay.vue'

// Absolute path to our custom overlay component
const overlayPath = fileURLToPath(new URL('./components/InspectorOverlay.vue', import.meta.url))

/**
 * enforce: 'pre' so our resolveId runs before vite-plugin-vue-inspector's,
 * intercepting the overlay virtual module and pointing it at our component.
 */
function overlayOverridePlugin(): Plugin {
	return {
		name: 'amberite-design-inspector-overlay',
		enforce: 'pre',
		resolveId(id) {
			if (id === OVERLAY_VIRTUAL) return overlayPath
		},
	}
}

/**
 * Returns the full Vite plugin array for the design inspector.
 * Include in vite.config.ts under plugins (dev-only recommended).
 */
export function designInspectorVitePlugin(): Plugin[] {
	return [
		overlayOverridePlugin(),
		Inspector({
			toggleButtonVisibility: 'never',
			enabled: false,
		}),
	]
}
