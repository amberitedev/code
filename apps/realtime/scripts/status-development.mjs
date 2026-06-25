import { createHmac } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const workspaceRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../../..')
const localEnv = parseEnv(await readFile(resolve(workspaceRoot, '.env.local'), 'utf8'))
const endpoint = required(localEnv, 'VITE_REALTIME_URL').replace(/\/$/, '')
const secret = required(localEnv, 'REALTIME_BRIDGE_HMAC_SECRET')
const body = '{}'
const timestamp = String(Date.now())
const signature = createHmac('sha256', secret).update(`${timestamp}.${body}`).digest('base64url')
const response = await fetch(`${endpoint}/v1/debug-presence`, {
	method: 'POST',
	headers: {
		'content-type': 'application/json',
		'x-amberite-timestamp': timestamp,
		'x-amberite-signature': signature,
	},
	body,
})

if (!response.ok) throw new Error(`Realtime status request failed (${response.status}).`)

const status = await response.json()
console.log(JSON.stringify(status, null, 2))

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
