import preset from './packages/tooling-config/tailwind/tailwind-preset.ts'
import type { Config } from 'tailwindcss'

const config: Config = {
	content: [
		'./src/**/*.{js,vue,ts}',
		'./src/pages/**/*.vue',
		'./src/plugins/**/*.{js,ts}',
		'./src/App.vue',
		'./packages/ui/src/**/*.{js,vue,ts}',
		'./packages/assets/**/*.{js,vue,ts,scss,css}',
	],
	presets: [preset],
}

export default config
