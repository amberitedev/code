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
