import type { ContentDiffItem } from '@modrinth/ui'

import type { SharedInstanceUpdateDiff } from '@/helpers/install'

/**
 * A shared instance's changes as the content diff list shows them, for updates, versions and
 * pushes alike. File names are URL-encoded because ContentDiffList decodes them.
 */
export function sharedDiffItems(diffs: SharedInstanceUpdateDiff[]): ContentDiffItem[] {
	return diffs.map((diff) => ({
		type: diff.type,
		projectName: diff.projectName ?? undefined,
		fileName: diff.fileName ? encodeURIComponent(diff.fileName) : undefined,
		currentVersionName: diff.currentVersionName ?? undefined,
		newVersionName: diff.newVersionName ?? undefined,
		fileCount: diff.configFileCount ?? undefined,
		fileNames: diff.configFileNames ?? undefined,
		disabled: diff.disabled,
		external: diff.type === 'added' && !diff.projectId && !!diff.fileName,
	}))
}
