import config from '@modrinth/tooling-config/eslint/nuxt.mjs'

export default config.append([
	{
		rules: {
			'turbo/no-undeclared-env-vars': ['error', { allowList: ['^DEV$', '^PROD$'] }],
		},
	},
])
