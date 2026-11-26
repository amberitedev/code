#!/usr/bin/env node

// `vp run scenarios:prove <scenario> [--base <ref>] [--upload-command <command>]`: runs one scenario on this checkout as it is
// and on the commit it branched from, and says whether it fails there and passes here.
// See docs/internals/fixtures.md#scenarios.
//
// Exits 0 when proven, 1 when not, 2 when the App did not run, and 3 when the scenario stopped
// before its assertion.

import * as NodeChildProcess from 'node:child_process'
import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import * as NodeURL from 'node:url'
import * as NodeUtil from 'node:util'

/** The parts of e2e's `report.json` (its `schema/report-v1.schema.json`) a proof reads. */
interface Report {
	readonly run: {
		readonly results: ReadonlyArray<{
			readonly titlePath: ReadonlyArray<string>
			readonly attempts: ReadonlyArray<{
				readonly status?: string
				readonly error?: { readonly code: string; readonly message: string }
				readonly artifacts?: ReadonlyArray<{ readonly kind: string; readonly path?: string }>
			}>
		}>
	}
}

export interface Side {
	/** `error`: the scenario could not run, which says nothing about the behavior. */
	readonly status: 'passed' | 'failed' | 'error'
	readonly title: string
	readonly message?: string
	/** For `error`: the App did not run, or it ran and the scenario stopped before its assertion. */
	readonly stopped?: 'app' | 'scenario'
	/** Screenshot paths, in the order the scenario took them. */
	readonly captures: ReadonlyArray<string>
}

/** Only a failed expectation shows the bug. A control that is missing or an App that is down shows nothing. */
export function readSide(resultsDir: string, report: Report | undefined): Side {
	// The report also lists the tests the run did not select, with no attempts.
	const ran = report?.run.results.filter((result) => result.attempts.length > 0) ?? []
	const attempt = ran.length === 1 ? ran[0]!.attempts.at(-1) : undefined
	if (!attempt) {
		// With several tests, a pass in one could hide a failure in another.
		const message =
			ran.length > 1
				? `the scenario file ran ${ran.length} tests; a proof needs exactly one`
				: 'e2e wrote no report: the stack did not start, or no test was selected'
		return { status: 'error', stopped: 'app', title: '', message, captures: [] }
	}
	const { error } = attempt
	const status =
		attempt.status === 'passed' ? 'passed' : error?.code === 'ASSERTION_FAILED' ? 'failed' : 'error'
	return {
		status,
		...(status === 'error' ? { stopped: error ? ('scenario' as const) : ('app' as const) } : {}),
		title: ran[0]!.titlePath.join(' > '),
		...(error
			? { message: `${error.code}: ${error.message}` }
			: status === 'passed'
				? {}
				: { message: `the test ended ${attempt.status ?? 'without a status'}` }),
		captures: (attempt.artifacts ?? []).flatMap((artifact) =>
			artifact.kind === 'screenshot' && artifact.path
				? [NodePath.join(resultsDir, artifact.path)]
				: [],
		),
	}
}

/** What the two runs prove, and the exit code that says it. `base` is absent when head did not pass. */
export function verdict(head: Side, base: Side | undefined): { text: string; exit: number } {
	const broken = head.status === 'error' ? head : base?.status === 'error' ? base : undefined
	if (broken) {
		const where = broken === head ? 'this checkout' : 'base'
		return {
			text: `Not proven: the scenario could not run on ${where} (${broken.message ?? 'unknown error'}).`,
			exit: broken.stopped === 'scenario' ? 3 : 2,
		}
	}
	if (head.status === 'failed' || !base) {
		return { text: 'Not proven: the scenario fails on this checkout.', exit: 1 }
	}
	if (base.status === 'passed') {
		return {
			text: 'Not proven: the scenario passes on base too, so it does not reproduce the bug.',
			exit: 1,
		}
	}
	return { text: 'Proven: the scenario fails on base and passes on this checkout.', exit: 0 }
}

/**
 * Runs `command` with the captures as arguments and reads one URL a line, in their order. Empty
 * when it fails: two finished runs are worth more than their links, so the proof names local files.
 */
export function upload(command: string, captures: ReadonlyArray<string>): Map<string, string> {
	if (captures.length === 0) return new Map()
	// A shell, so a command installed as a `.cmd` shim runs on Windows.
	const quoted = captures.map((capture) => `"${capture}"`).join(' ')
	const result = NodeChildProcess.spawnSync(`${command} ${quoted}`, {
		shell: true,
		encoding: 'utf8',
		windowsHide: true,
	})
	const urls = result.status === 0 ? result.stdout.trim().split(/\r?\n/) : []
	// Lines pair with captures by position, so another count would mislabel every image.
	if (urls.length !== captures.length) {
		console.error(`Upload failed, linking local captures: ${result.stderr || result.stdout}`)
		return new Map()
	}
	return new Map(captures.map((capture, index) => [capture, urls[index]!]))
}

function git(args: ReadonlyArray<string>, cwd: string): string {
	return NodeChildProcess.execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function node(args: ReadonlyArray<string>, cwd: string): number {
	return (
		NodeChildProcess.spawnSync(process.execPath, args, { cwd, stdio: 'inherit', windowsHide: true })
			.status ?? 1
	)
}

async function main(): Promise<void> {
	const { values, positionals } = NodeUtil.parseArgs({
		allowPositionals: true,
		options: {
			base: { type: 'string', default: 'origin/main' },
			'upload-command': { type: 'string' },
		},
	})
	const scenario = positionals[0]
	const baseRef = values.base
	if (!scenario || positionals.length > 1) {
		throw new Error(
			'Usage: vp run scenarios:prove <scenario file> [--base <ref>] [--upload-command <command>]',
		)
	}
	const head = git(['rev-parse', '--show-toplevel'], process.cwd())
	const scripts = NodePath.dirname(NodeURL.fileURLToPath(import.meta.url))
	const fixtures = NodePath.join(head, 'packages', 'fixtures')
	const out = NodePath.join(fixtures, '.e2e', 'prove')
	// A proof left by an earlier run must not be read as this one's.
	NodeFS.rmSync(out, { recursive: true, force: true })

	/** Runs the scenario on the stack of `checkout`. The scenario and e2e are always this checkout's. */
	const runSide = (side: 'head' | 'base', checkout: string): Side => {
		const output = NodePath.join('.e2e', 'prove', side)
		node(
			[
				NodePath.join(scripts, 'scenarios.ts'),
				NodePath.relative(fixtures, NodePath.resolve(scenario)),
				'--output',
				output,
			],
			checkout,
		)
		const report = NodePath.join(fixtures, output, 'report.json')
		return readSide(
			NodePath.join(fixtures, output, 'results'),
			NodeFS.existsSync(report)
				? (JSON.parse(NodeFS.readFileSync(report, 'utf8')) as Report)
				: undefined,
		)
	}

	const headSide = runSide('head', head)
	let baseSide: Side | undefined
	// A scenario that does not pass here proves nothing, whatever base does.
	if (headSide.status === 'passed') {
		const sha = git(['merge-base', 'HEAD', baseRef], head)
		// Beside this checkout, so its stack gets ports and data of its own.
		const worktree = `${head}-scenario-${sha.slice(0, 10)}`
		const discard = () => {
			if (!NodeFS.existsSync(worktree)) return
			node([NodePath.join(scripts, 'dev-runner.ts'), 'down'], worktree)
			git(['worktree', 'remove', '--force', worktree], head)
		}
		// A run that was killed left its worktree behind, and git refuses to add over it.
		discard()
		git(['worktree', 'add', '--quiet', '--detach', worktree, sha], head)
		try {
			if (node([NodePath.join(worktree, 'scripts', 'setup-worktree.ts')], worktree) !== 0) {
				throw new Error(`Could not set up the base checkout at ${worktree}.`)
			}
			baseSide = runSide('base', worktree)
		} finally {
			discard()
		}
	}

	const result = verdict(headSide, baseSide)
	const uploadCommand = values['upload-command']
	const links = uploadCommand
		? upload(uploadCommand, [...(baseSide?.captures ?? []), ...headSide.captures])
		: new Map<string, string>()
	const row = (name: string, side: Side | undefined) => {
		if (!side) return `| ${name} | not run | |`
		const message = side.message?.replaceAll('\n', ' ').replaceAll('|', '\\|')
		// Uploaded, or relative to the proof, so its Markdown shows them wherever the folder is opened.
		const captures = side.captures.map(
			(capture) =>
				`![${name}](${links.get(capture) ?? NodePath.relative(out, capture).split(NodePath.sep).join('/')})`,
		)
		return `| ${name} | ${side.status}${message ? `: ${message}` : ''} | ${captures.join(' ')} |`
	}
	const proof = [
		`### Scenario: ${headSide.title || baseSide?.title || scenario}`,
		'',
		`**${result.text}**`,
		'',
		'| | Result | Captures |',
		'| --- | --- | --- |',
		row('Base', baseSide),
		row('This checkout', headSide),
		'',
	].join('\n')
	NodeFS.mkdirSync(out, { recursive: true })
	NodeFS.writeFileSync(NodePath.join(out, 'proof.md'), proof)
	console.log(`\n${proof}`)
	process.exitCode = result.exit
}

if (
	process.argv[1] &&
	NodePath.resolve(process.argv[1]) === NodeURL.fileURLToPath(import.meta.url)
) {
	await main()
}
