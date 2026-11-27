#!/usr/bin/env node

// What docs/internals/testing.md asks of Rust tests, which Clippy cannot hold test code alone to:
// no sleeping to wait, no elapsed-time deltas, and a quarantine issue above every `#[ignore]`.
//
//   node scripts/check-rust-tests.ts              every tracked Rust file
//   node scripts/check-rust-tests.ts <file.rs>... only these
//
// Test code is a `tests/` folder, a module declared `#[cfg(test)] mod name;`, and any item marked
// `#[cfg(test)]`, `#[cfg(all(test, ...))]` or `#[test]`/`#[tokio::test]`. A test on tokio's paused
// clock (`start_paused = true`) may sleep and time itself with tokio's timers, which that clock
// drives, but not with `std`'s.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

const QUARANTINE = /^\s*\/\/ quarantine: #\d+ \S/
// `cfg(test)`, or `cfg(all(..., test, ...))`: compiled only into tests.
const TEST_CFG =
	/cfg\s*\(\s*(?:test|all\s*\((?:[^()]|\([^()]*\))*?(?<!not\s*\(\s*)\btest\b[^\]]*\))\s*\)/
const WAITS = [
	{
		pattern: /(?:\b[\w:]*::)?\b(?<!fn )sleep(?:_until)?\s*\(/g,
		message: 'sleeps; await the event instead',
		// A sleep is tokio's unless the thread's.
		tokio: (match: string, _test: string) => !/\bthread::/.test(match),
	},
	{
		pattern:
			/(?:\b[\w:]*::)?\bInstant::now\s*\(\s*\)\s*[-+]|\.elapsed\s*\(\s*\)|\b(?:checked_|saturating_)?duration_since\s*\(/g,
		message: 'measures elapsed time; assert on events and counts',
		// The test's `Instant` is tokio's when it names tokio's or the file imports it, and never
		// when the test reads `std`'s.
		tokio: (match: string, test: string) =>
			!/\bstd::time::/.test(match) &&
			(/\btokio::time::Instant\b/.test(match) ||
				(!/\bstd::time::Instant\b/.test(test) &&
					/\btokio::time::(?:\{[^}]*)?\bInstant\b/.test(test))),
	},
]

/**
 * The source with every comment, string and character literal blanked to spaces, keeping its
 * length and line breaks, so code can be searched and braces matched.
 */
function codeOnly(source: string): string {
	const out = source.split('')
	const blank = (from: number, to: number) => {
		for (let i = from; i < to; i++) if (out[i] !== '\n') out[i] = ' '
	}
	let i = 0
	while (i < source.length) {
		let end = i + 1
		if (source.startsWith('//', i)) {
			end = source.indexOf('\n', i)
			if (end < 0) end = source.length
		} else if (source.startsWith('/*', i)) {
			// Block comments nest.
			let depth = 0
			end = i
			do {
				if (source.startsWith('/*', end)) depth++
				else if (source.startsWith('*/', end)) depth--
				else {
					end++
					continue
				}
				end += 2
			} while (depth > 0 && end < source.length)
		} else if (/^b?r#*"/.test(source.slice(i, i + 260)) && !/\w/.test(source[i - 1] ?? '')) {
			const close = `"${/^b?r(#*)"/.exec(source.slice(i, i + 260))![1]}`
			const found = source.indexOf(close, source.indexOf('"', i) + 1)
			end = found < 0 ? source.length : found + close.length
		} else if (source[i] === '"') {
			end = i + 1
			while (end < source.length && source[end] !== '"') end += source[end] === '\\' ? 2 : 1
			end++
		} else if (source[i] === "'") {
			// A character literal closes within a few characters; a lifetime never does.
			const char = /^'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^\\'])'/.exec(
				source.slice(i, i + 12),
			)
			if (!char) {
				i++
				continue
			}
			end = i + char[0].length
		} else {
			i++
			continue
		}
		blank(i, end)
		i = end
	}
	return out.join('')
}

type Range = { readonly from: number; readonly to: number; readonly paused: boolean }

/** The items of `code` marked as test code, braces included, nested ones too. */
function testRanges(code: string): ReadonlyArray<Range> {
	const ranges: Range[] = []
	for (const match of code.matchAll(/#\[\s*(?:cfg\s*\((?:[^\]])*\)|(?:\w+::)*test\b[^\]]*)\]/g)) {
		if (match[0].includes('cfg') && !TEST_CFG.test(match[0])) continue
		// The item's body: its first `{` outside brackets, before a `;` that would end it bodiless.
		let open = match.index + match[0].length
		let nested = 0
		for (; open < code.length; open++) {
			const char = code[open]!
			if (char === '(' || char === '[') nested++
			else if (char === ')' || char === ']') nested--
			else if (nested === 0 && (char === '{' || char === ';')) break
		}
		if (code[open] !== '{') continue
		let depth = 0
		let close = open
		for (; close < code.length; close++) {
			if (code[close] === '{') depth++
			else if (code[close] === '}' && --depth === 0) break
		}
		const paused = /start_paused\s*=\s*true/.test(match[0])
		ranges.push({ from: match.index, to: close + 1, paused })
	}
	return ranges
}

/** What breaks the rules in one file, as `line: message`. `wholeFile` marks a test-only file. */
function problems(source: string, wholeFile: boolean): ReadonlyArray<string> {
	const code = codeOnly(source)
	const ranges = testRanges(code)
	// The file's import of tokio's `Instant`, which its paused tests read as their own.
	const tokioImport = /\buse\s+tokio::time::(?:\{[^}]*\bInstant\b|Instant\b)[^;]*;/.exec(code)?.[0]
	const enclosing = (index: number, paused: boolean) =>
		ranges.find((range) => index >= range.from && index < range.to && range.paused === paused)
	const within = (index: number, paused: boolean) => enclosing(index, paused) !== undefined
	const lineOf = (index: number) => code.slice(0, index).split('\n').length
	const found: Array<readonly [number, string]> = []
	for (const { pattern, message, tokio } of WAITS) {
		for (const match of code.matchAll(pattern)) {
			if (!wholeFile && !within(match.index, false) && !within(match.index, true)) continue
			const paused = enclosing(match.index, true)
			if (
				paused &&
				tokio(match[0], `${tokioImport ?? ''}\n${code.slice(paused.from, paused.to)}`)
			) {
				continue
			}
			found.push([lineOf(match.index), `\`${match[0].replace(/\s*\(.*$/s, '')}\` ${message}`])
		}
	}
	const lines = source.split('\n')
	for (const match of code.matchAll(/#\[\s*ignore\b/g)) {
		let above = lineOf(match.index) - 2
		while (above >= 0 && /^\s*#\[/.test(lines[above]!)) above--
		if (!QUARANTINE.test(lines[above] ?? '')) {
			found.push([lineOf(match.index), '`#[ignore]` needs `// quarantine: #1234 <why>` above it'])
		}
	}
	return found.sort(([a], [b]) => a - b).map(([line, message]) => `${line}: ${message}`)
}

/** Files that are test code throughout: integration tests and `#[cfg(test)] mod name;` modules. */
function testFiles(files: ReadonlyArray<string>): ReadonlySet<string> {
	const tests = new Set(files.filter((file) => /(?:^|\/)tests\//.test(file)))
	const declared = /#\[\s*(cfg\s*\((?:[^\]])*\))\s*\]\s*(?:pub\S*\s+)?mod\s+(\w+)\s*;/g
	for (const file of files) {
		const folder = /(?:^|\/)(?:lib|main|mod)\.rs$/.test(file)
			? NodePath.posix.dirname(file)
			: file.slice(0, -'.rs'.length)
		for (const [, cfg, name] of codeOnly(NodeFS.readFileSync(file, 'utf8')).matchAll(declared)) {
			if (!TEST_CFG.test(cfg!)) continue
			tests.add(`${folder}/${name}.rs`)
			tests.add(`${folder}/${name}/mod.rs`)
		}
	}
	return tests
}

const tracked = NodeChildProcess.execFileSync('git', ['ls-files', '-z', '--', '*.rs'], {
	encoding: 'utf8',
	maxBuffer: 64 * 1024 * 1024,
})
	.split('\0')
	.filter((file) => file && NodeFS.existsSync(file))
const named = process.argv.slice(2).map((file) => file.replaceAll('\\', '/'))
// Named files may be new and untracked; they can be tests, or declare test modules, too.
const tests = testFiles([...new Set([...tracked, ...named])])
let failed = false
for (const file of named.length > 0 ? named : tracked) {
	for (const problem of problems(NodeFS.readFileSync(file, 'utf8'), tests.has(file))) {
		console.error(`${file}:${problem}`)
		failed = true
	}
}
if (failed) {
	console.error('\nSee docs/internals/testing.md, Waiting for work.')
	process.exitCode = 1
}
