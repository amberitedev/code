import { createHash } from 'node:crypto'
import { readdir, readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const wranglerRequire = createRequire(require.resolve('wrangler'))
const { Miniflare } = wranglerRequire('miniflare')
const { build } = wranglerRequire('esbuild')

/** Runs the real Worker in wrangler's local runtime with a fresh, fully migrated D1 and an empty R2. */
export async function startBackend(name) {
	const bundle = await build({
		entryPoints: [fileURLToPath(new URL('../src/index.ts', import.meta.url))],
		bundle: true,
		write: false,
		format: 'esm',
		platform: 'browser',
		target: 'es2022',
		external: ['cloudflare:workers'],
	})
	const runtime = new Miniflare({
		modules: true,
		script: bundle.outputFiles[0].text,
		compatibilityDate: '2026-07-22',
		compatibilityFlags: ['nodejs_compat'],
		d1Databases: { DB: name },
		r2Buckets: { FILES: name },
		durableObjects: { FRIENDS: { className: 'FriendsHub', useSQLite: true } },
		bindings: { LOCAL_DEV: 'true' },
	})
	const db = await runtime.getD1Database('DB')
	const migrations = new URL('../migrations/', import.meta.url)
	for (const file of (await readdir(migrations)).sort()) {
		const sql = (await readFile(new URL(file, migrations), 'utf8')).replace(/^--.*$/gm, '')
		for (const statement of sql
			.split(';')
			.map((value) => value.trim())
			.filter(Boolean))
			await db.prepare(statement).run()
	}
	/** Opens a session for `user`; requests authenticate with `Bearer mra_<user>`. */
	async function session(user) {
		await db
			.prepare(
				'INSERT INTO sessions (id,token_hash,user_id,created,last_login,expires,refresh_expires,user_agent,ip) VALUES (?,?,?,?,?,?,?,?,?)',
			)
			.bind(
				user,
				createHash('sha256').update(`mra_${user}`).digest('hex'),
				user,
				'2026-01-01',
				'2026-01-01',
				'2099-01-01',
				'2099-01-01',
				'test',
				'127.0.0.1',
			)
			.run()
	}
	return {
		db,
		files: await runtime.getR2Bucket('FILES'),
		session,
		async account(user) {
			await db
				.prepare('INSERT INTO users (id,username,email,created) VALUES (?,?,?,?)')
				.bind(user, user, `${user}@local.invalid`, new Date().toISOString())
				.run()
			await session(user)
		},
		fetch: (url, init) => runtime.dispatchFetch(url, init),
		dispose: () => runtime.dispose(),
	}
}
