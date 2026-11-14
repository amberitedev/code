import { copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { expect, it } from 'vite-plus/test'
import { normalizeScenarioDatabase } from './scenario-state.ts'

it('isolates a copied scenario database without modifying its primary source or active App state', () => {
	const fixture = mkdtempSync(path.join(tmpdir(), 'scenario-state-'))
	const source = path.join(fixture, 'primary', '.data', 'scenarios', '1')
	const worktree = path.join(fixture, 'worktree')
	const target = path.join(worktree, '.data', 'scenarios', '1')
	mkdirSync(source, { recursive: true })
	mkdirSync(target, { recursive: true })
	try {
		const originalApp = new DatabaseSync(path.join(source, 'app.db'))
		originalApp.exec(
			'CREATE TABLE settings(id INTEGER PRIMARY KEY,custom_dir TEXT,prev_custom_dir TEXT)',
		)
		originalApp.prepare('INSERT INTO settings VALUES(0,?,?)').run(source, source)
		originalApp.close()
		const original = new DatabaseSync(path.join(source, 'engine.db'))
		original.exec(`
			CREATE TABLE instances(id TEXT,path TEXT,icon_path TEXT);
			CREATE TABLE java_versions(path TEXT);
			CREATE TABLE instance_launch_overrides(overrides BLOB);
			CREATE TABLE processes(pid INTEGER);
		`)
		original
			.prepare('INSERT INTO instances VALUES(?,?,?)')
			.run('test', 'My instance', path.join(source, 'caches', 'icon.png'))
		original
			.prepare('INSERT INTO java_versions VALUES(?)')
			.run(path.join(source, 'meta', 'java.exe'))
		original.prepare('INSERT INTO instance_launch_overrides VALUES(jsonb(?))').run(
			JSON.stringify({
				java: path.join(source, 'meta', 'java.exe'),
				unrelated: 'https://modrinth.com',
			}),
		)
		original.exec('INSERT INTO processes VALUES(123)')
		original.close()
		for (const file of ['app.db', 'engine.db'])
			copyFileSync(path.join(source, file), path.join(target, file))
		expect(() =>
			normalizeScenarioDatabase(worktree, target, [
				`theseus_gui --amberite-dev-config ${JSON.stringify({ dataDir: target })}`,
			]),
		).toThrow('Stop the App')
		expect(() => normalizeScenarioDatabase(worktree, source, [])).toThrow(
			'inside the current worktree',
		)
		normalizeScenarioDatabase(worktree, target, [
			`theseus_gui --amberite-dev-config ${JSON.stringify({ dataDir: `${target}0` })}`,
		])
		const copiedApp = new DatabaseSync(path.join(target, 'app.db'), { readOnly: true })
		try {
			expect(copiedApp.prepare('SELECT custom_dir,prev_custom_dir FROM settings').get()).toEqual({
				custom_dir: target,
				prev_custom_dir: target,
			})
		} finally {
			copiedApp.close()
		}
		const copied = new DatabaseSync(path.join(target, 'engine.db'), { readOnly: true })
		try {
			expect(copied.prepare('SELECT path,icon_path FROM instances').get()).toEqual({
				path: 'My instance',
				icon_path: path.join(target, 'caches', 'icon.png'),
			})
			expect(copied.prepare('SELECT path FROM java_versions').get()?.path).toBe(
				path.join(target, 'meta', 'java.exe'),
			)
			expect(
				copied.prepare('SELECT json(overrides) AS value FROM instance_launch_overrides').get()
					?.value,
			).toBe(
				JSON.stringify({
					java: path.join(target, 'meta', 'java.exe'),
					unrelated: 'https://modrinth.com',
				}),
			)
			expect(copied.prepare('SELECT COUNT(*) AS count FROM processes').get()?.count).toBe(0)
		} finally {
			copied.close()
		}
		const unchanged = new DatabaseSync(path.join(source, 'app.db'), { readOnly: true })
		try {
			expect(unchanged.prepare('SELECT custom_dir FROM settings').get()?.custom_dir).toBe(source)
		} finally {
			unchanged.close()
		}
	} finally {
		if (
			path.dirname(fixture) !== path.resolve(tmpdir()) ||
			!path.basename(fixture).startsWith('scenario-state-')
		)
			throw new Error('Unexpected fixture directory')
		rmSync(fixture, { recursive: true, force: true })
	}
})
