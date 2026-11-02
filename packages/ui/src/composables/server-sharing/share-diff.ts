import type { Copal } from '@modrinth/api-client'

import type { ContentDiffItem } from '#ui/layouts/shared/installation-settings/types'

/**
 * A share diff as the content diff list shows it: the App's updates, versions and pushes, and a
 * Core server's push, all give it in `sharing_client::diff`'s shape. File names are URL-encoded
 * because the diff list decodes them.
 */
export function sharedDiffItems(
	diffs: readonly Copal.SharedInstanceUpdateDiff[],
): ContentDiffItem[] {
	return diffs.map((diff) => ({
		type: diff.type,
		projectName: diff.projectName ?? undefined,
		fileName: diff.fileName ? encodeURIComponent(diff.fileName) : undefined,
		currentVersionName: diff.currentVersionName ?? undefined,
		newVersionName: diff.newVersionName ?? undefined,
		fileCount: diff.configFileCount,
		fileNames: diff.configFileNames,
		disabled: diff.disabled,
		external: diff.type === 'added' && !diff.projectId && !!diff.fileName,
	}))
}
