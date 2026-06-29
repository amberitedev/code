import vue from '@vitejs/plugin-vue'
import { existsSync, readFileSync } from 'fs'
import { resolve } from 'path'
import { defineConfig } from 'vite'
import svgLoader from 'vite-svg-loader'

import tauriConf from '../app/tauri.conf.json'

const projectRootDir = resolve(__dirname)
const appLibEnvDir = resolve(projectRootDir, '../../packages/app-lib')
const apiClientSource = resolve(projectRootDir, '../../packages/api-client/src/index.ts')
const devPort = Number(process.env.AMBERITE_APP_DEV_PORT ?? 1420)

const requiredEnvironmentVariables = [
	'MODRINTH_URL',
	'MODRINTH_API_BASE_URL',
	'MODRINTH_ARCHON_BASE_URL',
	'VITE_CONVEX_URL',
] as const

function loadEnvFile(envFilePath: string) {
	if (existsSync(envFilePath)) {
		for (const line of readFileSync(envFilePath, 'utf-8').split('\n')) {
			const withoutComment = line.split('#')[0]?.trim()
			if (!withoutComment) continue
			const eqIndex = withoutComment.indexOf('=')
			if (eqIndex === -1) continue
			const key = withoutComment.slice(0, eqIndex)
			const value = withoutComment.slice(eqIndex + 1)
			if (!(key in process.env)) {
				process.env[key] = value
			}
		}
	}
}

// Load .env from app-lib manually instead of using Vite's envDir, which would auto-load .env.local and override values
loadEnvFile(resolve(appLibEnvDir, '.env'))

const missingEnvironmentVariables = requiredEnvironmentVariables.filter(
	(key) => !process.env[key]?.trim(),
)

if (missingEnvironmentVariables.length > 0) {
	throw new Error(
		`Missing required environment variables: ${missingEnvironmentVariables.join(', ')}. ` +
			'Set them in packages/app-lib/.env.',
	)
}

// https://vitejs.dev/config/
export default defineConfig({
	css: {
		preprocessorOptions: {
			scss: {
				// TODO: dont forget about this
				silenceDeprecations: ['import'],
			},
		},
	},
	resolve: {
		alias: [
			{
				find: '@modrinth/api-client',
				replacement: apiClientSource,
			},
			{
				find: '@',
				replacement: resolve(projectRootDir, 'src'),
			},
		],
	},
	plugins: [
		vue({
			template: {
				compilerOptions: {
					isCustomElement: (tag) =>
						(tag.startsWith('Tres') && tag !== 'TresCanvas') || tag === 'primitive',
				},
			},
		}),
		svgLoader({
			svgoConfig: {
				plugins: [
					{
						name: 'preset-default',
						params: {
							overrides: {
								removeViewBox: false,
								cleanupIds: {
									minify: false,
								},
							},
						},
					},
				],
			},
		}),
	],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	// prevent vite from obscuring rust errors
	clearScreen: false,
	// tauri expects a fixed port, fail if that port is not available
	server: {
		port: devPort,
		strictPort: true,
		headers: {
			'content-security-policy': Object.entries(tauriConf.app.security.csp)
				.map(([directive, sources]) => {
					// An additional websocket connect-src is required for Vite dev tools to work
					if (directive === 'connect-src') {
						sources = Array.isArray(sources) ? sources : [sources]
						sources.push(`ws://localhost:${devPort}`)
					}

					return Array.isArray(sources)
						? `${directive} ${sources.join(' ')}`
						: `${directive} ${sources}`
				})
				.join('; '),
		},
	},
	// to make use of `TAURI_ENV_DEBUG` and other env variables
	// https://v2.tauri.app/reference/environment-variables/#tauri-cli-hook-commands
	envPrefix: ['VITE_', 'TAURI_', 'MODRINTH_'],
	build: {
		rolldownOptions: {
			onwarn(warning, defaultHandler) {
				if (warning.code === 'INEFFECTIVE_DYNAMIC_IMPORT') return
				defaultHandler(warning)
			},
		},
		// Tauri supports es2021
		target: process.env.TAURI_ENV_PLATFORM == 'windows' ? 'chrome105' : 'safari13', // eslint-disable-line turbo/no-undeclared-env-vars
		// don't minify for debug builds
		minify: !process.env.TAURI_ENV_DEBUG, // eslint-disable-line turbo/no-undeclared-env-vars
		// produce sourcemaps for debug builds
		sourcemap: !!process.env.TAURI_ENV_DEBUG, // eslint-disable-line turbo/no-undeclared-env-vars
	},
})
