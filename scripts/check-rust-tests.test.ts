import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodeOS from 'node:os'
import * as NodePath from 'node:path'

import { expect, it } from 'vitest'

// Each file as check:changed names it, untracked; the lines the script reports in it.
const FILES: Readonly<Record<string, readonly [source: string, lines: ReadonlyArray<number>]>> = {
	'tests/integration.rs': ['fn helper() {\n    std::thread::sleep(WAIT);\n}\n', [2]],
	'src/lib.rs': [
		[
			'#[cfg(all(test, feature = "export-ts"))]',
			'mod helpers;',
			'#[cfg(any(test, feature = "testing"))]',
			'fn shared() { std::thread::sleep(WAIT); }',
			'#[test]',
			'fn keyed(key: [u8; 32]) {',
			'    std::thread::sleep(WAIT);',
			'}',
			'#[cfg(all(test, unix))]',
			'fn waited(start: Instant) -> bool { Instant::now().checked_duration_since(start).is_some() }',
			'#[tokio::test(start_paused = true)]',
			'async fn paused() {',
			'    tokio::time::sleep(WAIT).await;',
			'    std::thread::sleep(WAIT);',
			'    let _ = start.saturating_duration_since(std::time::Instant::now());',
			'}',
			'// quarantine: #12 flakes on Windows',
			'#[test]',
			'#[ignore]',
			'fn quarantined() {}',
			'#[test]',
			'#[ignore]',
			'fn forgotten() {}',
			'',
		].join('\n'),
		[7, 10, 14, 15, 22],
	],
	'src/helpers.rs': ['pub fn settle() {\n    tokio::time::sleep(WAIT);\n}\n', [2]],
	'src/imported.rs': [
		[
			'use tokio::time::Instant;',
			'#[tokio::test(start_paused = true)]',
			'async fn tokio_clock() {',
			'    let start = Instant::now();',
			'    assert!(start.elapsed() > WAIT);',
			'}',
			'#[tokio::test(start_paused = true)]',
			'async fn std_clock() {',
			'    let start = std::time::Instant::now();',
			'    assert!(start.elapsed() > WAIT);',
			'}',
			'',
		].join('\n'),
		[10],
	],
	'src/full_path.rs': [
		[
			'#[tokio::test(start_paused = true)]',
			'async fn tokio_clock() {',
			'    let start = tokio::time::Instant::now();',
			'    assert!(start.elapsed() > WAIT);',
			'}',
			'',
		].join('\n'),
		[],
	],
}

it('reports sleeps, clock deltas and unmarked ignores in test code, and only there', () => {
	const folder = NodeFS.mkdtempSync(NodePath.join(NodeOS.tmpdir(), 'check-rust-tests-'))
	try {
		const files = Object.entries(FILES).map(([name, [source]]) => {
			const file = NodePath.join(folder, name)
			NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
			NodeFS.writeFileSync(file, source)
			return file.replaceAll('\\', '/')
		})
		const result = NodeChildProcess.spawnSync(
			process.execPath,
			[NodePath.join(import.meta.dirname, 'check-rust-tests.ts'), ...files],
			{ encoding: 'utf8' },
		)
		const reported = Object.keys(FILES).map((name) =>
			[...result.stderr.matchAll(/^(.+\.rs):(\d+): /gm)]
				.filter(([, file]) => file!.endsWith(`/${name}`))
				.map(([, , line]) => Number(line)),
		)
		expect(reported).toEqual(Object.values(FILES).map(([, lines]) => lines))
	} finally {
		NodeFS.rmSync(folder, { recursive: true, force: true })
	}
})
