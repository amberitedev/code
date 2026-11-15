// Lets a fixture anywhere on disk, and the kit itself, import the kit and the generated contracts
// by package name under plain Node. Imports nothing from either, so it can load before them.

import * as NodeFS from 'node:fs'
import { registerHooks } from 'node:module'
import * as NodePath from 'node:path'

export const PACKAGES = {
	'@amberite/fixtures': new URL('./index.ts', import.meta.url).href,
	'@amberite/contracts': new URL('../../contracts/ts/index.ts', import.meta.url).href,
}

let registered = false

export function resolvePackages(): void {
	if (registered) return
	registered = true
	registerHooks({
		resolve(specifier, context, next) {
			const known = PACKAGES[specifier as keyof typeof PACKAGES]
			if (known) return { url: known, shortCircuit: true }
			// The generated contracts import each other without a file extension.
			if (
				specifier.startsWith('.') &&
				context.parentURL?.startsWith('file:') &&
				!NodePath.extname(specifier)
			) {
				const file = new URL(`${specifier}.ts`, context.parentURL)
				if (NodeFS.existsSync(file)) return { url: file.href, shortCircuit: true }
			}
			return next(specifier, context)
		},
		load(url, context, next) {
			// A fixture outside the repo has no package.json to say it is an ES module.
			return next(url, url.endsWith('.ts') ? { ...context, format: 'module-typescript' } : context)
		},
	})
}
