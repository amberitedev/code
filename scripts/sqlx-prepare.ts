#!/usr/bin/env node

// `vp run sqlx:prepare`: writes packages/engine/.sqlx and apps/app/.sqlx again, after a query or a
// migration changed. Each crate's queries are checked against a database made from its own
// migrations. Uses this checkout's `target`, so run no other Cargo command here meanwhile.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import * as NodeURL from 'node:url'

const root = NodePath.resolve(NodeURL.fileURLToPath(new URL('..', import.meta.url)))
type Crate = {
	readonly name: string
	readonly dir: string
	/** Touched so the query macros run again. */
	readonly entry: string
	readonly features: ReadonlyArray<string>
}
const engine: Crate = {
	name: 'engine',
	dir: 'packages/engine',
	entry: 'src/lib.rs',
	features: ['export-ts'],
}
const app: Crate = { name: 'amberite-app', dir: 'apps/app', entry: 'src/main.rs', features: [] }
const databases = NodePath.join(root, 'target', 'sqlx')
NodeFS.mkdirSync(databases, { recursive: true })

/** `cargo check` of one crate. `database` is where its queries are checked, none for offline. */
function check(crate: Crate, database?: string): boolean {
	const online = database && {
		DATABASE_URL: `sqlite:${database.replaceAll('\\', '/')}`,
		SQLX_OFFLINE: 'false',
		SQLX_OFFLINE_DIR: NodePath.join(root, crate.dir, '.sqlx'),
	}
	const features = crate.features.flatMap((feature) => ['--features', feature])
	const run = NodeChildProcess.spawnSync(
		'cargo',
		['check', '-p', crate.name, '--all-targets', ...features],
		{
			cwd: root,
			env: { ...process.env, ...(online || { SQLX_OFFLINE: 'true' }) },
			stdio: 'inherit',
		},
	)
	return run.status === 0
}

for (const crate of [engine, app]) {
	const database = NodePath.join(databases, `${crate.name}.db`)
	NodeFS.rmSync(database, { force: true })
	const db = new DatabaseSync(database)
	const migrations = NodePath.join(root, crate.dir, 'migrations')
	for (const file of NodeFS.readdirSync(migrations).sort()) {
		db.exec(NodeFS.readFileSync(NodePath.join(migrations, file), 'utf8'))
	}
	db.close()
	const offline = NodePath.join(root, crate.dir, '.sqlx')
	for (const file of NodeFS.readdirSync(offline)) {
		if (/^query-.*\.json$/.test(file)) NodeFS.rmSync(NodePath.join(offline, file))
	}
	const now = new Date()
	NodeFS.utimesSync(NodePath.join(root, crate.dir, crate.entry), now, now)
}

if (!check(engine, NodePath.join(databases, 'engine.db'))) process.exit(1)
// Compiles the engine as the App uses it from the files just written, so the App's check below
// compiles only the App against its own database. The App's own queries fail here.
check(app)
if (!check(app, NodePath.join(databases, 'amberite-app.db'))) process.exit(1)
