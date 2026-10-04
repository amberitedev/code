import { spawnSync } from 'node:child_process'
import { existsSync, realpathSync } from 'node:fs'
import path from 'node:path'
import { DatabaseSync } from 'node:sqlite'

function normalized(value: string): string {
	return value.replaceAll('\\', '/').replace(/\/+/g, '/').replace(/\/$/, '').toLowerCase()
}

export function runningAppCommands(): string[] {
	const result =
		process.platform === 'win32'
			? spawnSync(
					'powershell.exe',
					[
						'-NoProfile',
						'-Command',
						"Get-CimInstance Win32_Process -Filter \"Name='theseus_gui.exe' OR Name='Amberite.exe'\" | Select-Object -ExpandProperty CommandLine",
					],
					{ encoding: 'utf8', windowsHide: true },
				)
			: spawnSync('ps', ['-eo', 'args='], { encoding: 'utf8' })
	if (result.status !== 0) throw new Error('Cannot verify that scenario Apps are stopped.')
	return result.stdout.split('\n').filter((line) => /theseus_gui|amberite/i.test(line))
}

/** A copied launcher database must never send instance writes or directory moves to its source. */
export function normalizeScenarioDatabase(
	worktree: string,
	dataDir: string,
	appCommands: readonly string[],
): void {
	if (
		appCommands.some((command) =>
			normalized(command)
				.split(normalized(dataDir))
				.slice(1)
				.some((suffix) => suffix === '' || /^[\s/"']/.test(suffix)),
		)
	) {
		throw new Error(`Stop the App using ${dataDir} before preparing its scenario database.`)
	}
	const databasePath = path.join(dataDir, 'app.db')
	if (!existsSync(databasePath)) return
	const relative = path.relative(realpathSync(worktree), realpathSync(databasePath))
	if (relative.startsWith('..') || path.isAbsolute(relative))
		throw new Error('Scenario database must stay inside the current worktree.')
	const db = new DatabaseSync(databasePath)
	try {
		const tables = new Set(
			db
				.prepare("SELECT name FROM sqlite_master WHERE type='table'")
				.all()
				.map((row) => row.name),
		)
		if (!tables.has('settings')) return
		const settings = db.prepare('SELECT custom_dir,prev_custom_dir FROM settings WHERE id=0').get()
		if (!settings) return
		if (tables.has('instances')) {
			for (const row of db.prepare('SELECT path FROM instances').all()) {
				if (
					typeof row.path !== 'string' ||
					path.isAbsolute(row.path) ||
					path.win32.isAbsolute(row.path) ||
					row.path.split(/[\\/]+/).includes('..')
				) {
					throw new Error(
						'Scenario instance paths must be relative to their own launcher directory.',
					)
				}
			}
		}
		const roots = [settings.custom_dir, settings.prev_custom_dir].filter(
			(root): root is string =>
				typeof root === 'string' && normalized(root) !== normalized(dataDir),
		)
		const rebase = (value: string): string => {
			const candidate = value.replaceAll('\\', '/').replace(/\/+/g, '/')
			for (const root of roots) {
				const prefix = normalized(root)
				if (normalized(value) === prefix) return dataDir
				if (normalized(value).startsWith(`${prefix}/`))
					return path.join(dataDir, candidate.slice(prefix.length + 1))
			}
			return value
		}
		const rebaseJson = (value: unknown): unknown => {
			if (typeof value === 'string') return rebase(value)
			if (Array.isArray(value)) return value.map(rebaseJson)
			if (value && typeof value === 'object')
				return Object.fromEntries(
					Object.entries(value).map(([key, item]) => [key, rebaseJson(item)]),
				)
			return value
		}
		db.exec('BEGIN IMMEDIATE')
		try {
			db.prepare('UPDATE settings SET custom_dir=?,prev_custom_dir=? WHERE id=0').run(
				dataDir,
				dataDir,
			)
			for (const [table, column] of [
				['instances', 'icon_path'],
				['java_versions', 'path'],
			] as const) {
				if (!tables.has(table)) continue
				for (const row of db
					.prepare(`SELECT rowid AS row_id,${column} AS value FROM ${table}`)
					.all()) {
					if (typeof row.value === 'string')
						db.prepare(`UPDATE ${table} SET ${column}=? WHERE rowid=?`).run(
							rebase(row.value),
							row.row_id,
						)
				}
			}
			for (const [table, column, binary] of [
				['instance_launch_overrides', 'overrides', true],
				['install_jobs', 'state', true],
				['self_hosted_uploads', 'manifest', false],
			] as const) {
				if (!tables.has(table)) continue
				for (const row of db
					.prepare(`SELECT rowid AS row_id,json(${column}) AS value FROM ${table}`)
					.all()) {
					if (typeof row.value !== 'string') continue
					const value = JSON.stringify(rebaseJson(JSON.parse(row.value)))
					db.prepare(
						`UPDATE ${table} SET ${column}=${binary ? 'jsonb(?)' : '?'} WHERE rowid=?`,
					).run(value, row.row_id)
				}
			}
			if (roots.length && tables.has('processes')) db.exec('DELETE FROM processes')
			db.exec('COMMIT')
		} catch (error) {
			db.exec('ROLLBACK')
			throw error
		}
	} finally {
		db.close()
	}
}
