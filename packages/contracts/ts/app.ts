// Generated from apps/app by ts-rs. Do not edit.
// Regenerate: cargo test -p theseus_gui -- --ignored write_ts

import type { CoreSession, Endpoint } from './contracts'
import type { CommandPayload, EngineEvent, FriendPayload, InstanceGroupsChangedPayload, OnboardingChecklist } from './engine'

export type Boot = { version: string, platform: Platform, os_version: string, 
/**
 * A debug build.
 */
dev: boolean, updates_enabled: boolean, 
/**
 * Hover styles crash older macOS WebKit.
 */
disable_mouseover: boolean, 
/**
 * The dev backend a debug build's dev config names.
 */
backend_url: string | null, };

export type Platform = "windows" | "macos" | "linux";

export type CoreTarget = { core_id: string, account: string, address: string, fingerprint: string, };

export type NewCoreSession = { core_id: string, account: string, address: string, fingerprint: string, ticket: string, };

export type CoreSessionKey = { core_id: string, account: string, };

export type LocalFileQuery = { path: string, };

export type ThumbnailQuery = { path: string, size: number, };

export type RemoteIcon = { source: string, };

export type FileStat = { name: string, size: number, };

export type OpenDialog = { title: string | null, 
/**
 * A folder to start in, or a file name, with or without its folder.
 */
default_path: string | null, directory: boolean, multiple: boolean, filters: Array<DialogFilter>, };

export type SaveDialog = { title: string | null, default_path: string | null, filters: Array<DialogFilter>, };

export type DialogFilter = { name: string, 
/**
 * Without the dot.
 */
extensions: Array<string>, };

export type ExternalLink = { url: string, };

export type LocalPath = { path: string, };

export type AppFolder = { "folder": "launcher_logs" } | { "folder": "app_db_backups" } | { "folder": "synced_options" } | { "folder": "screenshot", instance_id: string, file_name: string, };

export type NewShortcut = { instance_name: string, instance_id: string, output_path: string, server: string | null, singleplayer_world: string | null, };

export type WebviewLog = { level: WebviewLogLevel, message: string, };

export type WebviewLogLevel = "trace" | "debug" | "info" | "warn" | "error";

export type LocalEvent = EngineEvent | AppEvent;

export type AppEvent = { "type": "friend", "payload": FriendPayload } | { "type": "notification", "payload": unknown } | { "type": "onboarding_checklist", "payload": OnboardingChecklist } | { "type": "instance_groups_changed", "payload": InstanceGroupsChangedPayload } | { "type": "command", "payload": Command } | { "type": "game_option_locales_updated" } | { "type": "resync" };

export type Command = CommandPayload | AmberiteCommand;

export type AmberiteCommand = { "event": "ApproveCore", code: string, };

export const health = {
	checkHealth: { method: "GET", path: "/health", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
}

export const boot = {
	getBoot: { method: "GET", path: "/app/boot", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Boot>,
}

export const pairedCores = {
	connectCore: { method: "POST", path: "/app/cores/connect", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CoreTarget, string>,
	signInToCore: { method: "POST", path: "/app/cores/sessions", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewCoreSession, CoreSession>,
	signOutOfCore: { method: "DELETE", path: "/app/cores/sessions", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, CoreSessionKey, void, void>,
	checkCoreSession: { method: "GET", path: "/app/cores/sessions", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, CoreSessionKey, void, boolean>,
}

export const events = {
	eventSocket: { method: "GET", path: "/events", auth: "none", dangerous: false, body: "empty", response: "upgrade" } as Endpoint<null, null, void, { receive: LocalEvent, send: null }>,
}

export const localFiles = {
	getLocalFile: { method: "GET", path: "/app/file", auth: "none", dangerous: false, body: "empty", response: "stream" } as Endpoint<null, LocalFileQuery, void, Response>,
	getThumbnail: { method: "GET", path: "/app/thumbnail", auth: "none", dangerous: false, body: "empty", response: "stream" } as Endpoint<null, ThumbnailQuery, void, Response>,
	cacheRemoteIcon: { method: "POST", path: "/app/icons/remote", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, RemoteIcon, string>,
	readLocalFile: { method: "GET", path: "/app/fs/read", auth: "none", dangerous: false, body: "empty", response: "stream" } as Endpoint<null, LocalFileQuery, void, Response>,
	statLocalFile: { method: "GET", path: "/app/fs/stat", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, LocalFileQuery, void, FileStat>,
}

export const desktop = {
	pickFiles: { method: "POST", path: "/app/dialog/open", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, OpenDialog, Array<string> | null>,
	pickSavePath: { method: "POST", path: "/app/dialog/save", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, SaveDialog, string | null>,
	openUrl: { method: "POST", path: "/app/opener/url", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, ExternalLink, void>,
	openPath: { method: "POST", path: "/app/opener/path", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, LocalPath, void>,
	revealPath: { method: "POST", path: "/app/opener/reveal", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, LocalPath, void>,
	openFolder: { method: "POST", path: "/app/folders/open", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, AppFolder, void>,
	createShortcut: { method: "POST", path: "/app/shortcuts", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewShortcut, string>,
	checkNetworkMetered: { method: "GET", path: "/app/network/metered", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, boolean>,
	logFromWebview: { method: "POST", path: "/app/webview-log", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<WebviewLog>, void>,
}
