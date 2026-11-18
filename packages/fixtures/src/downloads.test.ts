import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'
import { describe, expect, it } from 'vite-plus/test'

import { fillGameFiles, forgetMergedProfiles, keepGameFiles } from './downloads.ts'

describe('game files', () => {
	it('shares downloads between programs without sharing their folders', () => {
		const scratch = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'fixtures-'))
		const write = (root: string, path: string, text: string) => {
			const file = NodePath.join(root, 'meta', path)
			NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
			NodeFS.writeFileSync(file, text)
		}
		const read = (root: string, path: string) => {
			const file = NodePath.join(root, 'meta', path)
			return NodeFS.existsSync(file) ? NodeFS.readFileSync(file, 'utf8') : undefined
		}
		const [first, second] = ['first', 'second'].map((name) => NodePath.join(scratch, name))
		try {
			write(first!, 'versions/1.21.1/client.jar', 'jar')
			write(first!, 'java_versions/21/bin/java', 'java')
			// Per-install state, a download still being written, and the engine's own loose files.
			write(first!, 'install_job_backups/job/instance.json', 'backup')
			write(first!, 'versions/1.21.1/.tmpAb12', 'partial')
			write(first!, 'launcher.json', 'state')
			// The version profile the engine merged, which its code decides.
			write(first!, 'versions/1.21.1-21.1.77/1.21.1-21.1.77.json', 'merged')
			expect(keepGameFiles(scratch, 'app', first!)).toBe(2)

			write(second!, 'versions/1.21.1/client.jar', 'its own')
			expect(fillGameFiles(scratch, 'app', second!)).toBe(1)
			expect(read(second!, 'java_versions/21/bin/java')).toBe('java')
			// A file the program already has is never replaced.
			expect(read(second!, 'versions/1.21.1/client.jar')).toBe('its own')
			expect(read(second!, 'install_job_backups/job/instance.json')).toBeUndefined()
			expect(read(second!, 'versions/1.21.1/.tmpAb12')).toBeUndefined()
			expect(read(second!, 'launcher.json')).toBeUndefined()
			expect(read(second!, 'versions/1.21.1-21.1.77/1.21.1-21.1.77.json')).toBeUndefined()
			// A program's own merged profile goes on the next apply; its downloads stay.
			forgetMergedProfiles(first!)
			expect(read(first!, 'versions/1.21.1-21.1.77/1.21.1-21.1.77.json')).toBeUndefined()
			expect(read(first!, 'versions/1.21.1/client.jar')).toBe('jar')

			// Installing Java deletes its folder and unpacks again: the other program keeps its copy.
			NodeFS.rmSync(NodePath.join(second!, 'meta', 'java_versions'), { recursive: true })
			expect(read(first!, 'java_versions/21/bin/java')).toBe('java')
			// An App's downloads never reach a Core, which runs other files.
			expect(fillGameFiles(scratch, 'core', NodePath.join(scratch, 'core'))).toBe(0)
		} finally {
			NodeFS.rmSync(scratch, { force: true, recursive: true })
		}
	})
})
