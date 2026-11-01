import { getVersion } from '@tauri-apps/api/app'
import { type App, createApp, ref } from 'vue'
import type { Router } from 'vue-router'

import CrashScreen from './crash-screen.vue'

/**
 * Bumped by "Try again". `App.vue` keys the current page with it, so the page is built again from
 * scratch.
 */
export const pageGeneration = ref(0)

/**
 * Where Vue says an error came from, for the errors that leave a page unable to show: a
 * component's setup or render, or a lazy component that failed to load. Vue names the source in
 * development and links its error code in production (`#runtime-0`, `-1`, `-13`).
 */
const pageBreaking = /^(setup function|render function|async component loader)$|#runtime-(0|1|13)$/

/**
 * Cover the App with a crash screen when a page cannot show, instead of leaving it blank. Errors
 * from handlers and watchers are only logged: the page is still usable. Every error is logged
 * with a `[crash]` or `[error]` prefix. Call before any other error reporting is set up, which
 * wraps this handler.
 */
export function installCrashScreen(app: App, router: Router) {
	// Asked once up front: the screen should not wait on a native call after a crash.
	const version = getVersion().catch(() => 'unknown')
	let crashed = false

	async function show(error: unknown, source: string) {
		console.error(`[crash] ${source}`, error)
		if (crashed) return
		crashed = true
		const message = error instanceof Error ? error.message : String(error)
		const report = [
			`Amberite ${await version}`,
			`Route: ${router.currentRoute.value.fullPath}`,
			`Source: ${source}`,
			error instanceof Error ? (error.stack ?? message) : message,
		].join('\n')
		const root = document.body.appendChild(document.createElement('div'))
		const screen = createApp(CrashScreen, {
			message,
			report,
			retry: () => {
				screen.unmount()
				root.remove()
				crashed = false
				pageGeneration.value++
			},
		})
		screen.mount(root)
	}

	app.config.errorHandler = (error, _instance, info) => {
		if (pageBreaking.test(info)) void show(error, info)
		else console.error(`[error] ${info}`, error)
	}
	router.onError((error) => void show(error, 'navigation'))
}
