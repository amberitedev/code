// Path rules for scripts/upstream-sync.ts. Keys in upstream.json are upstream paths; `renamed` maps
// them to ours. `ours` holds our paths, bucketing works on upstream paths, deletion on ours.

export type UpstreamConfig = {
	// Upstream paths we never take. Deleted from every merge.
	ignore: string[]
	// Upstream path prefix to the prefix it lives under here.
	renamed: Record<string, string>
	// Upstream path to our file that ports it. Its commits are reported for porting, and a ported
	// path we do not have is deleted from every merge.
	ported: Record<string, string>
}

// Each new upstream commit lands in one bucket. See bucketOf for the order they are checked in.
export type Bucket = 'ignored' | 'port' | 'new' | 'overlap' | 'other'

// One line of `git show --name-status`. A rename lists the old path, then the new one.
export type Change = { status: string; paths: string[] }

export function parseNameStatus(text: string): Change[] {
	return lines(text).map((line) => {
		const [status = '', ...paths] = line.split('\t')
		return { status, paths }
	})
}

// The longest matching `renamed` prefix wins.
export function ourPath(path: string, renamed: UpstreamConfig['renamed']) {
	const [from, to] = Object.entries(renamed)
		.filter(([prefix]) => matches(path, [prefix]))
		.sort(([a], [b]) => b.length - a.length)[0] ?? [path, path]
	return to + path.slice(from.length)
}

// Our paths whose content changed since the merge base. A file we only moved (R100) is not one.
export function changedPaths(changes: Change[]) {
	return new Set(
		changes
			.filter((change) => change.status !== 'R100')
			.flatMap((change) => change.paths.slice(-1)),
	)
}

// Our paths to delete from a merge. `have` answers for our tree before the merge.
export function removedPaths(config: UpstreamConfig, have: (path: string) => boolean) {
	const ours = (path: string) => ourPath(path, config.renamed)
	return [
		...config.ignore.map(ours),
		...Object.keys(config.ported)
			.map(ours)
			.filter((path) => !have(path)),
	]
}

// Port comes first because ported labrinth paths sit inside the ignored labrinth folder.
export function bucketOf(
	changes: Change[],
	config: UpstreamConfig,
	ours: ReadonlySet<string>,
): Bucket {
	const paths = changes.flatMap((change) => change.paths)
	if (paths.some((path) => matches(path, Object.keys(config.ported)))) return 'port'
	if (paths.every((path) => matches(path, config.ignore))) return 'ignored'
	if (changes.some((change) => change.status === 'A' && isNewSurface(change.paths[0] ?? '')))
		return 'new'
	if (paths.some((path) => ours.has(ourPath(path, config.renamed)))) return 'overlap'
	return 'other'
}

// A new app or package manifest, a new `*-page.vue`, or a new file under a pages or routes folder.
function isNewSurface(path: string) {
	return (
		/^(apps|packages)\/[^/]+\/(package\.json|Cargo\.toml)$/.test(path) ||
		/(^|\/)(pages|routes)\//.test(path) ||
		path.endsWith('-page.vue')
	)
}

export function matches(path: string, prefixes: string[]) {
	return prefixes.some((prefix) => path === prefix || path.startsWith(`${prefix}/`))
}

export function lines(text: string) {
	return text.split('\n').filter(Boolean)
}
