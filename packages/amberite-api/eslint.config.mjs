import config from '@modrinth/tooling-config/eslint/base.mjs'
export default [
	...config,
	{
		ignores: ['scripts/**'],
	},
]
