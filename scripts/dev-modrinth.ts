import { execFileSync, spawn } from 'node:child_process'
import { existsSync, realpathSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const args = process.argv.slice(2)

if (args.includes('--help')) {
	console.log(`Usage: vp run dev:modrinth [owner|recipient] [--dry-run]

Launch the original Modrinth App against local fake Hosting and Sharing services.
Requires the lab/modrinth-backend-proxy worktree and its dependencies.
State persists in that worktree's modrinthclonedata directory, never Amberite's .data.
Set MODRINTH_LAB_DIR to use another prepared upstream proxy-lab checkout.
This command does not reset data, switch branches, or update upstream.`)
} else {
	const worktrees = execFileSync('git', ['worktree', 'list', '--porcelain'], {
		cwd: root,
		encoding: 'utf8',
	})
	const labRecord = worktrees
		.split(/\r?\n\r?\n/)
		.find((record) =>
			record.split(/\r?\n/).includes('branch refs/heads/lab/modrinth-backend-proxy'),
		)
	const discovered = labRecord
		?.split(/\r?\n/)
		.find((line) => line.startsWith('worktree '))
		?.slice(9)
	const candidate = process.env.MODRINTH_LAB_DIR || discovered
	if (!candidate)
		throw new Error('No proxy-lab worktree found. Set MODRINTH_LAB_DIR to its checkout.')
	const labRoot = realpathSync(candidate)
	if (labRoot === realpathSync(root))
		throw new Error('The mock must run in its own upstream checkout.')
	const entry = resolve(labRoot, 'packages/proxy-lab/src/cli.ts')
	if (!existsSync(entry)) throw new Error(`Proxy-lab launcher is missing: ${entry}`)
	console.log(`[dev:modrinth] Upstream checkout: ${labRoot}`)
	console.log(`[dev:modrinth] Persistent isolated data: ${resolve(labRoot, 'modrinthclonedata')}`)
	const child = spawn(process.execPath, [entry, 'dev', ...args], {
		cwd: labRoot,
		stdio: 'inherit',
		windowsHide: true,
	})
	child.on('error', (error) => {
		console.error(error.message)
		process.exitCode = 1
	})
	child.on('exit', (code) => {
		process.exitCode = code ?? 1
	})
	// The lab runner owns cleanup of the backend and native app it starts.
	process.once('SIGINT', () => child.kill('SIGINT'))
	process.once('SIGTERM', () => child.kill('SIGTERM'))
}
