// Lets a fixture anywhere on disk, and the kit itself, import the kit and the generated contracts
// by package name under plain Node. Imports nothing from either, so it can load before them.

import * as NodeFS from 'node:fs'
import { registerHooks } from 'node:module'
import * as NodePath from 'node:path'

const CONTRACTS = new URL('../../contracts/ts/', import.meta.url)

/** Package names to files, as tsconfig `paths` take them: a `*` stands for the rest of the name. */
export const PACKAGES = {
	'@amberite/fixtures': new URL('./index.ts', import.meta.url).href,
	'@amberite/contracts': new URL('index.ts', CONTRACTS).href,
	// One generated file, such as `@amberite/contracts/core`.
	'@amberite/contracts/*': new URL('*.ts', CONTRACTS).href,
}

/** The file `specifier` names in `PACKAGES`, if it names one. */
function packageFile(specifier: string): string | undefined {
	for (const [name, url] of Object.entries(PACKAGES)) {
		if (name === specifier) return url
		const prefix = name.endsWith('/*') ? name.slice(0, -1) : undefined
		if (prefix && specifier.startsWith(prefix)) {
			return url.replace('*', specifier.slice(prefix.length))
		}
	}
}

let registered = false

export function resolvePackages(): void {
	if (registered) return
	registered = true
	registerHooks({
		resolve(specifier, context, next) {
			const known = packageFile(specifier)
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
