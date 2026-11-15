import * as NodeHttp from 'node:http'
import type { AddressInfo } from 'node:net'
import { describe, expect, it } from 'vite-plus/test'

import { restartIfPatched } from './call.ts'
import { begin, state } from './context.ts'

describe('restarting a patched program', () => {
	it('is asked for once by calls made together, and fails only with its own restart', async () => {
		const answering = NodeHttp.createServer((_, response) => response.end('ok')).listen(0)
		const sent: string[] = []
		const requests: Record<string, string> = {}
		// Left from before any request here: only what follows its own request counts.
		const programs = { backend: 'failed' }
		const backend = `http://127.0.0.1:${(answering.address() as AddressInfo).port}`
		begin({
			worktree: '',
			primary: '',
			data: '',
			send: (command) => sent.push(command),
			runtime: () => ({
				programs,
				requests,
				apps: [],
				cores: [],
				urls: { backend },
			}),
		})
		/** The runner finishing the latest request, leaving the program in `state`. */
		const answer = (state: string) => {
			programs.backend = state
			requests[sent.at(-1)!.split('#')[1]!] = state
		}
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

			// Started, then gone before it ever answered: the wait ends with that failure.
			await new Promise((resolve) => answering.close(resolve))
			state.dirty.add('backend')
			const exits = restartIfPatched('backend')
			await expect.poll(() => sent.length).toBe(3)
			answer('running')
			programs.backend = 'failed'
			await expect(exits).rejects.toThrow('did not start again')
		} finally {
			answering.close()
		}
	})
})
