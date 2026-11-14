import { ContractClient, desktop, type DialogFilter } from '@modrinth/api-client'

import { localTransport } from '@/platform/local'

// `open` and `save` keep `@tauri-apps/plugin-dialog`'s names and shapes. The App shows the dialog
// on its desktop, from the native window or a tab.

export interface OpenDialogOptions {
	title?: string
	defaultPath?: string
	directory?: boolean
	multiple?: boolean
	filters?: DialogFilter[]
}

export interface SaveDialogOptions {
	title?: string
	defaultPath?: string
	filters?: DialogFilter[]
}

export type OpenDialogReturn<T extends OpenDialogOptions> = T['multiple'] extends true
	? string[] | null
	: string | null

const local = new ContractClient(localTransport)

export async function open<T extends OpenDialogOptions>(options?: T): Promise<OpenDialogReturn<T>> {
	const paths = await local.call(desktop.pickFiles, {
		body: {
			title: options?.title ?? null,
			default_path: options?.defaultPath ?? null,
			directory: options?.directory ?? false,
			multiple: options?.multiple ?? false,
			filters: options?.filters ?? [],
		},
	})
	const picked = options?.multiple ? paths : (paths?.[0] ?? null)
	return picked as OpenDialogReturn<T>
}

export async function save(options?: SaveDialogOptions): Promise<string | null> {
	return await local.call(desktop.pickSavePath, {
		body: {
			title: options?.title ?? null,
			default_path: options?.defaultPath ?? null,
			filters: options?.filters ?? [],
		},
	})
}
