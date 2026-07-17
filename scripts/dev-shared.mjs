import { createHash, randomBytes } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

export function commandForPnpm(args) {
	if (process.platform === 'win32') {
		return {
			command: process.execPath,
			args: [
				join(dirname(process.execPath), 'node_modules', 'corepack', 'dist', 'corepack.js'),
				'pnpm',
				...args,
			],
		}
	}
	return { command: 'corepack', args: ['pnpm', ...args] }
}

export function gitValue(cwd, args) {
	const safeArgs =
		process.platform === 'win32' ? ['-c', `safe.directory=${resolve(cwd)}`, ...args] : args
	const result = spawnSync('git', safeArgs, { cwd, encoding: 'utf8', windowsHide: true })
	if (result.status !== 0) {
		throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`)
	}
	return result.stdout.trim()
}

export function findWorktreeRoot(cwd = process.cwd()) {
	return resolve(gitValue(cwd, ['rev-parse', '--show-toplevel']))
}

export function repositoryKey(cwd = process.cwd()) {
	const root = findWorktreeRoot(cwd)
	const commonDir = resolve(root, gitValue(root, ['rev-parse', '--git-common-dir']))
	return `${basename(dirname(commonDir))}-${createHash('sha256').update(commonDir).digest('hex').slice(0, 12)}`
}

export function globalDevRoot(cwd = process.cwd()) {
	const parent =
		process.platform === 'win32'
			? process.env.LOCALAPPDATA || join(homedir(), 'AppData', 'Local')
			: process.env.XDG_STATE_HOME || join(homedir(), '.local', 'state')
	return join(parent || tmpdir(), 'amberite-dev', repositoryKey(cwd))
}

export function worktreeKey(cwd = process.cwd()) {
	const root = findWorktreeRoot(cwd)
	return createHash('sha256').update(root).digest('hex').slice(0, 12)
}

export function primaryWorktreeRoot(cwd = process.cwd()) {
	const worktrees = gitValue(cwd, ['worktree', 'list', '--porcelain'])
	const first = worktrees.split(/\r?\n/).find((line) => line.startsWith('worktree '))
	if (!first) throw new Error('Git did not report a primary worktree.')
	return resolve(first.slice('worktree '.length))
}

export function readEnvFile(path) {
	if (!existsSync(path)) return {}
	const values = {}
	for (const line of readFileSync(path, 'utf8').split(/\r?\n/)) {
		const match = line.match(/^([A-Z0-9_]+)=(.*)$/)
		if (match) values[match[1]] = match[2].trim()
	}
	return values
}

export function validateDevUsername(username) {
	if (!/^[a-zA-Z0-9_]{3,24}$/.test(username)) {
		throw new Error('Amberite usernames must use 3-24 letters, numbers, or underscores.')
	}
	return username
}

export function makeShortId(existing = new Set()) {
	for (;;) {
		const id = randomBytes(4).toString('hex').slice(0, 6)
		if (!existing.has(id)) return id
	}
}

export function isProcessRunning(pid) {
	if (!Number.isInteger(pid) || pid <= 0) return false
	try {
		process.kill(pid, 0)
		return true
	} catch {
		return false
	}
}

export function killProcessTree(pid) {
	if (!isProcessRunning(pid)) return
	if (process.platform === 'win32') {
		spawnSync('taskkill.exe', ['/PID', String(pid), '/T', '/F'], { windowsHide: true })
		return
	}
	try {
		process.kill(-pid, 'SIGTERM')
	} catch {
		try {
			process.kill(pid, 'SIGTERM')
		} catch {}
	}
}
