/**
 * Pre-dev environment check.
 *
 * Validates that required env vars are present in packages/app-lib/.env
 * before starting the dev server. A missing VITE_CONVEX_URL causes the app
 * window to stay invisible (Vue setup crashes before onMounted fires).
 *
 * Exit codes: 0 = ok, 1 = missing vars or missing .env file.
 */

import { existsSync, readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')
const envPath = resolve(root, 'packages/app-lib/.env')

if (!existsSync(envPath)) {
	console.error('[pre-dev] ERROR: packages/app-lib/.env not found')
	console.error('         Copy packages/app-lib/.env.example to .env and fill in the values.')
	process.exit(1)
}

const content = readFileSync(envPath, 'utf8')

const required = [
	{
		key: 'VITE_CONVEX_URL',
		hint: 'Set the Convex deployment URL for the active desktop app profile.',
	},
]

const missing = required.filter(({ key }) => !new RegExp(`^${key}=.+`, 'm').test(content))

if (missing.length > 0) {
	console.error('[pre-dev] ERROR: Missing required env vars in packages/app-lib/.env:\n')
	for (const { key, hint } of missing) {
		console.error(`  ${key}`)
		console.error(`    ${hint}\n`)
	}
	console.error('  The app window will stay invisible without these variables.')
	process.exit(1)
}

console.log('[pre-dev] packages/app-lib/.env looks good')
