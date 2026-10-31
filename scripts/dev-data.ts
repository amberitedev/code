// Development data: accounts and sharing (`backend`), Core (`core`), and one folder per App
// installation (`scenarios/<number>`). They refer to each other, so they are copied together.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

export const DATA_ENTRIES = ['backend', 'core', 'scenarios'] as const
export type DataEntry = (typeof DATA_ENTRIES)[number]

// Inside a scenario: Minecraft's game assets and the App window's browser caches. They are most
// of the files, nothing reads them until a game starts or a page loads, and both are fetched again
// on demand.
const SKIPPED_SCENARIO_PATHS = [
	/^meta\/assets$/,
	/^webview2\/EBWebView\/(?:.+\/)?(?:Cache|Code Cache|GPUCache|GrShaderCache|ShaderCache|Dawn\w+Cache)$/,
]

/**
 * Copies one entry of the data folder `from` over the same entry in `to`. It is copied beside the
 * target and moved into place, so a failed copy never looks like finished data.
 */
export function copyDataEntry(from: string, to: string, entry: DataEntry): void {
	const source = NodePath.join(from, entry)
	const target = NodePath.join(to, entry)
	const partial = `${target}.partial`
	NodeFS.rmSync(partial, { recursive: true, force: true })
	NodeFS.cpSync(source, partial, {
		filter: (path) =>
			entry !== 'scenarios' || !skippedScenarioPath(NodePath.relative(source, path)),
		recursive: true,
	})
	NodeFS.rmSync(target, { recursive: true, force: true })
	NodeFS.renameSync(partial, target)
}

/** Whether `path`, relative to the scenarios folder, is left out of a copy. */
function skippedScenarioPath(path: string): boolean {
	const inScenario = path.split(NodePath.sep).slice(1).join('/')
	return SKIPPED_SCENARIO_PATHS.some((pattern) => pattern.test(inScenario))
}
