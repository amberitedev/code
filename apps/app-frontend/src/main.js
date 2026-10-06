import 'overlayscrollbars/overlayscrollbars.css'

import { installTooltipDirective } from '@modrinth/ui'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createApp } from 'vue'

import { overlayScrollbarsDirective } from '@/directives/overlayScrollbars'
import { setupErrorReporting } from '@/helpers/error-reporting'
import { debugStartup, traceStartupStep } from '@/helpers/startup-debug'
import i18nPlugin from '@/plugins/i18n'
import i18nDebugPlugin from '@/plugins/i18n-debug'

import { config } from '@/config'

async function bootstrap() {
	if (import.meta.env.DEV) {
		const { initializeDevRuntime } = await import('@/dev/runtime')
		await initializeDevRuntime()
	}
	const [{ default: App }, { default: router }] = await Promise.all([
		import('@/App.vue'), import('@/routes'),
	])
	debugStartup('Frontend entry module evaluated')
	const app = createApp(App)
	if (!config.accountApiUrl) setupErrorReporting(app, router)

	app.use(VueQueryPlugin)
	app.use(router)
	if (import.meta.env.DEV) {
		void traceStartupStep('Initial router readiness', () => router.isReady()).catch(() => {})
	}
	app.use(i18nPlugin)
	app.use(i18nDebugPlugin)
	installTooltipDirective(app)
	app.directive('overlay-scrollbars', overlayScrollbarsDirective)

	async function mount() {
		if (import.meta.env.DEV && import.meta.env.VITE_VUE_SCAN === 'true') {
			const { VueScanPlugin } = await traceStartupStep(
				'Load Vue render tracker',
				() => import('@taijased/vue-render-tracker'),
			)
			app.use(new VueScanPlugin({ enabled: true, showOverlay: true, log: false, playSound: false }))
		}
		debugStartup('Vue mount started')
		app.mount('#app')
		debugStartup('Vue mount completed')
	}

	await mount()
}

void bootstrap()
