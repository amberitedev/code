import type { Archon } from '@modrinth/api-client'

import type { ContentDiffItem } from '#ui/layouts/shared/installation-settings/types'

/**
 * A server share's push diff as the content diff list shows it. Core gives it in the App's diff
 * shape (`sharing_client::diff`), so this maps it as the App maps its own.
 */
export function resolveServerShareDiff(
	diff: Archon.Content.v1.SharedInstancePublishDiff,
): ContentDiffItem[] {
	return diff.diffs.map((entry) => ({
		type: entry.type,
		projectName: entry.projectName ?? undefined,
		// The diff list decodes file names.
		fileName: entry.fileName ? encodeURIComponent(entry.fileName) : undefined,
		currentVersionName: entry.currentVersionName ?? undefined,
		newVersionName: entry.newVersionName ?? undefined,
		fileCount: entry.configFileCount ?? undefined,
		fileNames: entry.configFileNames ?? undefined,
		disabled: entry.disabled,
		external: entry.type === 'added' && !entry.projectId && !!entry.fileName,
	}))
}
