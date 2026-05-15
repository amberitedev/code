import http from 'node:http'
import { join, relative } from 'node:path'
import { parse as vueParse } from '@vue/compiler-dom'
import type { Plugin, ViteDevServer } from 'vite'

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
// Vite plugin: T3 Code relay
//
// All design-inspector traffic goes through this proxy at /__design-relay.
// Targets the T3 Code dev server — port from T3CODE_PORT env var (default 3773).
// In dev:desktop mode T3 Code runs on 13773; set T3CODE_PORT=13773 accordingly.
// Path rewrite: /design-comments -> /api/design-comments
//
// The port is read lazily (per-request) so that env vars loaded after this
// module is imported (e.g. from packages/app-lib/.env in vite.config.ts) are
// picked up correctly.
// ---------------------------------------------------------------------------

function relayPlugin(): Plugin {
	return {
		name: 'amberite-t3code-relay',
		configureServer(server: ViteDevServer) {
			server.middlewares.use('/__design-relay', (req, res) => {
				const port = process.env['T3CODE_PORT'] ?? '3773'
				const target = `http://localhost:${port}`
				const rawPath = req.url || '/'
				const needsApiPrefix = rawPath.startsWith('/design-comments')
				|| rawPath.startsWith('/components/autocomplete')
				|| rawPath.startsWith('/files/autocomplete')
			const destPath = needsApiPrefix ? '/api' + rawPath : rawPath
				const destUrl = new URL(destPath, target)

				const headers: Record<string, string | string[]> = {
					...(req.headers as Record<string, string | string[]>),
					host: destUrl.host,
				}
				delete headers['connection']

				const proxyReq = http.request(destUrl, { method: req.method, headers }, (proxyRes) => {
					const resHeaders = { ...proxyRes.headers }
					delete resHeaders['transfer-encoding']
					res.writeHead(proxyRes.statusCode ?? 200, resHeaders)
					proxyRes.pipe(res, { end: true })
				})

				proxyReq.setTimeout(5000, () => {
					proxyReq.destroy()
					if (!res.headersSent) res.writeHead(504, { 'content-type': 'text/plain' })
					res.end('T3 Code timeout')
				})

				proxyReq.on('error', () => {
					if (!res.headersSent) res.writeHead(502, { 'content-type': 'text/plain' })
					res.end('T3 Code unreachable')
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
	if (process.env.VITE_ENABLE_DESIGN_INSPECTOR !== 'true') return []

	return [
		relayPlugin(),
		dataVInspectorTransformPlugin(),
	]
}
