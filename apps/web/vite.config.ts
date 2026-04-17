import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'
import { defineConfig } from 'vite'
import svgLoader from 'vite-svg-loader'

const tauriMock = resolve(__dirname, 'src/mocks/tauri-apps.ts')

export default defineConfig({
	root: resolve(__dirname),
	css: {
		preprocessorOptions: {
			scss: {
				silenceDeprecations: ['import'],
				api: 'legacy',
			},
		},
	},
	resolve: {
		alias: [
			{
				find: '@',
				replacement: resolve(__dirname, 'src'),
			},
			{
				find: '@modrinth/ui',
				replacement: resolve(__dirname, 'packages/ui'),
			},
			{
				find: '@modrinth/ui/src',
				replacement: resolve(__dirname, 'packages/ui/src'),
			},
			{
				find: '@modrinth/assets',
				replacement: resolve(__dirname, 'packages/assets'),
			},
			{
				find: '@modrinth/utils',
				replacement: resolve(__dirname, 'packages/utils'),
			},
			{
				find: '@modrinth/api-client',
				replacement: resolve(__dirname, 'packages/api-client'),
			},
			{
				find: '@modrinth/blog',
				replacement: resolve(__dirname, 'packages/blog/index.ts'),
			},
			{
				find: '@modrinth/tooling-config',
				replacement: resolve(__dirname, 'packages/tooling-config'),
			},
			{
				find: '#ui',
				replacement: resolve(__dirname, 'packages/ui/src'),
			},
			// Tauri mocks
			{ find: '@tauri-apps/api/core', replacement: tauriMock },
			{ find: '@tauri-apps/api/app', replacement: tauriMock },
			{ find: '@tauri-apps/api/window', replacement: tauriMock },
			{ find: '@tauri-apps/api/event', replacement: tauriMock },
			{ find: '@tauri-apps/api/webview', replacement: tauriMock },
			{ find: '@tauri-apps/plugin-opener', replacement: tauriMock },
			{ find: '@tauri-apps/plugin-dialog', replacement: tauriMock },
			{ find: '@tauri-apps/plugin-os', replacement: tauriMock },
			{ find: '@tauri-apps/plugin-fs', replacement: tauriMock },
			{ find: '@tauri-apps/plugin-window-state', replacement: tauriMock },
		],
	},
	plugins: [
		vue(),
		svgLoader({
			svgoConfig: {
				plugins: [
					{
						name: 'preset-default',
						params: {
							overrides: {
								removeViewBox: false,
							},
						},
					},
				],
			},
		}),
	],
	clearScreen: false,
	server: {
		port: 5173,
		fs: {
			allow: [resolve(__dirname)],
		},
	},
	build: {
		target: 'es2020',
	},
})
