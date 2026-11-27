// One line per setup, build, test run, start, and applied fixture, in the primary checkout's `.data/timings.jsonl`,
// shared by every checkout. It is how anyone checks whether a change to the dev setup made it faster.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

export type Timing = {
	readonly kind: 'setup' | 'build' | 'test' | 'dev' | 'fixture'
	readonly worktree: string
	readonly result: 'ok' | 'failed' | 'cancelled'
	/** Milliseconds per step, plus `total`. */
	readonly ms: Readonly<Record<string, number>>
	readonly [detail: string]: unknown
}

/** Appends `timing`. A log that cannot be written never fails the step it describes. */
export function recordTiming(primary: string, timing: Timing): void {
	try {
		const file = NodePath.join(primary, '.data', 'timings.jsonl')
		NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
		// One spelling per worktree, so lines group by it.
		const line = {
			at: new Date().toISOString(),
			...timing,
			worktree: NodePath.resolve(timing.worktree),
		}
		NodeFS.appendFileSync(file, `${JSON.stringify(line)}\n`)
	} catch {
		// Read-only or missing primary checkout.
	}
}

/** One compiled unit from a Cargo `--timings` report: a crate's library, binary, build script, or tests. */
export type CargoUnit = {
	readonly name: string
	/** Cargo's label past the name, such as `build-script (run)` or `"bin"`; empty for a library. */
	readonly target: string
	readonly seconds: number
	/** Seconds per compiler phase, such as `frontend` and `codegen`, when Cargo reports them. */
	readonly phases: Readonly<Record<string, number>>
}

export type CargoTimings = {
	readonly units: ReadonlyArray<CargoUnit>
	/** CPU use samples over the build, in percent. */
	readonly cpu: ReadonlyArray<number>
}

/**
 * The units a `--timings` build compiled, from the report Cargo leaves in `<targetDir>/cargo-timings`.
 * Removes the reports, which pile up one per build otherwise. `undefined` when there is no report
 * or Cargo changed its format, so a missing number never fails the build.
 */
export function takeCargoTimings(targetDir: string): CargoTimings | undefined {
	const folder = NodePath.join(targetDir, 'cargo-timings')
	try {
		return parseCargoTimings(
			NodeFS.readFileSync(NodePath.join(folder, 'cargo-timing.html'), 'utf8'),
		)
	} catch {
		return undefined
	} finally {
		try {
			NodeFS.rmSync(folder, { force: true, recursive: true })
		} catch {
			// Held open, as Windows does: the next build removes it.
		}
	}
}

/** Reads the `UNIT_DATA` and `CPU_USAGE` constants Cargo's timings report embeds as JSON. */
export function parseCargoTimings(html: string): CargoTimings | undefined {
	const constant = (name: string): unknown => {
		const json = new RegExp(String.raw`const ${name} = (\[[\s\S]*?\]);\n`).exec(html)?.[1]
		return json === undefined ? undefined : JSON.parse(json)
	}
	const unitData = constant('UNIT_DATA')
	if (!Array.isArray(unitData)) return undefined
	const units = unitData.flatMap((unit): CargoUnit[] => {
		if (!isRecord(unit) || typeof unit.name !== 'string' || typeof unit.duration !== 'number') {
			return []
		}
		// Cargo lists fresh units too, with no time.
		if (unit.duration <= 0) return []
		const phases: Record<string, number> = {}
		for (const section of Array.isArray(unit.sections) ? unit.sections : []) {
			const [phase, span] = Array.isArray(section) ? section : []
			if (typeof phase === 'string' && isRecord(span)) {
				const { start, end } = span
				if (typeof start === 'number' && typeof end === 'number') {
					phases[phase] = round(end - start)
				}
			}
		}
		// Cargo repeats the crate's name before a binary's or test's target (` copal "bin"`).
		const label = typeof unit.target === 'string' ? unit.target.trim() : ''
		const target = label.startsWith(`${unit.name} `) ? label.slice(unit.name.length + 1) : label
		return [{ name: unit.name, target, seconds: round(unit.duration), phases }]
	})
	const cpuData = constant('CPU_USAGE')
	const cpu = (Array.isArray(cpuData) ? cpuData : []).flatMap((sample) =>
		Array.isArray(sample) && typeof sample[1] === 'number' ? [sample[1]] : [],
	)
	return { units, cpu }
}

/**
 * Seconds per repo crate unit, keyed by name and target (`engine`, `engine build-script (run)`),
 * third-party units in one total, and the mean CPU use.
 */
export function summarizeCargoTimings(
	timings: ReadonlyArray<CargoTimings>,
	localCrates: ReadonlySet<string>,
) {
	const crates: Record<string, { seconds: number; phases: Record<string, number> }> = {}
	const thirdParty = { units: 0, seconds: 0 }
	const units = timings.flatMap((report) => report.units)
	for (const unit of units) {
		if (!localCrates.has(unit.name)) {
			thirdParty.units += 1
			thirdParty.seconds = round(thirdParty.seconds + unit.seconds)
			continue
		}
		const key = unit.target ? `${unit.name} ${unit.target}` : unit.name
		const entry = (crates[key] ??= { seconds: 0, phases: {} })
		entry.seconds = round(entry.seconds + unit.seconds)
		for (const [phase, seconds] of Object.entries(unit.phases)) {
			entry.phases[phase] = round((entry.phases[phase] ?? 0) + seconds)
		}
	}
	const cpu = timings.flatMap((report) => report.cpu)
	return {
		units: crates,
		thirdParty,
		...(cpu.length > 0
			? { cpuPercent: Math.round(cpu.reduce((sum, sample) => sum + sample, 0) / cpu.length) }
			: {}),
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null
}

function round(seconds: number): number {
	return Math.round(seconds * 100) / 100
}
