import type { StorybookConfig } from '@storybook/vue3-vite'

const config: StorybookConfig = {
	framework: {
		name: '@storybook/vue3-vite',
		options: {
			docgen: true,
		},
	},
	stories: ['../src/**/*.stories.@(js|jsx|mjs|ts|tsx)'],
	addons: ['@storybook/addon-docs', '@storybook/addon-themes', '@storybook/addon-a11y'],
	viteFinal: async (viteConfig) => {
		// vue-docgen-api (babel-based) throws on some TS constructs — catch and skip
		// rather than aborting the whole build.
		const plugins = (viteConfig.plugins ?? []).flat(Infinity as 1)
		for (const plugin of plugins) {
			if (
				plugin &&
				typeof plugin === 'object' &&
				'name' in plugin &&
				plugin.name === 'storybook:vue-docgen-plugin'
			) {
				const original = (plugin as { transform?: Function }).transform
				if (typeof original === 'function') {
					;(plugin as { transform: Function }).transform = async function (...args: unknown[]) {
						try {
							return await original.apply(this, args)
						} catch (e) {
							console.warn(`[docgen] parse error in ${args[1]}, skipping: ${(e as Error).message}`)
							return null
						}
					}
				}
			}
		}
		return viteConfig
	},
}
export default config
