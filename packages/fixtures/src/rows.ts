// Rows read and written directly, for the states a program never reaches by itself.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { dev } from '@amberite/contracts'

import { devHeaders, send } from './call.ts'
import { type App, type Core, FixtureError, runtime, state } from './context.ts'

/** The backend's database, or the two an App or a Core keeps in its data folder. */
export type Store = 'backend' | App | Core
export type Value = string | number | null
export type Row = Record<string, Value>

/**
 * Runs one statement and returns the rows it read. On an App or a Core it runs in whichever of
 * the program's databases has the statement's tables.
 */
export async function sql(
	store: Store,
	statement: string,
	values: ReadonlyArray<Value> = [],
): Promise<Row[]> {
	if (store === 'backend') return (await backend(statement, values)).rows
	// `all` also runs a statement that returns no rows, so nothing here guesses which kind it is.
	return local(store, (db) =>
		db
			.prepare(statement)
			.all(...values)
			.map(plain),
	)
}

/**
 * Changes columns of rows a program already wrote: `where` picks them, `values` is what they
 * become. A column the table lacks fails by name, and so does a `where` that matches nothing.
 *
 * Patch the program that owns the data. Much of what the backend knows about a Core is a copy the
 * Core sends again whenever it starts.
 *
 * A Core or an App is restarted before it is next called, because it may hold what it read
 * earlier in memory. State that lives only in memory, such as a Core's download links, cannot be
 * patched at all.
 */
export async function patch(store: Store, table: string, where: Row, values: Row): Promise<void> {
	const set = Object.keys(values)
	const match = Object.keys(where)
	if (set.length === 0 || match.length === 0) {
		throw new FixtureError(`patch of ${table} needs columns to match and columns to set.`)
	}
	const matching = match.map((name) => `"${name}" IS ?`).join(' AND ')
	if (store === 'backend') {
		const statement = `UPDATE "${table}" SET ${set.map((name) => `"${name}" = ?`).join(', ')} WHERE ${matching}`
		const { changes } = await backend(statement, [
			...Object.values(values),
			...Object.values(where),
		])
		if (changes === 0) throw noRow('backend', table, where)
		return
	}
	local(store, (db) => {
		const columns = new Map(
			db
				.prepare(`PRAGMA table_info("${table}")`)
				.all()
				.map((column) => [String(column.name), String(column.type)]),
		)
		if (columns.size === 0) throw new Error(`no such table: ${table}`)
		for (const name of [...set, ...match]) {
			if (!columns.has(name)) throw new FixtureError(`${table} has no column ${name}.`)
		}
		// These columns hold SQLite's binary JSON, which reads back wrong when written as text.
		const mark = (name: string) => (columns.get(name)!.toUpperCase() === 'JSONB' ? 'jsonb(?)' : '?')
		const statement = `UPDATE "${table}" SET ${set.map((name) => `"${name}" = ${mark(name)}`).join(', ')} WHERE ${matching}`
		const { changes } = db.prepare(statement).run(...Object.values(values), ...Object.values(where))
		if (changes === 0) throw noRow(label(store), table, where)
	})
}

function noRow(where: string, table: string, match: Row): FixtureError {
	return new FixtureError(`${table} on ${where} has no row with ${JSON.stringify(match)}.`)
}

async function backend(
	statement: string,
	values: ReadonlyArray<Value>,
): Promise<{ readonly rows: Row[]; readonly changes: number }> {
	// Through the backend itself: its database file belongs to the process serving it.
	const answer = await send(`${runtime().urls.backend}${dev.Sql.path}`, {
		method: dev.Sql.method,
		headers: devHeaders(),
		body: JSON.stringify({ sql: statement, params: values }),
	})
	const result = JSON.parse(answer.text) as { rows: Row[]; changes: number; description?: string }
	if (answer.status >= 400) {
		throw new FixtureError(`The backend's database refused \`${statement}\`: ${result.description}`)
	}
	return result
}

function label(store: App | Core): string {
	return `${store.kind}:${store.index}`
}

/** Runs `work` in the first of the program's databases that knows the tables it names. */
function local<T>(store: App | Core, work: (db: DatabaseSync) => T): T {
	const files =
		store.kind === 'app'
			? ['app.db', 'engine.db']
			: [NodePath.join('db', 'core.db'), NodePath.join('engine', 'engine.db')]
	let missing: unknown
	for (const file of files) {
		const path = NodePath.join(store.dir, file)
		if (!NodeFS.existsSync(path)) continue
		const db = new DatabaseSync(path)
		try {
			// The program has the file open too.
			db.exec('PRAGMA busy_timeout = 5000')
			const result = work(db)
			// Whatever the statement looked like, this is whether it changed a row.
			const { changed } = db.prepare('SELECT total_changes() AS changed').get()!
			if (Number(changed) > 0) state.dirty.add(label(store))
			return result
		} catch (error) {
			if (!(error instanceof Error) || !error.message.includes('no such table')) throw error
			missing = error
		} finally {
			db.close()
		}
	}
	throw new FixtureError(
		`${label(store)} has no such table in ${files.join(' or ')}${missing instanceof Error ? ` (${missing.message})` : ''}.`,
	)
}

function plain(row: Record<string, unknown>): Row {
	return Object.fromEntries(
		Object.entries(row).map(([key, value]) => [
			key,
			typeof value === 'bigint'
				? Number(value)
				: value instanceof Uint8Array
					? `<${value.length} bytes>`
					: (value as Value),
		]),
	)
}
