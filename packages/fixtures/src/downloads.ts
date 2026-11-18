// Game files (Minecraft, Java, loaders, server jars) are downloaded once per machine. Every App
// and Core keeps its own `meta` folder; a cache beside the shared Rust builds fills a new one with
// hard links, so nothing is copied and no two programs ever write the same folder.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

type Kind = 'app' | 'core'

/** Per-install state the engine keeps in `meta`, which no other install should see. */
const NOT_SHARED = new Set(['install_job_backups'])

function cache(primary: string, kind: Kind): string {
	return NodePath.join(primary, 'target', 'dev-shared', 'game-files', kind)
}

/**
 * Whether a path in `meta`, with `/` separators, is shared: not a download still being written,
 * not per-install state, and not a version profile the engine merged, which depends on its code.
 */
function shared(path: string): boolean {
	const parts = path.split('/')
	const name = parts.at(-1) ?? ''
	// The engine writes a download under a temporary dot name and renames it when it is whole.
	if (name.startsWith('.')) return false
	if (parts.length === 1 && NOT_SHARED.has(name)) return false
	return !(parts.length === 3 && parts[0] === 'versions' && name === `${parts[1]}.json`)
}

/** How many shared files a folder holds. Listing is far cheaper than asking about each file. */
function count(folder: string, path = ''): number {
	if (!NodeFS.existsSync(folder)) return 0
	let files = 0
	for (const entry of NodeFS.readdirSync(folder, { withFileTypes: true })) {
		const inner = path ? `${path}/${entry.name}` : entry.name
		if (!shared(inner)) continue
		if (entry.isDirectory()) files += count(NodePath.join(folder, entry.name), inner)
		else if (path) files += 1
	}
	return files
}

/**
 * Gives every file under `from` that `to` lacks a second name there. Returns how many it linked.
 * A file is never overwritten, so a program's own copy always wins.
 */
function linkMissing(from: string, to: string, path = ''): number {
	if (!NodeFS.existsSync(from)) return 0
	let linked = 0
	for (const entry of NodeFS.readdirSync(from, { withFileTypes: true })) {
		const inner = path ? `${path}/${entry.name}` : entry.name
		if (!shared(inner)) continue
		const source = NodePath.join(from, entry.name)
		const target = NodePath.join(to, entry.name)
		if (entry.isDirectory()) {
			linked += linkMissing(source, target, inner)
		} else if (entry.isFile() && path && !NodeFS.existsSync(target)) {
			NodeFS.mkdirSync(to, { recursive: true })
			try {
				NodeFS.linkSync(source, target)
			} catch {
				// Another drive, or a file system without hard links. Never over a file another
				// checkout put there in the meantime.
				try {
					NodeFS.copyFileSync(source, target, NodeFS.constants.COPYFILE_EXCL)
				} catch (error) {
					if ((error as NodeJS.ErrnoException).code === 'EEXIST') continue
					throw error
				}
			}
			linked += 1
		}
	}
	return linked
}

/**
 * Links what `to` lacks from `from`, unless `from` holds as many files as when this last ran:
 * files are only ever added, so an equal count means there is nothing new. `record` keeps the
 * count.
 */
function sync(from: string, to: string, record: string): number {
	const files = count(from)
	if (files === 0) return 0
	if (NodeFS.existsSync(record) && NodeFS.readFileSync(record, 'utf8') === String(files)) return 0
	const linked = linkMissing(from, to)
	NodeFS.mkdirSync(NodePath.dirname(record), { recursive: true })
	NodeFS.writeFileSync(record, String(files))
	return linked
}

/** Removes the version profiles a program's engine merged, so the current code merges them again. */
export function forgetMergedProfiles(programDir: string): void {
	const versions = NodePath.join(programDir, 'meta', 'versions')
	if (!NodeFS.existsSync(versions)) return
	for (const id of NodeFS.readdirSync(versions)) {
		NodeFS.rmSync(NodePath.join(versions, id, `${id}.json`), { force: true })
	}
}

/** Fills a program's game files from what any checkout downloaded before. */
export function fillGameFiles(primary: string, kind: Kind, programDir: string): number {
	const meta = NodePath.join(programDir, 'meta')
	return sync(cache(primary, kind), meta, NodePath.join(meta, '.filled'))
}

/** Keeps what a program downloaded for the next one. Call it only once the program is idle. */
export function keepGameFiles(primary: string, kind: Kind, programDir: string): number {
	const meta = NodePath.join(programDir, 'meta')
	return sync(meta, cache(primary, kind), NodePath.join(meta, '.kept'))
}
