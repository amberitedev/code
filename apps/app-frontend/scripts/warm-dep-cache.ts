/**
 * Vite's dependency cache includes the absolute project root, so each fresh
 * worktree needs to warm its own cache before the first App launch.
 */
import * as NodePath from 'node:path'
import * as NodeURL from 'node:url'

import { optimizeDeps, resolveConfig } from 'vite'

const appRoot = NodePath.dirname(NodePath.dirname(NodeURL.fileURLToPath(import.meta.url)))
const config = await resolveConfig({ root: appRoot, logLevel: 'error' }, 'serve')

await optimizeDeps(config)
console.log('[warm-dep-cache] App dependency cache is warm')
