import { defineConfig } from 'vitest/config'

export default defineConfig({
	resolve: {
		alias: {
			'@modrinth/api-client': new URL('../api-client/src/index.ts', import.meta.url).pathname,
		},
	},
	test: {
		environment: 'node',
		include: ['src/test/**/*.test.ts'],
		exclude: ['src/test/client.test.ts'],
	},
})
