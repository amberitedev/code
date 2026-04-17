/**
 * Mocked skins helper - returns empty/fake data
 * TODO: connect to backend API - replace with real skin management
 */

import { arrayBufferToBase64 } from '@modrinth/utils'

export interface Cape {
	id: string
	name: string
	texture: string
	is_default: boolean
	is_equipped: boolean
}

export type SkinModel = 'CLASSIC' | 'SLIM' | 'UNKNOWN'
export type SkinSource = 'default' | 'custom_external' | 'custom'

export interface Skin {
	texture_key: string
	name?: string
	variant: SkinModel
	cape_id?: string
	texture: string
	source: SkinSource
	is_equipped: boolean
}

export interface SkinTextureUrl {
	original: string
	normalized: string
}

export const DEFAULT_MODEL_SORTING = ['Steve', 'Alex'] as string[]

export const DEFAULT_MODELS: Record<string, SkinModel> = {
	Steve: 'CLASSIC',
	Alex: 'SLIM',
	Zuri: 'CLASSIC',
	Sunny: 'CLASSIC',
	Noor: 'SLIM',
	Makena: 'SLIM',
	Kai: 'CLASSIC',
	Efe: 'SLIM',
	Ari: 'CLASSIC',
}

export function filterSavedSkins(_list: Skin[]) {
	return []
}

export async function determineModelType(_texture: string): Promise<'SLIM' | 'CLASSIC'> {
	return 'CLASSIC'
}

export async function fixUnknownSkins(_list: Skin[]) {
	// no-op
}

export function filterDefaultSkins(_list: Skin[]) {
	return []
}

export async function get_available_capes(): Promise<Cape[]> {
	return []
}

export async function get_available_skins(): Promise<Skin[]> {
	return []
}

export async function add_and_equip_custom_skin(_textureBlob: Uint8Array, _variant: SkinModel, _capeOverride?: Cape): Promise<void> {
	// no-op
}

export async function set_default_cape(_cape?: Cape): Promise<void> {
	// no-op
}

export async function equip_skin(_skin: Skin): Promise<void> {
	// no-op
}

export async function remove_custom_skin(_skin: Skin): Promise<void> {
	// no-op
}

export async function get_normalized_skin_texture(_skin: Skin): Promise<string> {
	return ''
}

export async function normalize_skin_texture(_texture: Uint8Array | string): Promise<Uint8Array> {
	return new Uint8Array()
}

export async function unequip_skin(): Promise<void> {
	// no-op
}

export async function get_dragged_skin_data(_path: string): Promise<Uint8Array> {
	return new Uint8Array()
}
