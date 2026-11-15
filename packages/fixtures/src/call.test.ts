import * as NodeHttp from 'node:http'
import type { AddressInfo } from 'node:net'
import { describe, expect, it } from 'vite-plus/test'

import { restartIfPatched } from './call.ts'
import { begin, state } from './context.ts'

describe('restarting a patched program', () => {
	it('is asked for once by calls made together, and again after it failed', async () => {
		const answering = NodeHttp.createServer((_, response) => response.end('ok')).listen(0)
		const sent: string[] = []
		const requests: Record<string, string> = {}
		begin({
			worktree: '',
			primary: '',
			data: '',
			send: (command) => sent.push(command),
			runtime: () => ({
				// Left from before any request here: only an answer to its own request counts.
				programs: { backend: 'failed' },
				requests,
				apps: [],
				cores: [],
				urls: { backend: `http://127.0.0.1:${(answering.address() as AddressInfo).port}` },
			}),
		})
		const answer = (state: string) => (requests[sent.at(-1)!.split('#')[1]!] = state)
		try {
			state.dirty.add('backend')
			const together = Promise.allSettled([
				restartIfPatched('backend'),
				restartIfPatched('backend'),
			])
			await expect.poll(() => sent.length).toBe(1)
			answer('failed')
			// Both waited for the one restart, so both learn it failed.
			expect((await together).map((call) => call.status)).toEqual(['rejected', 'rejected'])

			// The failed restart left the program marked, so the next call asks again.
			const again = restartIfPatched('backend')
			await expect.poll(() => sent.length).toBe(2)
			answer('running')
			await again
			expect(state.dirty.has('backend')).toBe(false)
			// Nothing is patched now, so nothing more is asked.
			await restartIfPatched('backend')
			expect(sent).toHaveLength(2)
		} finally {
			answering.close()
		}
	})
})
