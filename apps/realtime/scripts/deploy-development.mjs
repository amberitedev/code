import { randomBytes } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { spawn } from 'node:child_process'

const workspaceRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../../..')
const envPath = resolve(workspaceRoot, '.env.local')
const pnpm = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'

const envFile = await readFile(envPath, 'utf8')
const localEnv = parseEnv(envFile)
const convexUrl = required(localEnv, 'CONVEX_URL')
const convexSiteUrl = required(localEnv, 'CONVEX_SITE_URL')
const hmacSecret = localEnv.REALTIME_BRIDGE_HMAC_SECRET || randomBytes(32).toString('base64url')

if (!localEnv.REALTIME_BRIDGE_HMAC_SECRET)
	await updateEnv(envPath, envFile, 'REALTIME_BRIDGE_HMAC_SECRET', hmacSecret)

await run(pnpm, ['exec', 'convex', 'env', 'set', 'REALTIME_BRIDGE_HMAC_SECRET', hmacSecret])

const deployment = await run(pnpm, [
	'--dir',
	'apps/realtime',
	'exec',
	'wrangler',
	'deploy',
	'--env',
	'development',
])
const workerUrl = deployment.match(/https:\/\/[^\s]+\.workers\.dev/)?.[0]

if (!workerUrl) throw new Error('Wrangler did not report a development Worker URL.')

const secrets = {
	CONVEX_JWKS_URL: `${convexUrl.replace(/\/$/, '')}/.well-known/jwks.json`,
	CONVEX_JWT_ISSUER: convexSiteUrl.replace(/\/$/, ''),
	CONVEX_JWT_AUDIENCE: 'convex',
	CONVEX_BRIDGE_URL: `${convexSiteUrl.replace(/\/$/, '')}/realtime/bridge`,
	REALTIME_BRIDGE_HMAC_SECRET: hmacSecret,
	DESKTOP_ORIGINS: 'tauri://localhost',
}

await run(
	pnpm,
	['--dir', 'apps/realtime', 'exec', 'wrangler', 'secret', 'bulk', '--env', 'development'],
	JSON.stringify(secrets),
)
await updateEnv(envPath, await readFile(envPath, 'utf8'), 'VITE_REALTIME_URL', workerUrl)

console.log(`Development realtime Worker ready at ${workerUrl}`)

function parseEnv(content) {
	return Object.fromEntries(
		content
			.split(/\r?\n/)
			.map((line) => line.match(/^([^#=\s]+)=(.*)$/))
			.filter(Boolean)
			.map(([, key, value]) => [key, value]),
	)
}

function required(environment, key) {
	const value = environment[key]?.trim()
	if (!value) throw new Error(`${key} must be configured in .env.local.`)
	return value
}

async function updateEnv(path, content, key, value) {
	const expression = new RegExp(`^${key}=.*$`, 'm')
	const next = expression.test(content)
		? content.replace(expression, `${key}=${value}`)
		: `${content.replace(/\s*$/, '\n')}${key}=${value}\n`
	await writeFile(path, next)
}

function run(command, args, input) {
	return new Promise((resolvePromise, reject) => {
		if (process.platform === 'win32' && args.some((value) => /[\s&|<>^]/.test(value)))
			return reject(new Error('The Windows deploy command received an unsafe argument.'))
		const child = spawn(
			process.platform === 'win32' ? process.env.ComSpec || 'cmd.exe' : command,
			process.platform === 'win32' ? ['/d', '/c', [command, ...args].join(' ')] : args,
			{
			cwd: workspaceRoot,
			stdio: ['pipe', 'pipe', 'pipe'],
			},
		)
		let output = ''
		child.stdout.on('data', (chunk) => {
			const text = chunk.toString()
			output += text
			process.stdout.write(text)
		})
		child.stderr.on('data', (chunk) => {
			const text = chunk.toString()
			output += text
			process.stderr.write(text)
		})
		child.on('error', reject)
		child.on('close', (code) => {
			if (code === 0) resolvePromise(output)
			else reject(new Error(`${command} ${args.join(' ')} exited with ${code}.`))
		})
		child.stdin.end(input)
	})
}
