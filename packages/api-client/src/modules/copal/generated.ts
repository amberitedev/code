// Generated from apps/core by ts-rs. Do not edit.
// Regenerate: pnpm --filter @amberite/core types

export type EditedFiles = { files: Array<EditedFile>, };

export type EditedFile = { 
/**
 * Relative to the server folder, with forward slashes.
 */
path: string, 
/**
 * The owner deleted it (or disabled it) rather than changed it.
 */
deleted: boolean, 
/**
 * What the last pack update wants for it, waiting on a [`Decision`]; null when nothing waits.
 */
pending: PendingChange | null, };

export type PendingChange = "changed" | "removed";

export type Decision = { path: string, choice: Choice, };

export type Choice = "keep" | "replace";

export type GameVersionChange = { from: string, to: string, };

export type SharedInstancePublishDiff = { shared_instance_id: string, latest_version: number, has_changes: boolean, 
/**
 * The server is running, so it runs these changes only once it restarts.
 */
requires_restart: boolean, diffs: Array<SharedInstanceUpdateDiff>, };

export type SharedInstanceUpdateDiff = { type: SharedInstanceUpdateDiffType, projectId: string | null, projectName: string | null, 
/**
 * A file without a Modrinth project, by its name in its type's folder.
 */
fileName: string | null, currentVersionName: string | null, newVersionName: string | null, configFileCount?: number, configFileNames?: Array<string>, 
/**
 * A removal of something the owner had turned off, which members never ran.
 */
disabled?: boolean, };

export type SharedInstanceUpdateDiffType = "added" | "removed" | "updated" | "modpack_linked" | "modpack_updated" | "modpack_unlinked" | "game_version_updated" | "loader_updated" | "config_files_updated";

export type SharedInstanceHistoryEntry = { version: number, 
/**
 * When the push finished, in milliseconds since the Unix epoch.
 */
created: number, gameVersion: string, 
/**
 * The share's loader name, such as `fabric`, or `vanilla` for a server without a client
 * loader.
 */
loader: string, loaderVersion: string, 
/**
 * Projects and files, not counting config files.
 */
contentCount: number, diffs: Array<SharedInstanceUpdateDiff>, 
/**
 * Paths of the config files it changed, or of all it holds without a base.
 */
configFiles: Array<string>, };
