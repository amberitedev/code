import 'floating-vue/dist/style.css'
import 'overlayscrollbars/overlayscrollbars.css'

import * as Sentry from '@sentry/vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import FloatingVue from 'floating-vue'
import { createPinia } from 'pinia'
import { createApp } from 'vue'

import App from '@/App.vue'
import { installAmberiteDevApi } from '@/dev/amberiteDevApi'
import { overlayScrollbarsDirective } from '@/directives/overlayScrollbars'
import i18nPlugin from '@/plugins/i18n'
import i18nDebugPlugin from '@/plugins/i18n-debug'
import router from '@/routes'

const pinia = createPinia()

let app = createApp(App)

Sentry.init({
	app,
	dsn: 'https://9508775ee5034536bc70433f5f531dd4@o485889.ingest.us.sentry.io/4504579615227904',
	integrations: [Sentry.browserTracingIntegration({ router })],
	tracesSampleRate: 0.1,
})

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

installAmberiteDevApi()
