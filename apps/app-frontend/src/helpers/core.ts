/**
 * TypeScript wrappers for Amberite Core Tauri commands.
 * All calls are forwarded to the `plugin:amberite|*` namespace.
 */
import { invoke } from '@tauri-apps/api/core'

export interface CoreInstanceDetail {
	id: string
	name: string
	game_version: string
	loader: string
	loader_version?: string
	/** "stopped" | "starting" | "running" | "stopping" */
	status: string
	data_dir: string
}

export function core_get_instance(id: string): Promise<CoreInstanceDetail> {
	return invoke('plugin:amberite|core_get_instance', { id })
}

export function core_start(id: string): Promise<void> {
	return invoke('plugin:amberite|core_start', { id })
}

export function core_stop(id: string): Promise<void> {
	return invoke('plugin:amberite|core_stop', { id })
}

export function core_restart(id: string): Promise<void> {
	return invoke('plugin:amberite|core_restart', { id })
}

export function core_send_command(id: string, command: string): Promise<void> {
	return invoke('plugin:amberite|core_send_command', { id, command })
}

/** Returns a one-time ticket string for the WebSocket console endpoint. */
export function core_issue_ws_token(): Promise<string> {
	return invoke('plugin:amberite|core_issue_ws_token')
}

/** Returns the Core base HTTP URL (e.g. "http://localhost:16662"). */
export function core_get_url(): Promise<string> {
	return invoke('plugin:amberite|core_get_url')
}
