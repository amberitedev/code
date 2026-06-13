/**
 * Diagnostic script: compare raw HTTP requests sent by `curl` vs `@amberite/amberite-api`.
 *
 * Run: node packages/core-client/scripts/diagnose.mjs
 *
 * What it does:
 * 1. Spins up a local echo server that returns the exact request it received.
 * 2. Makes the same request via core-client (fetch/XHR) and via curl.
 * 3. Pretty-prints both raw requests side-by-side.
 * 4. Highlights differences (headers, body, method, path).
 *
 * Use this to debug "failed to fetch" in the Tauri desktop app:
 * - If curl works but core-client fails, the diff will show you exactly
 *   what fetch() is doing differently (CORS preflight, missing headers,
 *   different User-Agent, etc.).
 */

import { spawn } from 'child_process'
import http from 'http'
import path from 'path'
import { fileURLToPath } from 'url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

// ── Echo Server ──────────────────────────────────────────────────────────────

/** Start a server that returns the raw request as JSON. */
function startEchoServer(port) {
	return new Promise((resolve) => {
		const server = http.createServer((req, res) => {
			const chunks = []
			req.on('data', (c) => chunks.push(c))
			req.on('end', () => {
				const body = Buffer.concat(chunks).toString('utf-8')
				const result = {
					method: req.method,
					url: req.url,
					headers: req.headers,
					body: body || null,
				}
				res.setHeader('Content-Type', 'application/json')
				res.setHeader('Access-Control-Allow-Origin', '*')
				res.setHeader('Access-Control-Allow-Methods', '*')
				res.setHeader('Access-Control-Allow-Headers', '*')
				res.writeHead(200)
				res.end(JSON.stringify(result, null, 2))
			})
		})
		server.listen(port, () => resolve(server))
	})
}

// ── Core-Client Fetch Test ───────────────────────────────────────────────────

/** Import the core-client api module and make a request. */
async function testViaCoreClient(baseUrl, endpoint, method = 'GET', body = null) {
	const apiPath = path.resolve(__dirname, '..', 'src', 'api.ts')
	// We can't easily import .ts in plain Node, so we inline the fetch logic
	// to match exactly what api.ts does.
	const url = `${baseUrl}${endpoint}`
	const init = { method }
	if (body) {
		init.headers = { 'Content-Type': 'application/json' }
		init.body = JSON.stringify(body)
	}

	const res = await fetch(url, init)
	const text = await res.text()
	return JSON.parse(text)
}

// ── Curl Test ────────────────────────────────────────────────────────────────

/** Run curl with -v and capture the raw request + response. */
function testViaCurl(url, method = 'GET', body = null) {
	return new Promise((resolve, reject) => {
		const args = ['-v', '-s', '-X', method]
		if (body) {
			args.push('-H', 'Content-Type: application/json')
			args.push('-d', JSON.stringify(body))
		}
		args.push(url)

		const proc = spawn('curl', args, { shell: true })
		const stderr = []
		const stdout = []

		proc.stderr.on('data', (d) => stderr.push(d.toString()))
		proc.stdout.on('data', (d) => stdout.push(d.toString()))

		proc.on('close', (code) => {
			if (code !== 0) {
				reject(new Error(`curl exited ${code}\n${stderr.join('')}`))
				return
			}
			// Parse the response body from stdout
			try {
				const body = JSON.parse(stdout.join(''))
				resolve(body)
			} catch {
				resolve({ raw: stdout.join('') })
			}
		})
	})
}

// ── Diff Helper ──────────────────────────────────────────────────────────────

function diff(labelA, a, labelB, b) {
	console.log(`\n${'─'.repeat(70)}`)
	console.log(`  ${labelA.padEnd(30)} │ ${labelB}`)
	console.log(`${'─'.repeat(70)}`)

	const keys = new Set([...Object.keys(a), ...Object.keys(b)])
	for (const key of [...keys].sort()) {
		const va = a[key]
		const vb = b[key]
		const same = JSON.stringify(va) === JSON.stringify(vb)
		const marker = same ? ' ' : '◆'
		const color = same ? '\x1b[90m' : '\x1b[33m'
		const reset = '\x1b[0m'
		console.log(
			`${color}${marker} ${key.padEnd(28)} │ ${JSON.stringify(va)?.substring(0, 35) || 'undefined'} │ ${JSON.stringify(vb)?.substring(0, 35) || 'undefined'}${reset}`,
		)
	}
}

// ── Main ─────────────────────────────────────────────────────────────────────

const PORT = 19666
const BASE = `http://127.0.0.1:${PORT}`

async function main() {
	console.log('🔬 Starting Copal diagnostic server...')
	const server = await startEchoServer(PORT)
	console.log(`   Echo server listening on ${BASE}\n`)

	const tests = [
		{ name: 'GET /health', endpoint: '/health', method: 'GET' },
		{
			name: 'GET /instances/:id',
			endpoint: '/instances/550e8400-e29b-41d4-a716-446655440000',
			method: 'GET',
		},
		{
			name: 'POST /instances/:id/command',
			endpoint: '/instances/550e8400-e29b-41d4-a716-446655440000/command',
			method: 'POST',
			body: { command: 'say hello' },
		},
		{
			name: 'PATCH /instances/:id/properties',
			endpoint: '/instances/550e8400-e29b-41d4-a716-446655440000/properties',
			method: 'PATCH',
			body: { 'max-players': '42' },
		},
		{ name: 'POST /ws-token', endpoint: '/ws-token', method: 'POST' },
	]

	for (const test of tests) {
		console.log(`\n${'═'.repeat(70)}`)
		console.log(`  TEST: ${test.name}`)
		console.log(`${'═'.repeat(70)}`)

		let ccResult, curlResult
		try {
			ccResult = await testViaCoreClient(BASE, test.endpoint, test.method, test.body)
		} catch (e) {
			ccResult = { error: e.message }
		}

		try {
			curlResult = await testViaCurl(`${BASE}${test.endpoint}`, test.method, test.body)
		} catch (e) {
			curlResult = { error: e.message }
		}

		diff('core-client (fetch)', ccResult, 'curl', curlResult)
	}

	console.log('\n' + '═'.repeat(70))
	console.log('  DIAGNOSTIC COMPLETE')
	console.log('═'.repeat(70))
	console.log('\nIf curl works but core-client fails, check:')
	console.log('  1. CORS preflight — is an OPTIONS request being blocked?')
	console.log('  2. User-Agent — some servers reject unknown agents.')
	console.log('  3. Origin header — WebView origin may differ from curl.')
	console.log('  4. Protocol — ensure http vs https matches Core config.')
	console.log('  5. Tauri fetch — in Tauri v2, fetch() goes through the WebView.')
	console.log('     Try using @tauri-apps/plugin-http instead of global fetch().')
	console.log()

	server.close()
}

main().catch((e) => {
	console.error(e)
	process.exit(1)
})
