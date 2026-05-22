import { defineConfig } from 'vitest/config'

export default defineConfig({
	test: {
		environment: 'node',
		include: ['src/test/**/*.test.ts'],
		exclude: ['src/test/client.test.ts'],
	},
})
