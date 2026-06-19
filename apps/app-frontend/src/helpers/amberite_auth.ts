import { invoke } from '@tauri-apps/api/core'

export interface MinecraftCredential {
	accessToken: string
	username: string
	uuid: string
}

export async function login(): Promise<MinecraftCredential> {
	return await invoke('plugin:auth|amberite_login')
}
