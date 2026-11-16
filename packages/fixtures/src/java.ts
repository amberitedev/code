// An App with a new database knows of no Java, and downloads and unpacks one again on its first
// install, even with that Java among its kept game files. So an apply has each App use the Java
// installs its `meta` folder holds, set through the App's own Java settings.
//
// A Core has no such setting, so a server's first install still downloads Java.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { java, type JavaVersion } from '@amberite/contracts'

import { call } from './call.ts'
import type { App } from './context.ts'

const folder = (app: App) => NodePath.join(app.dir, 'meta', 'java_versions')
/** What the App said its Java installs were when the last fixture finished. */
const record = (app: App) => NodePath.join(app.dir, 'meta', '.java.json')

function kept(app: App, install: JavaVersion): boolean {
	const inside = NodePath.resolve(install.path)
		.toLowerCase()
		.startsWith(NodePath.resolve(folder(app)).toLowerCase())
	return inside && NodeFS.existsSync(install.path)
}

/** Points a newly started App at the Java installs it kept. */
export async function useKeptJava(app: App): Promise<void> {
	if (!NodeFS.existsSync(folder(app))) return
	// Without a record the App searches the whole computer for Java, which takes seconds.
	const known: ReadonlyArray<JavaVersion> = NodeFS.existsSync(record(app))
		? JSON.parse(NodeFS.readFileSync(record(app), 'utf8'))
		: await call(app, java.FindJavaInstalls)
	const majors = new Set<number>()
	for (const install of known) {
		if (!kept(app, install) || majors.has(install.parsed_version)) continue
		majors.add(install.parsed_version)
		await call(app, java.SetJavaVersion, null, install)
	}
}

/** Notes which Java installs the App ended up with, for the next apply. */
export async function rememberJava(app: App): Promise<void> {
	const installs = Object.values(await call(app, java.GetJavaVersions)).filter((install) =>
		kept(app, install),
	)
	if (installs.length > 0) NodeFS.writeFileSync(record(app), JSON.stringify(installs))
}
