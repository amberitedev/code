import 'overlayscrollbars/overlayscrollbars.css'

import { installTooltipDirective } from '@modrinth/ui'
import { focusManager, VueQueryPlugin } from '@tanstack/vue-query'
import { invoke } from '@tauri-apps/api/core'
import { createApp } from 'vue'

import { applyDevAppConfig, config } from '@/config'
import { overlayScrollbarsDirective } from '@/directives/overlayScrollbars'
import { installCrashScreen } from '@/features/crash'
import { setupErrorReporting } from '@/helpers/error-reporting'
import { debugStartup, traceStartupStep } from '@/helpers/startup-debug'
import { loadBoot } from '@/platform/os'
import { getCurrentWindow } from '@/platform/window'
import i18nPlugin from '@/plugins/i18n'
import i18nDebugPlugin from '@/plugins/i18n-debug'

async function bootstrap() {
	const boot = await loadBoot()
	applyDevAppConfig({ backendUrl: boot.backend_url ?? undefined })
	const [{ default: App }, { default: router }] = await Promise.all([
		import('@/App.vue'),
		import('@/routes'),
	])
	debugStartup('Frontend entry module evaluated')
	const app = createApp(App)
	installCrashScreen(app, router)
	if (!config.accountApiUrl) setupErrorReporting(app, router)

	focusManager.setEventListener((setFocused) => {
		const unlisten = getCurrentWindow().onFocusChanged(({ payload }) => setFocused(payload))
		return () => void unlisten.then((stop) => stop())
	})

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

// Until App.vue loads there is no crash screen, and a release window stays hidden.
bootstrap().catch((error) => {
	console.error(error)
	const reason = error instanceof Error ? error.message : String(error)
	document.getElementById('app')?.replaceChildren(`Amberite could not start. ${reason}`)
	void invoke('show_window').catch(() => {})
})
