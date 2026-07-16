#!/usr/bin/env node
import { copyFileSync, existsSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'

import { findWorktreeRoot, primaryWorktreeRoot } from './dev-shared.mjs'

const worktree = findWorktreeRoot()
const primary = primaryWorktreeRoot(worktree)
const developmentFiles = [
	'.env.local',
	'packages/app-lib/.env',
	'apps/frontend/.env.local',
	'apps/labrinth/.env.local',
	'apps/daedalus_client/.env.local',
]

if (primary !== worktree) {
	for (const relativePath of developmentFiles) {
		const source = join(primary, relativePath)
		const destination = join(worktree, relativePath)
		if (!existsSync(source) || existsSync(destination)) continue
		mkdirSync(dirname(destination), { recursive: true })
		copyFileSync(source, destination)
		console.log(`Created ${relativePath} from the primary worktree.`)
	}
}

const appEnv = join(worktree, 'packages', 'app-lib', '.env')
const appEnvTemplate = join(worktree, 'packages', 'app-lib', '.env.local')
if (!existsSync(appEnv) && existsSync(appEnvTemplate)) {
	copyFileSync(appEnvTemplate, appEnv)
	console.log('Created packages/app-lib/.env from its tracked local template.')
}

if (!existsSync(join(worktree, '.env.local'))) {
	console.warn('Convex cloud development is not configured. Run pnpm exec convex dev --configure existing.')
}

console.log('Amberite worktree setup is ready.')
