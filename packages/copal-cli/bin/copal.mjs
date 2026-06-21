#!/usr/bin/env node

import { existsSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'

const require = createRequire(import.meta.url)

const packages = {
	'linux-x64': '@amberitedev/copal-linux-x64',
	'win32-x64': '@amberitedev/copal-win32-x64',
}

const platform = `${process.platform}-${process.arch}`
const packageName = packages[platform]

if (!packageName) {
	console.error(`Copal does not support ${platform} yet. Supported platform: linux-x64.`)
	process.exit(1)
}

let packageJson
try {
	packageJson = require.resolve(`${packageName}/package.json`)
} catch {
	console.error(
		`Copal's native package (${packageName}) is missing. Reinstall with: npm install -g @amberitedev/copal`,
	)
	process.exit(1)
}

const binary = join(
	dirname(packageJson),
	'bin',
	process.platform === 'win32' ? 'copal.exe' : 'copal',
)
if (!existsSync(binary)) {
	console.error(`Copal binary is missing from ${packageName}. Reinstall @amberitedev/copal.`)
	process.exit(1)
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' })
if (result.error) {
	console.error(`Failed to start Copal: ${result.error.message}`)
	process.exit(1)
}
process.exit(result.status ?? 1)
