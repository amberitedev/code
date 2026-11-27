#!/usr/bin/env node

// `vp run scenarios:prove <scenario> [options]`: runs one scenario on this checkout as it is and on
// the commit it branched from, says whether it fails there and passes here, and can post that on a
// pull request.
// See docs/internals/testing.md#scenarios.
//
// Exits 0 when proven, 1 when not, 2 when the scenario could not run (the App did not run, or the
// command was used wrongly), and 3 when the scenario cannot prove anything as written (it stopped
// before its assertion, or its file holds several tests).

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
	/** For `error`: the scenario could not run, or it cannot prove anything as it is written. */
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
		return {
			status: 'error',
			stopped: ran.length > 1 ? 'scenario' : 'app',
			title: '',
			message,
			captures: [],
		}
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
	const urls =
		result.status === 0
			? result.stdout
					.split(/\r?\n/)
					.map((line) => line.trim())
					.filter(Boolean)
			: []
	// Lines pair with captures by position, so another count would mislabel every image.
	if (urls.length !== captures.length) {
		console.error(`Upload failed, linking local captures: ${result.stderr || result.stdout}`)
		return new Map()
	}
	return new Map(captures.map((capture, index) => [capture, urls[index]!]))
}

/**
 * The proof as a pull request comment reads it: what was shown as its headline, one sentence on
 * what was tried, and each capture beside the one base took under the same name. A run that
 * proved nothing gets its verdict and what each side did, for whoever ran it.
 */
export function renderProof(input: {
	readonly title: string
	/** What was tried and what base did wrong, in a user's words. */
	readonly what?: string
	readonly result: { readonly text: string; readonly exit: number }
	readonly base: Side | undefined
	readonly head: Side
	readonly baseName: string
	/** Where a capture's Markdown points: an uploaded URL, or a path beside the proof. */
	readonly link: (capture: string) => string
}): string {
	const { base, head, baseName, link } = input
	const headline = `### ${input.title.charAt(0).toUpperCase()}${input.title.slice(1)}`
	if (input.result.exit !== 0) {
		const line = (name: string, side: Side | undefined) =>
			`- ${name}: ${side ? `${side.status}${side.message ? `, ${side.message.replaceAll(/\s+/g, ' ')}` : ''}` : 'not run'}`
		return [
			headline,
			'',
			`**${input.result.text}**`,
			'',
			line('Base', base),
			line('This checkout', head),
			'',
		].join('\n')
	}
	// Base also has the capture e2e takes when a test fails, which nothing here pairs with.
	const before = new Map(base?.captures.map((capture) => [NodePath.basename(capture), capture]))
	const rows = head.captures.map((capture) => {
		const pair = before.get(NodePath.basename(capture))
		return `| ${pair ? `![Before](${link(pair)})` : ''} | ![After](${link(capture)}) |`
	})
	return [
		headline,
		'',
		input.what ??
			`Ran this on \`${baseName}\` and on this branch. It fails on \`${baseName}\` and passes here.`,
		'',
		...(rows.length > 0 ? ['| Before | After |', '| --- | --- |', ...rows, ''] : []),
	].join('\n')
}

/**
 * Posts the proof on a pull request through `gh`, which uploads the captures the proof names by
 * path. An agent posts it from the developer's account, so the last line says who did.
 */
export function postComment(
	pullRequest: string,
	out: string,
	proof: string,
	by: string | undefined,
) {
	const gh = (args: ReadonlyArray<string>) =>
		NodeChildProcess.spawnSync('gh', args, { cwd: out, encoding: 'utf8', windowsHide: true })
	const login = gh(['api', 'user', '--jq', '.login']).stdout.trim()
	const body = `${proof}\n<sub>🤖 ${by ?? 'An agent'} on behalf of ${login || 'the developer'}, from \`vp run scenarios:prove\`</sub>\n`
	NodeFS.writeFileSync(NodePath.join(out, 'proof.md'), body)
	const captures = [...proof.matchAll(/\]\((\.\/[^)]+)\)/g)].map(([, path]) => path!)
	const posted = gh([
		'pr',
		'comment',
		pullRequest,
		'--body-file',
		'proof.md',
		...captures.flatMap((capture) => ['--attach', capture]),
	])
	if (posted.status === 0) console.log(`Posted: ${posted.stdout.trim()}`)
	else console.error(`Could not post the proof: ${posted.stderr.trim() || posted.stdout.trim()}`)
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
			what: { type: 'string' },
			comment: { type: 'string' },
			by: { type: 'string' },
		},
	})
	const scenario = positionals[0]
	const baseRef = values.base
	if (!scenario || positionals.length > 1) {
		throw new Error(
			'Usage: vp run scenarios:prove <scenario file> [--base <ref>] [--what <sentence>] [--comment <pull request>] [--by <model>] [--upload-command <command>]',
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
		let worktree: string | undefined
		const discard = () => {
			if (!worktree) return
			if (NodeFS.existsSync(worktree)) {
				node([NodePath.join(scripts, 'dev-runner.ts'), 'down'], worktree)
				// Not `git worktree remove`: on Windows it deletes what a folder junction points to,
				// and installed packages are links into the store every checkout shares.
				NodeFS.rmSync(worktree, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 })
			}
			git(['worktree', 'prune'], head)
		}
		try {
			const sha = git(['merge-base', 'HEAD', baseRef], head)
			// Beside this checkout, so its stack gets ports and data of its own.
			worktree = `${head}-scenario-${sha.slice(0, 10)}`
			// A run that was killed left its worktree behind, and git refuses to add over it.
			discard()
			git(['worktree', 'add', '--quiet', '--detach', worktree, sha], head)
			if (node([NodePath.join(worktree, 'scripts', 'setup-worktree.ts')], worktree) !== 0) {
				throw new Error(`could not set up the base checkout at ${worktree}`)
			}
			baseSide = runSide('base', worktree)
		} catch (error) {
			// No base to run on says nothing about the behavior, as when the App did not run there.
			// One line: the verdict quotes it, and git's errors span several.
			const message = (error instanceof Error ? error.message : String(error))
				.replaceAll(/\s+/g, ' ')
				.trim()
			baseSide = { status: 'error', stopped: 'app', title: '', message, captures: [] }
		} finally {
			// Two finished runs are worth more than a tidy folder. A run from the same base clears it.
			try {
				discard()
			} catch (error) {
				console.error(`Could not remove ${worktree}: ${String(error)}`)
			}
		}
	}

	const result = verdict(headSide, baseSide)
	const uploadCommand = values['upload-command']
	const links = uploadCommand
		? upload(uploadCommand, [...(baseSide?.captures ?? []), ...headSide.captures])
		: new Map<string, string>()
	// Uploaded, or relative to the proof, so its Markdown shows them wherever the folder is opened.
	const link = (capture: string) =>
		links.get(capture) ?? `./${NodePath.relative(out, capture).split(NodePath.sep).join('/')}`
	const proof = renderProof({
		title: headSide.title || baseSide?.title || scenario,
		what: values.what,
		result,
		base: baseSide,
		head: headSide,
		baseName: baseRef.replace(/^origin\//, ''),
		link,
	})
	NodeFS.mkdirSync(out, { recursive: true })
	NodeFS.writeFileSync(NodePath.join(out, 'proof.md'), proof)
	console.log(`\n${proof}`)
	// For whoever ran this, to see that base failed for the bug and not beside it.
	if (baseSide?.message) console.log(`Base: ${baseSide.message.replaceAll(/\s+/g, ' ')}`)
	if (values.comment) {
		if (result.exit === 0) postComment(values.comment, out, proof, values.by)
		else console.error('Not posted: there is no proof to show.')
	}
	process.exitCode = result.exit
}

if (import.meta.main) {
	// Not 1, which says the scenario ran on both sides and did not prove the fix.
	await main().catch((error: unknown) => {
		console.error(error instanceof Error ? error.message : String(error))
		process.exitCode = 2
	})
}
