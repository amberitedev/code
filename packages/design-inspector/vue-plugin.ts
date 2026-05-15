import { createApp, type App, type Plugin } from 'vue'
import DesignInspectorRoot from './components/DesignInspectorRoot.vue'

/**
 * Vue plugin that mounts DesignInspectorRoot as a separate app
 * into a dedicated #design-inspector-ui div at the body level.
 * This keeps the inspector overlay isolated from the main app.
 */
export const DesignInspectorPlugin: Plugin = {
	install(_app: App) {
		if (typeof window === 'undefined') return
		if (!import.meta.env.DEV || import.meta.env.VITE_ENABLE_DESIGN_INSPECTOR !== 'true') return
		if (document.getElementById('design-inspector-ui')) return

		const container = document.createElement('div')
		container.id = 'design-inspector-ui'
		document.body.appendChild(container)

		createApp(DesignInspectorRoot).mount(container)
	},
}
