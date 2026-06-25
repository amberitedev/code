import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = readFileSync(new URL('./index.ts', import.meta.url), 'utf8')

describe('realtime Worker protocol surface', () => {
	it('keeps the Worker scoped to desktop user presence', () => {
		expect(source).toContain('/v1/desktop-sessions')
		expect(source).toContain('/v1/connect')
		expect(source).toContain('/v1/invalidate')
		expect(source).not.toContain('core-sessions')
		expect(source).not.toContain('coreScope')
		expect(source).not.toContain('presence.core')
		expect(source).not.toContain('core.health')
	})
})
