import 'floating-vue/dist/style.css'
import 'overlayscrollbars/overlayscrollbars.css'

import * as Sentry from '@sentry/vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import FloatingVue from 'floating-vue'
import { createPinia } from 'pinia'
import { createApp } from 'vue'

import { overlayScrollbarsDirective } from '@/directives/overlayScrollbars'
import i18nPlugin from '@/plugins/i18n'
import i18nDebugPlugin from '@/plugins/i18n-debug'

const pinia = createPinia()

void bootstrap()

async function bootstrap() {
	if (import.meta.env.DEV) {
		const { initializeDevRuntime } = await import('@/dev/runtime')
		await initializeDevRuntime()
	}

	const [{ default: App }, { default: router }] = await Promise.all([
		import('@/App.vue'),
		import('@/routes'),
	])
	let app = createApp(App)

	function isLocalHost() {
		const { hostname } = window.location
		return (
			hostname === 'localhost' ||
			hostname.endsWith('.localhost') ||
			hostname === '127.0.0.1' ||
			hostname === '::1'
		)
	}

	function shouldEnableSentry() {
		if (import.meta.env.VITE_ENABLE_SENTRY === 'true') return true
		return !import.meta.env.DEV && !isLocalHost()
	}

	const sentryDsn = import.meta.env.VITE_SENTRY_DSN?.trim()
	if (sentryDsn && shouldEnableSentry()) {
		Sentry.init({
			app,
			dsn: sentryDsn,
			integrations: [Sentry.browserTracingIntegration({ router })],
			tracesSampleRate: Number(import.meta.env.VITE_SENTRY_TRACES_SAMPLE_RATE ?? 0.1),
		})
	}

	app.use(VueQueryPlugin)
	app.use(router)
	app.use(pinia)
	app.use(FloatingVue, {
		themes: {
			'ribbit-popout': {
				$extend: 'dropdown',
				placement: 'bottom-end',
				instantMove: true,
				distance: 8,
			},
			'dismissable-prompt': {
				$extend: 'dropdown',
				placement: 'bottom-start',
			},
		},
	})
	app.use(i18nPlugin)
	app.use(i18nDebugPlugin)
	app.directive('overlay-scrollbars', overlayScrollbarsDirective)

	app.mount('#app')
}
