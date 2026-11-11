// Generated from apps/app by ts-rs. Do not edit.
// Regenerate: cargo test -p theseus_gui -- --ignored write_ts

import type { Endpoint } from './contracts'

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

export const health = {
	checkHealth: { method: "GET", path: "/health", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
}

export const boot = {
	getBoot: { method: "GET", path: "/app/boot", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Boot>,
}
