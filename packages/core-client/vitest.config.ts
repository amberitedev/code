import { defineConfig } from 'vitest/config'

export default defineConfig({
	test: {
		environment: 'node',
		testTimeout: 30_000,
		globalSetup: ['./src/test/global-setup.ts'],
		include: ['src/test/**/*.test.ts'],
	},
})
