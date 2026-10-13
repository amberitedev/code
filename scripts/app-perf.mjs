#!/usr/bin/env node
// app-perf.mjs measures how responsive the running dev App feels, page by page.
//
// It connects to the App's WebView2 debugging port, clicks through a fixed tour with real mouse
// input, and records per click: time to the next paint, time until the page stops changing, main
// thread blocking, dropped frames, layout cost, and the scripts responsible.
//
// Start the dev stack with the port open, then run the tour:
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=19222 --remote-allow-origins=*" vp run dev
//   node scripts/app-perf.mjs run --label baseline
//   node scripts/app-perf.mjs run --label after --baseline .data/perf/baseline.json
//   node scripts/app-perf.mjs run --label browse --groups browse,project
//   node scripts/app-perf.mjs eval "location.pathname"
//
// Dev mode serves unbundled modules, so numbers rank pages and compare before/after. They are not
// production speeds. Keep the App window visible and leave the machine idle while it runs.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

// A real user rests on a target briefly before pressing, which is when hover prefetching can run.
const HOVER_DWELL_MS = 100
// A click has settled once nothing changed and nothing was in flight for this long.
const QUIET_MS = 700
const SETTLE_TIMEOUT_MS = 15_000

// Each step clicks one element from wherever the previous step left the App. `find` is evaluated in
// the page and returns the element to click. `--groups` limits a run to some groups.
const TOUR = [
	{ name: 'Home', group: 'home', find: `document.querySelector('.app-grid-navbar a[href="/"]')` },
	{
		name: 'Browse',
		group: 'browse',
		find: `document.querySelector('.app-grid-navbar a[href="/browse/modpack"]')`,
	},
	{
		name: 'Browse: Mods tab',
		group: 'browse',
		find: `document.querySelector('nav a[href="/browse/mod"]')`,
	},
	{
		name: 'Browse: Resource Packs tab',
		group: 'browse',
		find: `document.querySelector('nav a[href^="/browse/resourcepack"]')`,
	},
	{
		name: 'Browse: Modpacks tab',
		group: 'browse',
		find: `document.querySelector('nav a[href^="/browse/modpack"]')`,
	},
	{
		name: 'Project',
		group: 'project',
		find: `document.querySelector('.app-viewport a[href^="/project/"]')`,
	},
	{
		name: 'Project: Versions tab',
		group: 'project',
		find: `document.querySelector('nav a[href*="/versions"]')`,
	},
	{
		name: 'Project: Gallery tab',
		group: 'project',
		find: `document.querySelector('nav a[href*="/gallery"]')`,
	},
	{
		name: 'Home (from project)',
		group: 'project',
		find: `document.querySelector('.app-grid-navbar a[href="/"]')`,
	},
	{
		name: 'Instance',
		group: 'instance',
		find: `document.querySelector('.app-grid-navbar a[href^="/instance/"]')`,
	},
	{
		name: 'Instance: Worlds tab',
		group: 'instance',
		find: `document.querySelector('nav a[href$="/worlds"]')`,
	},
	{
		name: 'Instance: Files tab',
		group: 'instance',
		find: `document.querySelector('nav a[href$="/files"]')`,
	},
	{
		name: 'Instance: Logs tab',
		group: 'instance',
		find: `document.querySelector('nav a[href$="/logs"]')`,
	},
	{
		name: 'Skins',
		group: 'skins',
		find: `document.querySelector('.app-grid-navbar a[href="/skins"]')`,
	},
	{
		name: 'Servers',
		group: 'servers',
		find: `document.querySelector('.app-grid-navbar a[href="/hosting/manage"]')`,
	},
]

// Steps a group needs clicked first to reach its starting page. They are clicked but not recorded.
const LEAD_IN = { project: ['Browse'] }

// Picks the steps for a run: a custom tour file, some groups of the built-in tour, or all of it.
// A custom tour is a JSON array of { name, find, leadIn? }.
function selectTour(options) {
	if (options.tour) return JSON.parse(readFileSync(options.tour, 'utf8'))
	if (!options.groups) return TOUR
	const groups = options.groups.split(',')
	const leadIns = new Set(groups.flatMap((group) => LEAD_IN[group] ?? []))
	return TOUR.filter((step) => groups.includes(step.group) || leadIns.has(step.name)).map(
		(step) => ({ ...step, leadIn: !groups.includes(step.group) }),
	)
}

// Installed once per page load. Collects raw entries; all math happens in Node.
const RECORDER = `(() => {
	if (window.__appPerf) return
	const state = { inputAt: 0, events: [], longTasks: [], loafs: [], frames: [], shift: 0, firstMutation: 0, lastMutation: 0 }
	// Requests still open keep a click from counting as settled.
	let openRequests = 0
	let lastRequestEnd = 0
	let resetAt = 0
	const nativeFetch = window.fetch
	window.fetch = (...args) => {
		openRequests++
		return nativeFetch(...args).finally(() => {
			openRequests--
			lastRequestEnd = performance.now()
		})
	}
	const observe = (type, onEntry, options = {}) => {
		try {
			new PerformanceObserver((list) => list.getEntries().forEach(onEntry)).observe({ type, ...options })
		} catch {}
	}
	observe('longtask', (e) => state.longTasks.push({ start: e.startTime, duration: e.duration }))
	observe('long-animation-frame', (e) =>
		state.loafs.push({
			start: e.startTime,
			duration: e.duration,
			scripts: e.scripts.map((s) => ({
				source: s.sourceURL,
				fn: s.sourceFunctionName || s.invoker,
				duration: s.duration,
				forcedLayout: s.forcedStyleAndLayoutDuration,
			})),
		}),
	)
	observe('event', (e) => state.events.push({ name: e.name, start: e.startTime, duration: e.duration }), { durationThreshold: 16 })
	observe('resource', (e) => { lastRequestEnd = Math.max(lastRequestEnd, e.responseEnd) })
	observe('layout-shift', (e) => { if (!e.hadRecentInput) state.shift += e.value })
	new MutationObserver(() => {
		const now = performance.now()
		if (!state.firstMutation) state.firstMutation = now
		state.lastMutation = now
	}).observe(document, { childList: true, subtree: true, characterData: true })
	addEventListener('pointerdown', (e) => { if (!state.inputAt) state.inputAt = e.timeStamp }, { capture: true })
	const frame = (now) => { state.frames.push(now); requestAnimationFrame(frame) }
	requestAnimationFrame(frame)
	window.__appPerf = {
		reset() {
			resetAt = performance.now()
			Object.assign(state, { inputAt: 0, events: [], longTasks: [], loafs: [], frames: [], shift: 0, firstMutation: 0, lastMutation: 0 })
		},
		activity() {
			const lastLoaf = state.loafs.at(-1)
			return { now: performance.now(), openRequests, lastChange: Math.max(state.inputAt, state.lastMutation, lastRequestEnd, lastLoaf ? lastLoaf.start + lastLoaf.duration : 0) }
		},
		snapshot() {
			// A click that never reached the page has no input time, so measure from the reset.
			return JSON.stringify({ ...state, inputAt: state.inputAt || resetAt, path: location.pathname + location.search })
		},
	}
})()`

async function connect(port) {
	const targets = await fetch(`http://127.0.0.1:${port}/json/list`)
		.then((response) => response.json())
		.catch(() => {
			throw new Error(
				`No debugging port on ${port}. Start the dev stack with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS set (see the top of this file).`,
			)
		})
	const target = targets.find((t) => t.type === 'page' && t.url.startsWith('http://localhost'))
	if (!target) throw new Error(`No App page found on port ${port}.`)

	const socket = new WebSocket(target.webSocketDebuggerUrl)
	await new Promise((resolve, reject) => {
		socket.addEventListener('open', resolve, { once: true })
		socket.addEventListener(
			'error',
			() => reject(new Error('Could not open the debugging socket.')),
			{
				once: true,
			},
		)
	})

	let nextId = 0
	const pending = new Map()
	socket.addEventListener('message', (message) => {
		const data = JSON.parse(message.data)
		if (data.id === undefined) return
		const { resolve, reject } = pending.get(data.id)
		pending.delete(data.id)
		if (data.error) reject(new Error(data.error.message))
		else resolve(data.result)
	})

	socket.addEventListener('close', (event) => {
		const error = new Error(`Debugging socket closed (${event.code} ${event.reason})`)
		for (const { reject } of pending.values()) reject(error)
		pending.clear()
	})

	const send = (method, params = {}) =>
		new Promise((resolve, reject) => {
			const id = ++nextId
			pending.set(id, { resolve, reject })
			socket.send(JSON.stringify({ id, method, params }))
		})

	// Evaluates an expression in the page and returns its JSON-serializable value.
	const evaluate = async (expression) => {
		const { result, exceptionDetails } = await send('Runtime.evaluate', {
			expression,
			returnByValue: true,
			awaitPromise: true,
		})
		if (exceptionDetails) {
			throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text)
		}
		return result.value
	}

	return {
		send,
		evaluate,
		close: () => socket.close(),
		url: target.url,
	}
}

const median = (values) => {
	const sorted = values.toSorted((a, b) => a - b)
	const middle = Math.floor(sorted.length / 2)
	return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}

async function timelineMetrics(page) {
	const { metrics } = await page.send('Performance.getMetrics')
	return Object.fromEntries(metrics.map((metric) => [metric.name, metric.value]))
}

// Dev module URLs carry the origin, a cache-busting query, and an absolute path for workspace files.
const sourceLabel = (url) =>
	url
		.replace(/^https?:\/\/[^/]+/, '')
		.replace(/\?.*$/, '')
		.replace(/^\/@fs\/.*?\/(?=(apps|packages|node_modules)\/)/, '')
		.replace(/^.*node_modules\/\.vite\//, '') || '(browser)'

// Turns a sampled CPU profile into the functions and files that used the most time themselves.
function hotspots(profile) {
	const nodes = new Map(profile.nodes.map((node) => [node.id, node.callFrame]))
	const functions = new Map()
	const files = new Map()
	profile.samples.forEach((id, index) => {
		const frame = nodes.get(id)
		if (['(idle)', '(program)', '(root)'].includes(frame.functionName)) return
		const ms = profile.timeDeltas[index] / 1000
		const file = sourceLabel(frame.url)
		const name = `${frame.functionName || '(anonymous)'} ${file}:${frame.lineNumber + 1}`
		functions.set(name, (functions.get(name) ?? 0) + ms)
		files.set(file, (files.get(file) ?? 0) + ms)
	})
	const top = (totals, limit) =>
		[...totals]
			.toSorted((a, b) => b[1] - a[1])
			.slice(0, limit)
			.map(([name, ms]) => ({ name, ms: Math.round(ms) }))
	return { functions: top(functions, 12), files: top(files, 8) }
}

// Clicks one tour step and returns its measurements, or null when the element is not on screen.
// With `profile`, it returns CPU hotspots instead; the sampler's overhead would skew the timings.
async function measureStep(page, step, profile = false) {
	const point = await page.evaluate(`(() => {
		const element = ${step.find}
		if (!element) return null
		element.scrollIntoView({ block: 'center', inline: 'center' })
		const rect = element.getBoundingClientRect()
		return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }
	})()`)
	if (!point) return null

	const mouse = (type, extra = {}) =>
		page.send('Input.dispatchMouseEvent', { type, x: point.x, y: point.y, ...extra })
	await mouse('mouseMoved')
	await sleep(HOVER_DWELL_MS)

	await page.evaluate('window.__appPerf.reset()')
	const before = await timelineMetrics(page)
	if (profile) await page.send('Profiler.start')
	await mouse('mousePressed', { button: 'left', clickCount: 1 })
	await mouse('mouseReleased', { button: 'left', clickCount: 1 })

	const startedAt = Date.now()
	let settled = false
	while (Date.now() - startedAt < SETTLE_TIMEOUT_MS) {
		await sleep(100)
		const activity = await page.evaluate('window.__appPerf.activity()')
		if (activity.openRequests === 0 && activity.now - activity.lastChange > QUIET_MS) {
			settled = true
			break
		}
	}

	if (profile) {
		const result = await page.send('Profiler.stop')
		await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 })
		return hotspots(result.profile)
	}
	const after = await timelineMetrics(page)
	const raw = JSON.parse(await page.evaluate('window.__appPerf.snapshot()'))
	// Park the pointer so the next step's hover starts fresh.
	await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 })

	const end = Math.max(raw.lastMutation, raw.inputAt)
	const window = (entry) => entry.start >= raw.inputAt - 1 && entry.start <= end + QUIET_MS
	const input = raw.events.filter(
		(event) =>
			['pointerdown', 'pointerup', 'mousedown', 'mouseup', 'click'].includes(event.name) &&
			window(event),
	)
	const longTasks = raw.longTasks.filter(window)

	const gaps = raw.frames
		.filter((time) => time >= raw.inputAt && time <= end + 100)
		.map((time, index, times) => time - times[index - 1])
		.slice(1)
	const budget = gaps.length ? median(gaps) : 0
	const dropped = gaps.reduce((total, gap) => total + Math.max(0, Math.round(gap / budget) - 1), 0)

	const bySource = new Map()
	for (const loaf of raw.loafs.filter(window)) {
		for (const script of loaf.scripts) {
			const source = sourceLabel(script.source)
			const entry = bySource.get(source) ?? { source, duration: 0, forcedLayout: 0 }
			entry.duration += script.duration
			entry.forcedLayout += script.forcedLayout
			bySource.set(source, entry)
		}
	}

	const delta = (name) => (after[name] - before[name]) * 1000
	return {
		path: raw.path,
		settled,
		// Input to the first frame that could show a response. Entries under 16ms are not reported.
		nextPaint: input.length ? Math.max(...input.map((event) => event.duration)) : 16,
		firstChange: raw.firstMutation ? raw.firstMutation - raw.inputAt : 0,
		settle: end - raw.inputAt,
		blocked: longTasks.reduce((total, task) => total + Math.max(0, task.duration - 50), 0),
		longTasks: longTasks.length,
		worstFrame: gaps.length ? Math.max(...gaps) : 0,
		dropped,
		layoutShift: raw.shift,
		script: delta('ScriptDuration'),
		layout: delta('LayoutDuration'),
		style: delta('RecalcStyleDuration'),
		layouts: after.LayoutCount - before.LayoutCount,
		scripts: [...bySource.values()].toSorted((a, b) => b.duration - a.duration).slice(0, 5),
	}
}

const NUMERIC = [
	'nextPaint',
	'firstChange',
	'settle',
	'blocked',
	'longTasks',
	'worstFrame',
	'dropped',
	'layoutShift',
	'script',
	'layout',
	'style',
	'layouts',
]

function summarize(runs) {
	const summary = Object.fromEntries(
		NUMERIC.map((key) => [key, median(runs.map((run) => run[key]))]),
	)
	const totals = new Map()
	for (const script of runs.flatMap((run) => run.scripts)) {
		totals.set(script.source, (totals.get(script.source) ?? 0) + script.duration / runs.length)
	}
	summary.scripts = [...totals]
		.toSorted((a, b) => b[1] - a[1])
		.slice(0, 3)
		.map(([source, duration]) => ({ source, duration }))
	return summary
}

function printTable(steps, baseline) {
	const ms = (value) => `${Math.round(value)}ms`
	const versus = (step, key) => {
		const previous = baseline?.steps.find((other) => other.name === step.name)?.median[key]
		if (previous === undefined) return ms(step.median[key])
		const change = Math.round(step.median[key] - previous)
		return `${ms(step.median[key])} (${change > 0 ? '+' : ''}${change})`
	}
	const rows = steps.map((step) => ({
		step: step.name,
		'next paint': versus(step, 'nextPaint'),
		settle: versus(step, 'settle'),
		blocked: versus(step, 'blocked'),
		'worst frame': ms(step.median.worstFrame),
		dropped: step.median.dropped,
		layout: ms(step.median.layout + step.median.style),
		'top script': step.median.scripts[0]
			? `${step.median.scripts[0].source.split('/').slice(-2).join('/')} ${ms(step.median.scripts[0].duration)}`
			: '',
	}))
	console.table(rows)
}

async function run(options) {
	const page = await connect(options.port)
	await page.send('Page.enable')
	await page.send('Performance.enable')
	await page.send('Page.bringToFront')

	// Reloading starts every run from the same state and installs the recorder before App code, so
	// it sees every request. The Network domain is not used: its events drop the socket on some pages.
	await page.send('Page.addScriptToEvaluateOnNewDocument', { source: RECORDER })
	await page.send('Page.reload')
	for (let waited = 0; ; waited += 250) {
		await sleep(250)
		const ready = await page
			.evaluate(
				`!!window.__appPerf && !!document.querySelector('.app-grid-navbar a') && !document.querySelector('.splash-screen')`,
			)
			.catch(() => false)
		if (ready) break
		if (waited > 60_000) throw new Error('The App did not finish loading after a reload.')
	}
	await sleep(2000)
	if ((await page.evaluate('document.visibilityState')) !== 'visible') {
		throw new Error(
			'The App window is hidden, so it is not painting. Bring it on screen and rerun.',
		)
	}

	const tour = selectTour(options)
	const results = new Map(tour.map((step) => [step.name, []]))
	const skipped = new Map()
	// Pass 0 loads every page's modules and data and is thrown away.
	for (let pass = 0; pass <= options.passes; pass++) {
		for (const step of tour) {
			const measurement = await measureStep(page, step)
			if (!measurement) {
				skipped.set(step.name, (skipped.get(step.name) ?? 0) + 1)
				continue
			}
			if (pass > 0 && !step.leadIn) results.get(step.name).push(measurement)
			process.stderr.write(
				`${pass === 0 ? 'warm-up' : `pass ${pass}`}  ${step.name}  ${Math.round(measurement.settle)}ms${measurement.settled ? '' : '  (never settled)'}\n`,
			)
		}
	}

	// One more pass under the CPU sampler says which code the time went to.
	await page.send('Profiler.enable')
	await page.send('Profiler.setSamplingInterval', { interval: 500 })
	const profiles = new Map()
	for (const step of tour) {
		const profile = await measureStep(page, step, true)
		if (profile) profiles.set(step.name, profile)
		process.stderr.write(`profile  ${step.name}\n`)
	}

	const steps = [...results]
		.filter(([, runs]) => runs.length > 0)
		.map(([name, runs]) => ({ name, median: summarize(runs), hotspots: profiles.get(name), runs }))
	const output = join(options.out, `${options.label}.json`)
	mkdirSync(dirname(output), { recursive: true })
	writeFileSync(
		output,
		JSON.stringify(
			{ label: options.label, recordedAt: new Date().toISOString(), passes: options.passes, steps },
			null,
			'\t',
		),
	)

	printTable(
		steps,
		options.baseline ? JSON.parse(readFileSync(options.baseline, 'utf8')) : undefined,
	)
	for (const step of steps) {
		const functions = step.hotspots?.functions
			.slice(0, 6)
			.map((entry) => `${entry.name} ${entry.ms}ms`)
		if (functions?.length) console.log(`${step.name} hotspots:\n  ${functions.join('\n  ')}`)
	}
	if (skipped.size) {
		const missed = [...skipped].map(
			([name, passes]) => `${name} (${passes} of ${options.passes + 1} passes)`,
		)
		console.log(
			`Element not found, so later steps may have started from the wrong page: ${missed.join(', ')}`,
		)
	}
	console.log(`Saved ${output}`)
	page.close()
}

const { values, positionals } = parseArgs({
	allowPositionals: true,
	options: {
		port: { type: 'string', default: process.env.APP_CDP_PORT ?? '19222' },
		label: { type: 'string', default: 'run' },
		passes: { type: 'string', default: '3' },
		baseline: { type: 'string' },
		groups: { type: 'string' },
		tour: { type: 'string' },
		out: { type: 'string', default: join(root, '.data', 'perf') },
	},
})

const [command, ...rest] = positionals
if (command === 'run') {
	await run({ ...values, port: Number(values.port), passes: Number(values.passes) })
} else if (command === 'eval') {
	const page = await connect(Number(values.port))
	console.log(JSON.stringify(await page.evaluate(rest.join(' ')), null, '\t'))
	page.close()
} else {
	console.log(
		'Usage: node scripts/app-perf.mjs <run|eval> [--label name] [--passes 3] [--baseline file] [--groups home,browse,project,instance,skins,servers] [--tour file.json] [--out dir] [--port 19222]',
	)
	process.exitCode = 1
}
