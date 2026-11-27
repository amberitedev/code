import { describe, expect, it } from 'vite-plus/test'
import { parseCargoTimings, summarizeCargoTimings } from './timings.ts'

// The parts of a Cargo 1.95 `--timings` report the parser reads.
const REPORT = `<script>
const UNIT_DATA = [
  {"i": 0, "name": "serde", "target": "", "start": 0.0, "duration": 2.5, "sections": null},
  {"i": 1, "name": "engine", "target": "", "start": 2.5, "duration": 80.0,
   "sections": [["frontend", {"start": 0.0, "end": 60.0}], ["codegen", {"start": 60.0, "end": 80.0}]]},
  {"i": 2, "name": "engine", "target": " build-script (run)", "start": 1.0, "duration": 0.4, "sections": null},
  {"i": 3, "name": "copal", "target": " copal \\"bin\\"", "start": 82.5, "duration": 3.0, "sections": null},
  {"i": 4, "name": "copal", "target": "", "start": 0.0, "duration": 0.0, "sections": null}
];
const CPU_USAGE = [[1.0, 40.0], [2.0, 80.0]];
const X_LINE = 50;
</script>`

describe('cargo timings', () => {
	it('splits repo crates by unit from third-party ones and skips fresh units', () => {
		const timings = parseCargoTimings(REPORT)
		expect(timings).toBeDefined()
		expect(summarizeCargoTimings([timings!], new Set(['engine', 'copal']))).toEqual({
			units: {
				engine: { seconds: 80, phases: { frontend: 60, codegen: 20 } },
				'engine build-script (run)': { seconds: 0.4, phases: {} },
				'copal "bin"': { seconds: 3, phases: {} },
			},
			thirdParty: { units: 1, seconds: 2.5 },
			cpuPercent: 60,
		})
	})

	it('reads nothing from a report in another format', () => {
		expect(parseCargoTimings('<html></html>')).toBeUndefined()
	})
})
