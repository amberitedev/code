import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import { applyDevAppConfig } from '@/config'

export interface DevAppConfig {
	appId: string
	credentialNamespace: string
	username: string | null
	authMode: 'dev' | 'real'
	branch: string
	title: string
	dataDir: string
	convexUrl: string
	convexSiteUrl: string
	controlPort: number
}

interface AccountSwitchRequest {
	requestId: string
	username: string
}

type AccountSwitcher = (request: AccountSwitchRequest) => Promise<void>

let config: DevAppConfig | null = null
let accountSwitcher: AccountSwitcher | null = null

export async function initializeDevRuntime(): Promise<void> {
	config = await invoke<DevAppConfig | null>('get_amberite_dev_config')
	if (!config) return
	applyDevAppConfig(config)
	await listen<AccountSwitchRequest>('amberite://dev-account', async (event) => {
		let error: string | null = null
		try {
			if (!accountSwitcher) throw new Error('Amberite auth is not ready')
			await accountSwitcher(event.payload)
			config = { ...config!, username: event.payload.username }
		} catch (reason) {
			error = reason instanceof Error ? reason.message : String(reason)
		} finally {
			await invoke('complete_amberite_dev_account_switch', {
				requestId: event.payload.requestId,
				error,
			})
		}
	})
}

export function getDevAppConfig(): DevAppConfig | null {
	return config
}

export function registerDevAccountSwitcher(switcher: AccountSwitcher): void {
	accountSwitcher ??= switcher
	void invoke('mark_amberite_dev_ui_ready')
}
