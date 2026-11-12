// Generated from packages/engine by ts-rs. Do not edit.
// Regenerate: cargo test -p engine -- --ignored write_ts

export type EngineEvent = { "type": "loading", "payload": LoadingPayload } | { "type": "process", "payload": ProcessPayload } | { "type": "instance", "payload": InstancePayload } | { "type": "install_job", "payload": InstallJobSnapshot } | { "type": "warning", "payload": WarningPayload } | { "type": "log", "payload": LogPayload } | { "type": "ads_consent_required", "payload": boolean };

export type LoadingPayload = { event: LoadingBarType, loader_uuid: string, fraction: number | null, message: string, };

export type LoadingBarType = { "type": "legacy_data_migration" } | { "type": "directory_move", old: string, new: string, } | { "type": "java_download", version: number, } | { "type": "pack_file_download", instance_id: string, pack_name: string, icon: string | null, pack_version: string, } | { "type": "pack_download", instance_id: string, pack_name: string, icon: string | null, pack_id: string | null, pack_version: string | null, } | { "type": "pack_import", pack_name: string, } | { "type": "minecraft_download", instance_id: string, instance_name: string, } | { "type": "zip_extract", instance_id: string, instance_name: string, } | { "type": "pack_export", instance_id: string, instance_name: string, } | { "type": "config_change", new_path: string, } | { "type": "copy_instance", import_location: string, instance_name: string, } | { "type": "launcher_update", version: string, current_version: string, };

export type ProcessPayload = { instance_id: string, uuid: string, event: ProcessPayloadType, message: string, };

export type ProcessPayloadType = "launched" | "finished";

export type InstancePayload = { instance_id: string, } & ({ "event": "created" } | { "event": "synced" } | { "event": "servers_updated" } | { "event": "screenshots_updated" } | { "event": "world_updated", world: string, } | { "event": "server_joined", host: string, port: number, timestamp: string, } | { "event": "edited" } | { "event": "content_install_finished", project_ids: Array<string>, } | { "event": "content_install_failed", project_ids: Array<string>, message: string, } | { "event": "removed" });

export type InstancePayloadType = { "event": "created" } | { "event": "synced" } | { "event": "servers_updated" } | { "event": "screenshots_updated" } | { "event": "world_updated", world: string, } | { "event": "server_joined", host: string, port: number, timestamp: string, } | { "event": "edited" } | { "event": "content_install_finished", project_ids: Array<string>, } | { "event": "content_install_failed", project_ids: Array<string>, message: string, } | { "event": "removed" };

export type InstallJobSnapshot = { content_count: number | null, job_id: string, instance_id: string | null, kind: InstallJobKind, status: InstallJobStatus, paused: boolean, canceling: boolean, can_pause: boolean, can_cancel: boolean, target: InstallTarget, phase: InstallPhaseId, progress: InstallProgress | null, details: InstallPhaseDetails, display: InstallJobDisplay | null, error: InstallErrorView | null, rollback_error: InstallErrorView | null, created: string, modified: string, finished: string | null, };

export type InstallJobKind = "create_instance" | "create_modpack_instance" | "create_shared_instance" | "import_instance" | "duplicate_instance" | "install_existing_instance" | "install_pack_to_existing_instance" | "update_shared_instance" | "bulk_update_content";

export type InstallJobStatus = "queued" | "running" | "succeeded" | "failed" | "interrupted" | "canceled";

export type InstallTarget = { "type": "new_instance", instance_id: string | null, } | { "type": "existing_instance", instance_id: string, };

export type InstallPhaseId = "preparing_instance" | "resolving_pack" | "downloading_pack_file" | "reading_pack_manifest" | "downloading_content" | "extracting_overrides" | "resolving_minecraft" | "resolving_loader" | "preparing_java" | "downloading_minecraft" | "running_loader_processors" | "finalizing" | "rolling_back";

export type InstallProgress = { current: number, total: number, secondary?: InstallProgressSecondary, };

export type InstallProgressSecondary = { current: number, total: number, };

export type InstallPhaseDetails = { "type": "empty" } | { "type": "instance", name: string, } | { "type": "minecraft", game_version: string, loader: ModLoader, } | { "type": "java", major_version: number, step: InstallJavaStep, } | { "type": "modpack", project_id: string | null, version_id: string | null, title: string | null, } | { "type": "import", launcher_type: ImportLauncherType, instance_folder: string, };

export type InstallJavaStep = "resolving" | "fetching_metadata" | "downloading" | "extracting" | "validating";

export type InstallJobDisplay = { title: string, icon: string | null, };

export type InstallErrorView = { code: string, phase?: InstallPhaseId, message: string, api?: InstallApiErrorDetails, context?: InstallErrorContext, };

export type InstallApiErrorDetails = { error: string, status?: number, method?: string, url?: string, route?: string, };

export type InstallErrorContext = { operation: string, source_path?: string, target_path?: string, file_path?: string, entry_path?: string, urls: Array<string>, expected_hash?: string, expected_size?: number, project_id?: string, version_id?: string, minecraft_version?: string, loader?: string, java_version?: number, os?: string, arch?: string, };

export type ImportLauncherType = "MultiMC" | "PrismLauncher" | "ATLauncher" | "GDLauncher" | "Curseforge" | "Unknown";

export type ModLoader = "vanilla" | "forge" | "fabric" | "quilt" | "neoforge";

export type WarningPayload = { message: string, };

export type LogPayload = { instance_id: string, } & ({ "type": "log4j" } & Log4jEvent | { "type": "legacy", message: string, });

export type LogEvent = { "type": "log4j" } & Log4jEvent | { "type": "legacy", message: string, };

export type Log4jEvent = { timestamp_millis: number | null, logger_name: string | null, level: string | null, thread_name: string | null, message: string | null, throwable: string | null, };

export type FriendPayload = { "event": "friend_request", from: string, } | { "event": "user_offline", id: string, } | { "event": "status_update", user_status: FriendStatusPayload, } | { "event": "status_sync" } | { "event": "friend_request_rejected", from: string, };

export type FriendStatusPayload = { user_id: string, profile_name: string | null, last_update: string, };

export type OnboardingChecklist = { has_created_instance: boolean, has_logged_into_minecraft: boolean, has_logged_into_modrinth: boolean, show_checklist: boolean, };

export type InstanceGroupsChangedPayload = { instance_ids: Array<string>, };

export type CommandPayload = { "event": "InstallMod", id: string, } | { "event": "InstallVersion", id: string, } | { "event": "InstallModpack", id: string, } | { "event": "InstallServer", id: string, } | { "event": "LaunchInstance", id: string, server: string | null, singleplayer_world: string | null, } | { "event": "InstallSharedInstanceInvite", invite_id: string, } | { "event": "RunMRPack", path: string, } | { "event": "PlayHostingServer", server_id: string, world_id: string, };
