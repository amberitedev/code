#!/usr/bin/env node
// app-debug.mjs — unified dev tool for Amberite app + core with structured output for agents.
// start():L75  coreStart():L130  appConsole():L300  appNetwork():L350  coreTsx():L420
// appEval():L270  coreApi():L450  healthAll():L500  main():L550
import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync, unlinkSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import os from 'node:os'
import net from 'node:net'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const dir = join(os.tmpdir(), 'opencode', 'amberite-app-debug')
const appStateFile = join(dir, 'app-state.json')
const coreStateFile = join(dir, 'core-state.json')

const APP_TITLE = 'Amberite Dev'
const APP_CDP_PORT = process.env.APP_CDP_PORT ? Number(process.env.APP_CDP_PORT) : 9222
const cdpBase = `http://127.0.0.1:${APP_CDP_PORT}`

const CORE_TITLE = 'Copal Dev'
const CORE_HOST = process.env.CORE_HOST || '127.0.0.1'
const CORE_PORT = Number(process.env.CORE_PORT) || 16662
const CORE_TIMEOUT_MS = Number(process.env.CORE_DEV_TIMEOUT) || 180_000

const JSON_MODE = process.argv.includes('--json')
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

function log(data) {
	if (JSON_MODE) {
		if (typeof data === 'string') console.log(JSON.stringify({ message: data }))
		else console.log(JSON.stringify(data))
	} else {
		if (typeof data === 'string') console.log(data)
		else console.dir(data, { depth: null })
	}
}

function usage() {
	log(`Usage: node scripts/app-debug.mjs <command> [args] [--json]

App (frontend):
  app start                  Open visible terminal with pnpm app:dev + CDP
  app stop                   Kill app terminal
  app restart                Stop then start
  app status                 Terminal alive? CDP up? Targets?
  app health                 Quick health check (CDP + page reachable)
  app console [s]              Stream console + exceptions for s sec (default 30)
  app errors [s]             Stream only exceptions for s sec (default 30)
  app eval <js>              Run JS in app DevTools context
  app js <js>                Alias for eval
  app network [s]            Monitor network requests for s sec (default 30)
  app screenshot             Capture PNG screenshot
  app dom <selector>         Query DOM (default: body)
  app config                 Show app configuration

Core (backend):
  core start                 Open visible terminal with cargo run
  core stop                  Kill core terminal
  core restart               Stop then start
  core status                Process alive? Port open?
  core health                Hit health endpoint
  core logs [lines]          Show last log lines (default 50)
  core shell <cmd>           Run command in core directory
  core test                  Run cargo test
  core check                 Run cargo check
  core tsx <code>            Execute TypeScript code (fetches core API)
  core run <file>            Run TypeScript file against core
  core api <method> <path>   Call core HTTP API (e.g. GET /health)

Combined:
  start-all                  Start app + core
  stop-all                   Stop app + core
  status-all                 Status of both
  health-all                 Health check both

Global flags:
  --json                     Output structured JSON for agent consumption
`)
}

function ensureDir() { mkdirSync(dir, { recursive: true }) }
function readState(file) { if (!existsSync(file)) return null; try { return JSON.parse(readFileSync(file, 'utf8')) } catch { return null } }
function saveState(file, state) { ensureDir(); writeFileSync(file, JSON.stringify(state, null, 2)) }

function isAlive(pid) {
	if (!pid) return false
	try { process.kill(pid, 0); return true } catch { return false }
}

function ps(v) { return `'${String(v).replaceAll("'", "''")}'` }

function findWindowByTitle(title) {
	const r = spawnSync('powershell.exe', [
		'-NoLogo', '-NoProfile', '-NonInteractive', '-Command',
		`$p = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -eq ${ps(title)} } | Select-Object -First 1; if ($p) { $p.Id }`,
	], { encoding: 'utf8', windowsHide: true, timeout: 7000 })
	const n = Number(r.stdout.trim())
	return Number.isInteger(n) && n > 0 ? n : null
}

function openTerminal(title, cwd, env, command) {
	if (process.platform === 'win32') {
		const inner = [
			`$host.UI.RawUI.WindowTitle = ${ps(title)}`,
			...Object.entries(env).map(([k, v]) => `$env:${k} = ${ps(v)}`),
			`Set-Location -LiteralPath ${ps(cwd)}`,
			command,
		].join('; ')
		const outer = [
			`$p = Start-Process powershell.exe -ArgumentList @('-NoLogo','-NoProfile','-NoExit','-Command',${ps(inner)}) -WindowStyle Normal -PassThru`,
			`Write-Output $p.Id`,
		].join('; ')
		const result = spawnSync('powershell.exe', ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', outer], {
			encoding: 'utf8', windowsHide: true, timeout: 8000,
		})
		if (result.status !== 0) throw new Error(result.stderr?.trim() || 'Failed to open terminal')
		const pid = Number(result.stdout.trim().split(/\r?\n/).at(-1)?.trim())
		if (!Number.isInteger(pid) || pid <= 0) throw new Error(`Unexpected PID: ${JSON.stringify(result.stdout)}`)
		return pid
	}
	const child = spawn('sh', ['-c', command], {
		cwd, detached: true, stdio: 'ignore', env: { ...process.env, ...env },
	})
	child.unref()
	return child.pid
}

function killProcessTree(pid) {
	if (!pid) return
	if (process.platform === 'win32') {
		spawnSync('taskkill.exe', ['/PID', String(pid), '/T', '/F'], { windowsHide: true, timeout: 8000 })
	} else {
		try { process.kill(-pid, 'SIGKILL') } catch {}
	}
}

function isPortOpen(host, port, timeout = 2000) {
	return new Promise((resolve) => {
		const socket = new net.Socket()
		socket.setTimeout(timeout)
		socket.once('connect', () => { socket.destroy(); resolve(true) })
		socket.once('error', () => resolve(false))
		socket.once('timeout', () => { socket.destroy(); resolve(false) })
		socket.connect(port, host)
	})
}

async function waitForPort(host, port, timeoutMs, intervalMs = 1000) {
	const end = Date.now() + timeoutMs
	while (Date.now() < end) {
		if (await isPortOpen(host, port)) return true
		await sleep(intervalMs)
	}
	return false
}

function tail(file, lines) {
	if (!existsSync(file)) return '(missing)'
	const text = readFileSync(file, { encoding: 'utf8', flag: 'r' })
	return text.split(/\r?\n/).slice(-lines).join('\n')
}

// ----- App -----

function appStart() {
	const state = readState(appStateFile)
	if (state?.terminalPid && isAlive(state.terminalPid)) {
		log(`App already running (pid ${state.terminalPid})`)
		return
	}
	const existing = findWindowByTitle(APP_TITLE)
	if (existing) {
		saveState(appStateFile, { terminalPid: existing })
		log(`App window already open (pid ${existing})`)
		return
	}
	const pnpm = process.env.APPDATA ? join(process.env.APPDATA, 'npm', 'pnpm.cmd') : 'pnpm'
	const pid = openTerminal(APP_TITLE, root, {
		WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${APP_CDP_PORT} --auto-open-devtools-for-tabs`,
		RUST_LOG: 'info',
	}, process.platform === 'win32' ? `& ${ps(pnpm)} app:dev` : 'pnpm app:dev')
	saveState(appStateFile, { terminalPid: pid })
	log(`App terminal opened (pid ${pid})`)
	log(`DevTools auto-open; CDP at ${cdpBase}`)
}

async function appStop() {
	const state = readState(appStateFile)
	let pid = state?.terminalPid
	if (pid && !isAlive(pid)) pid = null
	if (!pid) pid = findWindowByTitle(APP_TITLE)
	if (!pid) { log('No app terminal found'); return }
	killProcessTree(pid)
	try { unlinkSync(appStateFile) } catch {}
	log(`App stopped (pid ${pid})`)
}

async function appStatus() {
	const state = readState(appStateFile)
	const pid = state?.terminalPid
	const alive = isAlive(pid)
	const byTitle = findWindowByTitle(APP_TITLE)
	let cdpUp = false
	let targets = []
	try {
		targets = await getTargets(3000)
		cdpUp = true
	} catch (e) {
		// CDP unavailable
	}
	if (JSON_MODE) {
		log({ scope: 'app', pid, alive, windowByTitle: byTitle, cdpUp, targetCount: targets.length, targets })
	} else {
		log(`app terminal  : ${pid ?? 'none'} (${alive ? 'alive' : 'not running'})`)
		log(`app window    : ${byTitle ? `pid ${byTitle}` : 'not found'}`)
		log(`app cdp       : ${cdpUp ? 'UP' : 'unavailable'}`)
		for (const t of targets) log(`  [${t.type}] ${t.title}  ${t.url}`)
	}
}

async function appHealth() {
	let healthy = false
	let pageUrl = null
	let error = null
	try {
		const targets = await getTargets(5000)
		const page = targets.find((t) => t.type === 'page' && t.url.includes('localhost:1420'))
			?? targets.find((t) => t.type === 'page')
		if (page) {
			healthy = true
			pageUrl = page.url
		}
	} catch (e) {
		error = e.message
	}
	if (JSON_MODE) {
		log({ scope: 'app', healthy, pageUrl, error })
	} else {
		log(`app health    : ${healthy ? 'HEALTHY' : 'UNHEALTHY'}`)
		if (pageUrl) log(`page          : ${pageUrl}`)
		if (error) log(`error         : ${error}`)
	}
}

function appConfig() {
	const config = {
		cdpPort: APP_CDP_PORT,
		cdpBase,
		appStateFile,
		tempDir: dir,
		repoRoot: root,
	}
	log(config)
}

// ----- Core -----

async function coreStart() {
	const state = readState(coreStateFile)
	if (state?.terminalPid && isAlive(state.terminalPid)) {
		if (await isPortOpen(CORE_HOST, CORE_PORT)) {
			log(`Core already online at http://${CORE_HOST}:${CORE_PORT}`)
			return
		}
		log('Core process exists but port not open yet...')
	}
	if (await isPortOpen(CORE_HOST, CORE_PORT)) {
		log(`Core already listening on port ${CORE_PORT}`)
		return
	}
	const coreDir = join(root, 'apps', 'core')
	const coreLog = join(coreDir, 'logs', 'core-dev.log')
	const command = [
		`New-Item -ItemType Directory -Force -Path ${ps(join(coreDir, 'logs'))} | Out-Null`,
		`"" | Set-Content ${ps(coreLog)} -NoNewline`,
		`cargo run 2>&1 | Tee-Object -FilePath ${ps(coreLog)}`,
	].join('; ')
	const pid = openTerminal(CORE_TITLE, coreDir, { RUST_LOG: 'info' }, command)
	saveState(coreStateFile, { terminalPid: pid, host: CORE_HOST, port: CORE_PORT })
	log(`Core terminal opened (pid ${pid}), waiting for port ${CORE_PORT}...`)
	const up = await waitForPort(CORE_HOST, CORE_PORT, CORE_TIMEOUT_MS)
	if (up) {
		log(`Core is ONLINE at http://${CORE_HOST}:${CORE_PORT}`)
	} else {
		log(`Core did not come up within ${CORE_TIMEOUT_MS / 1000}s`)
		log(`Last logs:\n${tail(coreLog, 50)}`)
		await coreStop()
	}
}

async function coreStop() {
	const state = readState(coreStateFile)
	let pid = state?.terminalPid
	if (pid && !isAlive(pid)) pid = null
	if (!pid) {
		const r = spawnSync('powershell.exe', [
			'-NoLogo', '-NoProfile', '-NonInteractive', '-Command',
			`$c = Get-NetTCPConnection -LocalPort ${CORE_PORT} -ErrorAction SilentlyContinue | Select-Object -First 1; if ($c) { $c.OwningProcess }`,
		], { encoding: 'utf8', windowsHide: true, timeout: 5000 })
		pid = Number(r.stdout.trim()) || null
	}
	if (!pid) { log('No core process found'); return }
	killProcessTree(pid)
	try { unlinkSync(coreStateFile) } catch {}
	log(`Core stopped (pid ${pid})`)
}

async function coreStatus() {
	const state = readState(coreStateFile)
	const pid = state?.terminalPid
	const alive = isAlive(pid)
	const portOpen = await isPortOpen(CORE_HOST, CORE_PORT, 2000)
	if (JSON_MODE) {
		log({ scope: 'core', pid, alive, portOpen, host: CORE_HOST, port: CORE_PORT })
	} else {
		log(`core terminal : ${pid ?? 'none'} (${alive ? 'alive' : 'not running'})`)
		log(`core port     : ${CORE_HOST}:${CORE_PORT} ${portOpen ? 'OPEN' : 'CLOSED'}`)
	}
}

async function coreHealth() {
	let healthy = false
	let statusCode = null
	let error = null
	try {
		const res = await fetch(`http://${CORE_HOST}:${CORE_PORT}/health`, { signal: AbortSignal.timeout(5000) })
		statusCode = res.status
		healthy = res.ok
	} catch (e) {
		error = e.message
	}
	if (JSON_MODE) {
		log({ scope: 'core', healthy, statusCode, error })
	} else {
		log(`core health   : ${healthy ? 'HEALTHY' : 'UNHEALTHY'} (${statusCode ?? 'no response'})`)
		if (error) log(`error         : ${error}`)
	}
}

function coreLogs(lines = 50) {
	const coreLog = join(root, 'apps', 'core', 'logs', 'core-dev.log')
	if (JSON_MODE) {
		log({ scope: 'core', logFile: coreLog, lines, content: tail(coreLog, lines) })
	} else {
		log(`--- ${coreLog}\n${tail(coreLog, lines)}`)
	}
}

function coreShell(cmd) {
	const coreDir = join(root, 'apps', 'core')
	const r = spawnSync('powershell.exe', ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', cmd], {
		cwd: coreDir, encoding: 'utf8', windowsHide: true, timeout: 30000,
	})
	if (r.stdout) process.stdout.write(r.stdout)
	if (r.stderr) process.stderr.write(r.stderr)
	if (r.error) throw r.error
}

function coreTest() { coreShell('cargo test') }
function coreCheck() { coreShell('cargo check') }

async function coreTsx(code) {
	const tmpFile = join(dir, `core-script-${Date.now()}.ts`)
	const wrapped = `const CORE_BASE = 'http://${CORE_HOST}:${CORE_PORT}';
async function main() {
  try {
${code.split('\n').map((l) => '    ' + l).join('\n')}
  } catch (e) {
    console.error('Error:', e);
    process.exit(1);
  }
}
main();
`
	writeFileSync(tmpFile, wrapped)
	log(`Executing TS script: ${tmpFile}`)
	const r = spawnSync('npx', ['tsx', tmpFile], {
		cwd: root, encoding: 'utf8', shell: true, timeout: 30000,
	})
	if (r.stdout) process.stdout.write(r.stdout)
	if (r.stderr) process.stderr.write(r.stderr)
	try { unlinkSync(tmpFile) } catch {}
	if (r.status !== 0) throw new Error(`Script exited with code ${r.status}`)
}

async function coreApi(method, path, body) {
	const url = `http://${CORE_HOST}:${CORE_PORT}${path}`
	const options = {
		method,
		headers: { 'Content-Type': 'application/json' },
		signal: AbortSignal.timeout(10000),
	}
	if (body) options.body = body
	const res = await fetch(url, options)
	const responseBody = await res.text()
	if (JSON_MODE) {
		log({ scope: 'core', method, path, url, status: res.status, response: responseBody })
	} else {
		log(`[${res.status}] ${method} ${url}`)
		log(responseBody)
	}
}

// ----- CDP -----

async function getTargets(timeout = 3000) {
	const res = await fetch(`${cdpBase}/json/list`, { signal: AbortSignal.timeout(timeout) })
	if (!res.ok) throw new Error(`HTTP ${res.status}`)
	return res.json()
}

async function appTarget() {
	const list = await getTargets()
	const t = list.find((t) => t.type === 'page' && t.url.includes('localhost:1420'))
		?? list.find((t) => t.type === 'page')
	if (!t?.webSocketDebuggerUrl) throw new Error('No page target found')
	return t
}

async function cdpConnect(wsUrl) {
	if (!globalThis.WebSocket) throw new Error('WebSocket global unavailable — requires Node >= 22')
	const ws = new WebSocket(wsUrl)
	await Promise.race([
		new Promise((res, rej) => {
			ws.addEventListener('open', res, { once: true })
			ws.addEventListener('error', (e) => rej(new Error(String(e.message ?? 'WS error'))), { once: true })
		}),
		sleep(8000).then(() => { throw new Error('WebSocket connect timed out (8s)') }),
	])
	let id = 0
	const send = (method, params = {}) => { ws.send(JSON.stringify({ id: ++id, method, params })); return id }
	return { ws, send }
}

async function appConsole(seconds = 30, filter = null) {
	let target
	try {
		target = await Promise.race([appTarget(), sleep(10000).then(() => { throw new Error('Timed out waiting for target (10s)') })])
	} catch (e) { log(`Cannot attach: ${e.message}`); return }
	const { ws, send } = await cdpConnect(target.webSocketDebuggerUrl)
	if (!JSON_MODE) log(`Attached: ${target.title} — streaming for ${seconds}s`)
	const messages = []
	ws.addEventListener('message', ({ data }) => {
		const m = JSON.parse(data)
		if (m.method === 'Runtime.consoleAPICalled') {
			const entry = {
				type: 'console',
				level: m.params.type,
				args: m.params.args.map((a) => a.value ?? a.description ?? ''),
				timestamp: Date.now(),
			}
			if (!filter || filter === 'console') messages.push(entry)
		}
		if (m.method === 'Runtime.exceptionThrown') {
			const d = m.params.exceptionDetails
			const stackTrace = d.stackTrace?.callFrames?.map((f) => ({
				functionName: f.functionName,
				url: f.url,
				line: f.lineNumber + 1,
				column: f.columnNumber + 1,
			})) || []
			const entry = {
				type: 'exception',
				text: d.text,
				description: d.exception?.description,
				line: d.lineNumber + 1,
				column: d.columnNumber + 1,
				scriptId: d.scriptId,
				stackTrace,
				timestamp: Date.now(),
			}
			if (!filter || filter === 'exception') messages.push(entry)
		}
		if (m.method === 'Log.entryAdded') {
			const entry = {
				type: 'log',
				level: m.params.entry.level,
				text: m.params.entry.text,
				timestamp: Date.now(),
			}
			if (!filter || filter === 'log') messages.push(entry)
		}
	})
	send('Runtime.enable')
	send('Log.enable')
	await sleep(seconds * 1000)
	ws.close()
	if (JSON_MODE) {
		log({ scope: 'app', stream: 'console', duration: seconds, messageCount: messages.length, messages })
	} else {
		for (const m of messages) {
			if (m.type === 'console') log(`[${m.level}] ${m.args.join(' ')}`)
			if (m.type === 'exception') {
				log(`[exception] ${m.text}`)
				if (m.stackTrace.length > 0) {
					for (const frame of m.stackTrace.slice(0, 5)) {
						log(`  at ${frame.functionName || '<anonymous>'} (${frame.url}:${frame.line}:${frame.column})`)
					}
				}
			}
			if (m.type === 'log') log(`[${m.level}] ${m.text}`)
		}
	}
}

async function appErrors(seconds = 30) {
	return appConsole(seconds, 'exception')
}

async function appEval(expression) {
	let target
	try {
		target = await Promise.race([appTarget(), sleep(8000).then(() => { throw new Error('Timed out (8s)') })])
	} catch (e) { log(`Cannot attach: ${e.message}`); return }
	const { ws, send } = await cdpConnect(target.webSocketDebuggerUrl)
	const evalId = send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
	const result = await Promise.race([
		new Promise((resolve) => {
			ws.addEventListener('message', ({ data }) => {
				const m = JSON.parse(data)
				if (m.id !== evalId) return
				ws.close()
				resolve(m.result)
			})
		}),
		sleep(8000).then(() => { ws.close(); throw new Error('eval timed out (8s)') }),
	])
	if (JSON_MODE) {
		log({ scope: 'app', eval: expression, result })
	} else {
		log(JSON.stringify(result, null, 2))
	}
}

async function appNetwork(seconds = 30) {
	let target
	try {
		target = await Promise.race([appTarget(), sleep(10000).then(() => { throw new Error('Timed out (10s)') })])
	} catch (e) { log(`Cannot attach: ${e.message}`); return }
	const { ws, send } = await cdpConnect(target.webSocketDebuggerUrl)
	if (!JSON_MODE) log(`Monitoring network for ${seconds}s...`)
	const requests = new Map()
	const entries = []
	ws.addEventListener('message', ({ data }) => {
		const m = JSON.parse(data)
		if (m.method === 'Network.requestWillBeSent') {
			const entry = {
				requestId: m.params.requestId,
				method: m.params.request.method,
				url: m.params.request.url,
				timestamp: m.params.timestamp,
				type: 'request',
			}
			requests.set(m.params.requestId, entry)
			entries.push(entry)
		}
		if (m.method === 'Network.responseReceived') {
			const req = requests.get(m.params.requestId)
			if (req) {
				req.type = 'response'
				req.status = m.params.response.status
				req.statusText = m.params.response.statusText
				req.mimeType = m.params.response.mimeType
				req.timing = m.params.response.timing
			}
		}
		if (m.method === 'Network.loadingFailed') {
			const req = requests.get(m.params.requestId)
			if (req) {
				req.type = 'failed'
				req.errorText = m.params.errorText
			}
		}
	})
	send('Network.enable')
	await sleep(seconds * 1000)
	ws.close()
	if (JSON_MODE) {
		log({ scope: 'app', stream: 'network', duration: seconds, entryCount: entries.length, entries })
	} else {
		for (const e of entries) {
			if (e.type === 'request') log(`[→] ${e.method} ${e.url}`)
			if (e.type === 'response') log(`[←] ${e.status} ${e.method || ''} ${e.url}`)
			if (e.type === 'failed') log(`[✗] ${e.method || ''} ${e.url} — ${e.errorText}`)
		}
	}
}

async function appScreenshot() {
	let target
	try {
		target = await Promise.race([appTarget(), sleep(10000).then(() => { throw new Error('Timed out (10s)') })])
	} catch (e) { log(`Cannot attach: ${e.message}`); return }
	const { ws, send } = await cdpConnect(target.webSocketDebuggerUrl)
	send('Page.enable')
	await sleep(300)
	const id = send('Page.captureScreenshot', { format: 'png' })
	const result = await Promise.race([
		new Promise((resolve) => {
			ws.addEventListener('message', ({ data }) => {
				const m = JSON.parse(data)
				if (m.id !== id) return
				ws.close()
				resolve(m.result)
			})
		}),
		sleep(8000).then(() => { ws.close(); throw new Error('screenshot timed out (8s)') }),
	])
	if (result?.data) {
		const buf = Buffer.from(result.data, 'base64')
		const outPath = join(dir, `screenshot-${Date.now()}.png`)
		writeFileSync(outPath, buf)
		if (JSON_MODE) {
			log({ scope: 'app', screenshot: outPath, size: buf.length })
		} else {
			log(`Screenshot saved: ${outPath}`)
		}
	} else {
		if (JSON_MODE) {
			log({ scope: 'app', error: 'Screenshot failed', result })
		} else {
			log('Screenshot failed:', JSON.stringify(result))
		}
	}
}

async function appDom(selector) {
	let target
	try {
		target = await Promise.race([appTarget(), sleep(10000).then(() => { throw new Error('Timed out (10s)') })])
	} catch (e) { log(`Cannot attach: ${e.message}`); return }
	const { ws, send } = await cdpConnect(target.webSocketDebuggerUrl)
	const expression = `JSON.stringify(Array.from(document.querySelectorAll(${JSON.stringify(selector)})).map(el => ({ tag: el.tagName, text: el.textContent?.slice(0, 200), id: el.id, class: el.className, href: el.href, src: el.src })))`
	const evalId = send('Runtime.evaluate', { expression, returnByValue: true })
	const result = await Promise.race([
		new Promise((resolve) => {
			ws.addEventListener('message', ({ data }) => {
				const m = JSON.parse(data)
				if (m.id !== evalId) return
				ws.close()
				resolve(m.result?.result?.value)
			})
		}),
		sleep(8000).then(() => { ws.close(); throw new Error('dom timed out (8s)') }),
	])
	try {
		const parsed = result ? JSON.parse(result) : []
		if (JSON_MODE) {
			log({ scope: 'app', selector, elements: parsed })
		} else {
			log(JSON.stringify(parsed, null, 2))
		}
	} catch {
		log(result ?? 'null')
	}
}

// ----- Combined -----

async function healthAll() {
	await appHealth()
	await coreHealth()
}

// ----- CLI -----

function stripFlags(args) {
	return args.filter((a) => a !== '--json')
}

async function main() {
	const rawArgs = process.argv.slice(2)
	const args = stripFlags(rawArgs)
	const [a1, a2, ...rest] = args

	if (a1 === 'app') {
		switch (a2) {
			case 'start': return appStart()
			case 'stop': return appStop()
			case 'restart': { await appStop(); return appStart() }
			case 'status': return appStatus()
			case 'health': return appHealth()
			case 'console': return appConsole(Number(rest[0] ?? 30))
			case 'errors': return appErrors(Number(rest[0] ?? 30))
			case 'eval':
			case 'js': return appEval(rest.join(' '))
			case 'network': return appNetwork(Number(rest[0] ?? 30))
			case 'screenshot': return appScreenshot()
			case 'dom': return appDom(rest.join(' ') || 'body')
			case 'config': return appConfig()
		}
	}

	if (a1 === 'core') {
		switch (a2) {
			case 'start': return coreStart()
			case 'stop': return coreStop()
			case 'restart': { await coreStop(); return coreStart() }
			case 'status': return coreStatus()
			case 'health': return coreHealth()
			case 'logs': return coreLogs(Number(rest[0] ?? 50))
			case 'shell': return coreShell(rest.join(' '))
			case 'test': return coreTest()
			case 'check': return coreCheck()
			case 'tsx': return coreTsx(rest.join(' '))
			case 'run': return coreTsx(readFileSync(rest[0], 'utf8'))
			case 'api': return coreApi(rest[0] || 'GET', rest[1] || '/', rest[2])
		}
	}

	if (a1 === 'start-all') { appStart(); return coreStart() }
	if (a1 === 'stop-all') { await appStop(); return coreStop() }
	if (a1 === 'status-all') { await appStatus(); log('---'); return coreStatus() }
	if (a1 === 'health-all') return healthAll()

	switch (a1) {
		case 'start': return appStart()
		case 'stop': return appStop()
		case 'restart': { await appStop(); return appStart() }
		case 'status': return appStatus()
		case 'health': return appHealth()
		case 'console': return appConsole(Number(a2 ?? 30))
		case 'errors': return appErrors(Number(a2 ?? 30))
		case 'eval':
		case 'js': return appEval([a2, ...rest].join(' '))
		case 'network': return appNetwork(Number(a2 ?? 30))
		case 'screenshot': return appScreenshot()
		case 'dom': return appDom([a2, ...rest].join(' ') || 'body')
		case 'help':
		case undefined: return usage()
	}

	log(`Unknown command: ${a1}`)
	usage()
}

main().catch((error) => {
	if (JSON_MODE) {
		log({ error: error.message, stack: error.stack })
	} else {
		console.error(error.stack ?? error.message)
	}
	process.exit(1)
})
