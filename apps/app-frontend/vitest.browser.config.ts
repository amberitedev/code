// The App's browser tests: `*.browser.test.ts` files that render components in a real Chromium,
// with the App's own plugins, import shortcuts and styles. See docs/internals/testing.md.
import * as NodePath from 'node:path'

import autoprefixer from 'autoprefixer'
import tailwindcss from 'tailwindcss'
import { playwright } from 'vite-plus/test/browser-playwright'
import { defineProject } from 'vite-plus/test/config'

import app from './vite.config.ts'

export default defineProject({
	// Not the App's `server`: its fixed port and content security policy are the dev server's.
	plugins: app.plugins,
	resolve: app.resolve,
	optimizeDeps: app.optimizeDeps,
	css: {
		...app.css,
		// Named here because tests run from the repository root, where PostCSS finds no config.
		postcss: {
			plugins: [
				tailwindcss(NodePath.join(import.meta.dirname, 'tailwind.config.ts')),
				autoprefixer(),
			],
		},
	},
	test: {
		name: 'browser',
		include: ['src/**/*.browser.test.ts'],
		setupFiles: ['./test/browser-setup.ts'],
		browser: {
			enabled: true,
			headless: true,
			provider: playwright(),
			instances: [{ browser: 'chromium' }],
			screenshotFailures: false,
		},
	},
})
