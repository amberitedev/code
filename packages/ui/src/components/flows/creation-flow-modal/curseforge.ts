import type { Kyros } from '@modrinth/api-client'
import { ref } from 'vue'

import { defineMessages } from '#ui/composables/i18n'
import { createContext } from '#ui/providers/create-context'

import type { CreationFlowContextValue, CurseForgePackFile } from './creation-flow-context'

export const curseforgeMessages = defineMessages({
	title: {
		id: 'creation-flow.curseforge.title',
		defaultMessage: 'Upload a CurseForge modpack',
	},
	description: {
		id: 'creation-flow.curseforge.description',
		defaultMessage: 'Use a modpack that isn’t available on Modrinth.',
	},
	linkLabel: {
		id: 'creation-flow.curseforge.link-label',
		defaultMessage: 'CurseForge modpack link',
	},
	linkDescription: {
		id: 'creation-flow.curseforge.link-description',
		defaultMessage: 'You can use a link to a modpack or a specific modpack version.',
	},
	invalidLink: {
		id: 'creation-flow.curseforge.invalid-link',
		defaultMessage: 'Enter a CurseForge Minecraft modpack or modpack version link.',
	},
	orUpload: {
		id: 'creation-flow.curseforge.or-upload',
		defaultMessage: 'Or upload it yourself',
	},
	fileLabel: {
		id: 'creation-flow.curseforge.file-label',
		defaultMessage: 'CurseForge Modpack file',
	},
	selectedFileLabel: {
		id: 'creation-flow.curseforge.selected-file-label',
		defaultMessage: 'Modpack file',
	},
	fileDescription: {
		id: 'creation-flow.curseforge.file-description',
		defaultMessage:
			'You’ll need the regular modpack file, and can optionally add a server pack after if we can’t find one.',
	},
	serverPackLabel: {
		id: 'creation-flow.curseforge.server-pack-label',
		defaultMessage: 'Server pack <optional>(optional)</optional>',
	},
	serverPackDescription: {
		id: 'creation-flow.curseforge.server-pack-description',
		defaultMessage:
			'You can optionally add a server pack to make it easier to determine what needs to be installed on the server.',
	},
	uploadPrompt: {
		id: 'creation-flow.curseforge.upload-prompt',
		defaultMessage: 'Drag and drop file or click to browse',
	},
	processingFile: {
		id: 'creation-flow.curseforge.processing-file',
		defaultMessage: 'Processing...',
	},
	removeFile: {
		id: 'creation-flow.curseforge.remove-file',
		defaultMessage: 'Remove {filename}',
	},
	notFound: {
		id: 'creation-flow.curseforge.not-found',
		defaultMessage: 'We couldn’t find this modpack on CurseForge.',
	},
	lookupFailed: {
		id: 'creation-flow.curseforge.lookup-failed',
		defaultMessage: 'We couldn’t reach CurseForge. Please try again.',
	},
	changeButton: {
		id: 'creation-flow.curseforge.change-button',
		defaultMessage: 'Change',
	},
	minecraftVersion: {
		id: 'creation-flow.curseforge.minecraft-version',
		defaultMessage: 'Minecraft {version}',
	},
	checkingServerPack: {
		id: 'creation-flow.curseforge.checking-server-pack',
		defaultMessage: 'Checking for a server pack...',
	},
	serverPackAvailable: {
		id: 'creation-flow.curseforge.server-pack-available',
		defaultMessage:
			'A server pack is available for this version. We’ll install {filename} on the server.',
	},
	serverPackClosest: {
		id: 'creation-flow.curseforge.server-pack-closest',
		defaultMessage:
			'A server pack is not available for this version. The closest version with a server pack is <link>{version}</link>.',
	},
	serverPackNone: {
		id: 'creation-flow.curseforge.server-pack-none',
		defaultMessage:
			'We could not find a server pack for any version of this project. We recommend choosing a different pack or uploading the server pack manually.',
	},
	ftbInstaller: {
		id: 'creation-flow.curseforge.ftb-installer',
		defaultMessage:
			'This version comes with FTB’s server installer, which we’ll use to set up the server.',
	},
})

/**
 * Normalizes any CurseForge modpack page link to its project or file link, dropping extra path
 * segments, queries, and fragments, such as those of the files list.
 *
 * @returns The canonical link, or `null` when the value isn't a CurseForge modpack link.
 */
export function normalizeCurseForgeModpackUrl(value: string): string | null {
	const trimmed = value.trim()
	let url: URL
	try {
		url = new URL(/^[a-z][a-z\d+.-]*:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`)
	} catch {
		return null
	}
	if (
		(url.protocol !== 'https:' && url.protocol !== 'http:') ||
		(url.hostname !== 'curseforge.com' && url.hostname !== 'www.curseforge.com') ||
		url.username ||
		url.password ||
		url.port
	) {
		return null
	}

	const segments = url.pathname.split('/').filter(Boolean)
	if (segments[0] === 'minecraft') segments.shift()
	const [section, slug, kind, fileId] = segments
	if (section !== 'modpacks' || !slug || !/^[\w-]+$/.test(slug)) return null

	const projectUrl = `https://www.curseforge.com/minecraft/modpacks/${slug}`
	return (kind === 'files' || kind === 'download') && fileId && /^\d+$/.test(fileId)
		? `${projectUrl}/files/${fileId}`
		: projectUrl
}

export function isCurseForgeModpackUrl(value: string): boolean {
	return normalizeCurseForgeModpackUrl(value) !== null
}

/** The CurseForge modpack picked from a link or uploaded packs, shared by the flows that pick one. */
export type CurseForgeSelection = Pick<
	CreationFlowContextValue,
	| 'curseforgeUrl'
	| 'curseforgeModpackFile'
	| 'curseforgeServerPackFile'
	| 'curseforgeModpack'
	| 'curseforgeFileId'
	| 'curseforgeFiles'
	| 'curseforgeProcessing'
	| 'queryCurseForgeLink'
	| 'listCurseForgeFiles'
>

export const [injectCurseForgeSelection, provideCurseForgeSelection] =
	createContext<CurseForgeSelection>('CurseForgeStage', 'curseforgeSelection')

/** A CurseForge modpack resolves through Modrinth Hosting's nodes, which a Core has none of. */
export function curseForgeUnsupported(): never {
	throw new Error('A Core does not install CurseForge modpacks.')
}

/** Creates an empty CurseForge selection. A Core resolves no links. */
export function createCurseForgeSelection(): CurseForgeSelection {
	return {
		curseforgeUrl: ref(''),
		curseforgeModpackFile: ref<CurseForgePackFile | null>(null),
		curseforgeServerPackFile: ref<CurseForgePackFile | null>(null),
		curseforgeModpack: ref<Kyros.FlameAnvil.v1.LinkQueryResponse | null>(null),
		curseforgeFileId: ref<number | null>(null),
		curseforgeFiles: ref<Kyros.FlameAnvil.v1.LinkQueryFile[]>([]),
		curseforgeProcessing: ref(false),
		queryCurseForgeLink: async () => curseForgeUnsupported(),
		listCurseForgeFiles: async () => curseForgeUnsupported(),
	}
}

/** Whether the selection is ready to install: an uploaded pack, or a link resolved for the selected file. */
export function isCurseForgeSelectionReady(selection: CurseForgeSelection): boolean {
	if (selection.curseforgeProcessing.value) return false
	if (selection.curseforgeModpackFile.value) return true
	const modpack = selection.curseforgeModpack.value
	return !!modpack && modpack.file.id === selection.curseforgeFileId.value
}

export function hasCurseForgeUploads(selection: CurseForgeSelection): boolean {
	return !!selection.curseforgeModpackFile.value || !!selection.curseforgeServerPackFile.value
}
