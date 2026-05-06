import { readFileSync } from 'node:fs'
import http from 'node:http'
import { tmpdir } from 'node:os'
import { join, relative } from 'node:path'
import { parse as vueParse } from '@vue/compiler-dom'
import type { Plugin, ViteDevServer } from 'vite'

// ---------------------------------------------------------------------------
// Lock file helpers
// ---------------------------------------------------------------------------

function readLockFile(): { url: string; auth: string } | null {
	try {
		const raw = readFileSync(join(tmpdir(), 'opencurser-server.json'), 'utf8')
		const parsed = JSON.parse(raw)
		if (typeof parsed.url === 'string') return parsed
	} catch {}
	return null
}

function fetchActiveSessionId(target: string, auth: string): Promise<string | null> {
	return new Promise((resolve) => {
		const url = new URL('/design-session/active', target)
		const reqHeaders: Record<string, string> = { accept: 'application/json' }
		if (auth) reqHeaders['authorization'] = auth

		const req = http.request(url, { method: 'GET', headers: reqHeaders }, (res) => {
			let body = ''
			res.on('data', (chunk: Buffer) => { body += chunk.toString() })
			res.on('end', () => {
				try {
					const data = JSON.parse(body) as { sessionID?: string } | null
					resolve(data?.sessionID ?? null)
				} catch { resolve(null) }
			})
		})
		req.setTimeout(500, () => { req.destroy(); resolve(null) })
		req.on('error', () => resolve(null))
		req.end()
	})
}

// ---------------------------------------------------------------------------
// data-v-inspector injection
//
// Uses vueParse to get the AST, then a plain recursive walk to collect element
// positions. Does NOT call vueTransform — that is the full compilation pipeline
// and hangs on certain templates. We only need source locations, not IR nodes.
// ---------------------------------------------------------------------------

const SKIP_TAGS = new Set(['template', 'script', 'style'])

interface Insertion { offset: number; text: string }

function walkNode(node: ReturnType<typeof vueParse>['children'][number], insertions: Insertion[], relativePath: string): void {
	if ((node as { type: number }).type !== 1) return

	const el = node as {
		type: 1; tagType: number; tag: string
		loc: { source: string; start: { line: number; column: number; offset: number } }
		props: { loc: { end: { offset: number } } }[]
		children: typeof node[]
	}

	if (
		(el.tagType === 0 || el.tagType === 1) &&
		!SKIP_TAGS.has(el.tag) &&
		!el.loc.source.includes('data-v-inspector')
	) {
		const insertAt = el.props.length > 0
			? el.props.reduce((max, p) => Math.max(max, p.loc.end.offset), 0)
			: el.loc.start.offset + el.tag.length + 1

		insertions.push({
			offset: insertAt,
			text: ` data-v-inspector="${relativePath}:${el.loc.start.line}:${el.loc.start.column}"`,
		})
	}

	for (const child of el.children ?? []) {
		walkNode(child, insertions, relativePath)
	}
}

function injectDataVInspector(code: string, filename: string): { code: string } | null {
	const relativePath = relative(process.cwd(), filename).replace(/\\/g, '/')
	const insertions: Insertion[] = []

	try {
		const ast = vueParse(code, { comments: true })
		for (const child of ast.children) {
			walkNode(child, insertions, relativePath)
		}
	} catch {
		return null
	}

	if (insertions.length === 0) return null

	insertions.sort((a, b) => b.offset - a.offset)
	let result = code
	for (const { offset, text } of insertions) {
		result = result.slice(0, offset) + text + result.slice(offset)
	}

	return { code: result }
}

// ---------------------------------------------------------------------------
// Vite plugin: data-v-inspector transform
// ---------------------------------------------------------------------------

function dataVInspectorTransformPlugin(): Plugin {
	return {
		name: 'amberite-data-v-inspector',
		enforce: 'pre',
		transform(code: string, id: string) {
			if (!id.includes('type=template')) return null
			const filename = id.split('?')[0]
			return injectDataVInspector(code, filename)
		},
	}
}

// ---------------------------------------------------------------------------
// Vite plugin: relay
// ---------------------------------------------------------------------------

function relayPlugin(): Plugin {
	return {
		name: 'amberite-opencurser-relay',
		configureServer(server: ViteDevServer) {
			server.middlewares.use('/__design-relay', async (req, res) => {
				const lock = readLockFile()
				const target = lock?.url ?? 'http://localhost:4096'
				const auth = lock?.auth ?? ''

				const destUrl = new URL(req.url || '/', target)
				const headers: Record<string, string | string[]> = {
					...(req.headers as Record<string, string | string[]>),
					host: destUrl.host,
				}
				delete headers['connection']
				if (auth) headers['authorization'] = auth

				if (req.url === '/design-comments' && req.method === 'POST') {
					const sessionID = await fetchActiveSessionId(target, auth)
					if (sessionID) headers['x-opencode-session-id'] = sessionID
				}

				const proxyReq = http.request(destUrl, { method: req.method, headers }, (proxyRes) => {
					const resHeaders = { ...proxyRes.headers }
					delete resHeaders['transfer-encoding']
					res.writeHead(proxyRes.statusCode ?? 200, resHeaders)
					proxyRes.pipe(res, { end: true })
				})

				proxyReq.setTimeout(5000, () => {
					proxyReq.destroy()
					if (!res.headersSent) res.writeHead(504, { 'content-type': 'text/plain' })
					res.end('OpenCurser timeout')
				})

				proxyReq.on('error', () => {
					if (!res.headersSent) res.writeHead(502, { 'content-type': 'text/plain' })
					res.end('OpenCurser unreachable')
				})

				req.pipe(proxyReq, { end: true })
			})
		},
	}
}

// ---------------------------------------------------------------------------
// Public export
// ---------------------------------------------------------------------------

export function designInspectorVitePlugin(): Plugin[] {
	return [
		relayPlugin(),
		dataVInspectorTransformPlugin(),
	]
}
