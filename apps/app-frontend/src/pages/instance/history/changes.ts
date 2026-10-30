import type { AvatarGroupItem } from '@modrinth/ui'

import type { SharedInstanceUpdateDiff } from '@/helpers/install'

export interface ChangedItem extends AvatarGroupItem {
	/** The version it moved to, the one it left, or both for an update. */
	version?: string
}

const ORDER = ['added', 'updated', 'removed'] as const

function nameOf(diff: SharedInstanceUpdateDiff) {
	return diff.projectName ?? (diff.fileName ? decodeURIComponent(diff.fileName) : '')
}

/** The projects and files a version changed, added first and removed last, as one group. */
export function changedItems(
	diffs: SharedInstanceUpdateDiff[],
	icons: Map<string, string | null | undefined>,
): ChangedItem[] {
	return ORDER.flatMap((kind) =>
		diffs.flatMap((diff) => {
			if (diff.type !== kind) return []
			const name = nameOf(diff)
			const version =
				kind === 'added'
					? diff.newVersionName
					: kind === 'removed'
						? diff.currentVersionName
						: [diff.currentVersionName, diff.newVersionName].filter(Boolean).join(' → ')
			return {
				key: diff.projectId ?? diff.fileName ?? name,
				name,
				src: diff.projectId ? icons.get(diff.projectId) : null,
				version: version || undefined,
				muted: kind === 'removed',
				highlighted: kind === 'updated',
			}
		}),
	)
}

/**
 * A version's name: what it changed, in a line, such as "Added Sodium and 7 more, removed Fabric
 * API". Config files are named only when nothing else changed, to keep it short.
 */
export function changeTitle(diffs: SharedInstanceUpdateDiff[], configFiles: number) {
	const game = changeOf(diffs, 'game_version_updated')
	const loader = changeOf(diffs, 'loader_updated')
	const clauses = [
		game?.to && `moved to Minecraft ${game.to}`,
		loader?.to && `switched to ${loader.to}`,
		...ORDER.map((kind) => {
			const names = diffs.filter((diff) => diff.type === kind).map(nameOf)
			return names.length > 0 && `${kind} ${listed(names)}`
		}),
	].filter(Boolean)
	if (clauses.length === 0 && configFiles > 0)
		clauses.push(`changed ${plural(configFiles, 'config file')}`)
	const title = clauses.join(', ') || 'no changes'
	return title[0].toUpperCase() + title.slice(1)
}

/** Up to two names, or the first and how many more. */
function listed(names: string[]) {
	if (names.length <= 2) return names.join(' and ')
	return `${names[0]} and ${names.length - 1} more`
}

function plural(count: number, noun: string) {
	return `${count} ${count === 1 ? noun : `${noun}s`}`
}

/** What a version moved from and to, for one of the changes that is not a mod. */
export function changeOf(
	diffs: SharedInstanceUpdateDiff[],
	type: 'game_version_updated' | 'loader_updated',
) {
	const diff = diffs.find((each) => each.type === type)
	return diff && { from: diff.currentVersionName, to: diff.newVersionName }
}
