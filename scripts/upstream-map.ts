// What scripts/upstream-sync.ts cannot read from git. Keys are Modrinth's paths, a file or a folder,
// and the longest matching prefix wins.
//
// Nothing here lists what we deleted: a file or folder Modrinth has and we do not stays deleted on
// every merge, because our tree already says so.

// Modrinth's paths that live under another name here. The merge renames them in Modrinth's tree
// first, so Modrinth's edits and new files land in ours. List every folder we moved. Git pairs a
// single moved file by itself while the two stay alike; list one once it has drifted so far that
// the report shows its changes as dropped.
export const MOVED: Record<string, string> = {
	'apps/app/src/api/oauth_utils': 'apps/app/src/accounts/oauth_utils',
	'apps/app/src/api/shortcuts': 'apps/app/src/desktop/shortcuts',
	'packages/app-lib': 'packages/engine',
	'packages/app-lib/src/api/friends.rs': 'apps/app/src/friends/api.rs',
	'packages/app-lib/src/api/instance/hidden.rs': 'apps/app/src/library/hidden.rs',
	'packages/app-lib/src/api/instance/screenshots': 'apps/app/src/screenshots',
	'packages/app-lib/src/api/instance/synced_options': 'apps/app/src/synced_options/options',
	'packages/app-lib/src/api/instance/synced_packs': 'apps/app/src/synced_options/packs',
	'packages/app-lib/src/api/instance/synced_servers': 'apps/app/src/synced_options/servers',
	'packages/app-lib/src/api/instance/synced_servers/codec.rs':
		'packages/engine/src/api/server_list.rs',
	'packages/app-lib/src/api/minecraft_skins': 'apps/app/src/skins/minecraft_skins',
	'packages/app-lib/src/api/minecraft_auth.rs': 'apps/app/src/accounts/minecraft_auth.rs',
	'packages/app-lib/src/api/mr_auth.rs': 'apps/app/src/accounts/mr_auth.rs',
	'packages/app-lib/src/api/onboarding_checklist.rs': 'apps/app/src/onboarding/checklist.rs',
	'packages/app-lib/src/state/minecraft_skins': 'apps/app/src/skins/state',
	'packages/app-lib/src/state/server_join_log.rs': 'apps/app/src/library/join_log.rs',
}

// Modrinth's code we rewrote, and the place that does its job here. Its changes cannot merge, so
// the report lists each one for porting by hand. List a folder we rewrote as a whole, or its new
// files are dropped with it unseen.
export const PORTED: Record<string, string> = {
	// Tauri commands. Ours are Endpoints: engine routes, or `apps/app/src/<domain>/routes.rs` for a
	// desktop domain.
	'apps/app/src/api': 'packages/engine/src/routes',
	'apps/app-frontend/src/helpers/instance-content.ts':
		'packages/ui/src/layouts/shared/content-tab/providers/instance-content.ts',
	// Modrinth's `invoke` helpers, one folder per domain. Ours call `client.local` from helpers.
	'apps/app-frontend/src/platform/app-lib': 'apps/app-frontend/src/helpers',
	'apps/app-frontend/src/platform/app-lib/files/commands.ts':
		'packages/ui/src/layouts/shared/files-tab/providers/instance-files.ts',
	'apps/app-frontend/src/platform/app-lib/hidden-instances/commands.ts':
		'apps/app-frontend/src/features/library/hidden-instances/queries.ts',
	'apps/app-frontend/src/platform/app-lib/icons/commands.ts':
		'apps/app-frontend/src/platform/adapters/icon-cache.ts',
	'apps/app-frontend/src/platform/app-lib/onboarding-checklist/commands.ts':
		'apps/app-frontend/src/pages/Index.vue',
	'apps/app-frontend/src/platform/app-lib/shared-instances/commands.ts':
		'apps/app-frontend/src/features/library/shared-instances/shared-instance-modals.vue',
	// The parts of labrinth that apps/backend serves. Keep labrinth's schemas and contracts.
	'apps/labrinth/src/database/models/blocked_user_item.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/database/models/friend_item.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/database/models/notification_item.rs':
		'apps/backend/src/common/notifications.ts',
	'apps/labrinth/src/database/models/session_item.rs': 'apps/backend/src/common/auth.ts',
	'apps/labrinth/src/database/models/user_item.rs': 'apps/backend/src/common/auth.ts',
	'apps/labrinth/src/database/models/user_lock_item.rs': 'apps/backend/src/common/auth.ts',
	'apps/labrinth/src/database/models/user_preferences_item.rs':
		'apps/backend/src/social/preferences.ts',
	'apps/labrinth/src/models/v3/notifications.rs': 'apps/backend/src/common/notifications.ts',
	'apps/labrinth/src/models/v3/sessions.rs': 'apps/backend/src/accounts/index.ts',
	'apps/labrinth/src/models/v3/users.rs': 'apps/backend/src/common/auth.ts',
	'apps/labrinth/src/routes/internal/admin/user_credentials.rs':
		'apps/backend/src/accounts/index.ts',
	'apps/labrinth/src/routes/internal/admin/user_lock.rs': 'apps/backend/src/accounts/index.ts',
	'apps/labrinth/src/routes/internal/admin/user_sessions.rs': 'apps/backend/src/accounts/index.ts',
	'apps/labrinth/src/routes/internal/flows.rs': 'apps/backend/src/accounts/index.ts',
	'apps/labrinth/src/routes/internal/session.rs': 'apps/backend/src/accounts/index.ts',
	'apps/labrinth/src/routes/v2/users.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/routes/v3/blocked_users.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/routes/v3/friends.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/routes/v3/notifications.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/routes/v3/users.rs': 'apps/backend/src/social/index.ts',
	'apps/labrinth/src/sync/friends.rs': 'apps/backend/src/social/socket.ts',
	'apps/meta-generator': 'apps/server-meta/src/main.rs',
	// A new Modrinth migration merges in as a file. Fold it into our schema and delete it.
	'packages/app-lib/migrations': 'packages/engine/migrations/20261113120000_init.sql',
	'packages/app-lib/src/api/reports.rs': 'apps/app-frontend/src/helpers/reports.ts',
	'packages/app-lib/src/api/users.rs': 'apps/app-frontend/src/helpers/users.ts',
	'packages/app-lib/src/state/content_store/commands/migration.rs':
		'packages/engine/src/state/content_store/commands/recovery.rs',
	// The engine keeps Modrinth's file, which merges, and the App has its own copy of the desktop half.
	'packages/app-lib/src/state/db_backup.rs': 'apps/app/src/db_backup.rs',
	'packages/app-lib/src/state/settings.rs': 'apps/app/src/settings/mod.rs',
	'packages/app-lib/src/util/network.rs': 'apps/app/src/desktop/network.rs',
	'packages/ariadne/src/networking': 'apps/backend/src/social/socket.ts',
}

// Files nobody merges by hand. When both sides changed one, this side wins.
export const SIDES: readonly (readonly [RegExp, 'ours' | 'theirs'])[] = [
	[/(^|\/)(AGENTS|CLAUDE)\.md$/, 'ours'],
	[/^README\.md$/, 'ours'],
	// Keyed by a hash of the query, so ours describes our schema.
	[/\/\.sqlx\//, 'ours'],
	// Modrinth's translators own every language but the source one.
	[/\/locales\/(?!en-US\/)[^/]+\/index\.json$/, 'theirs'],
	// `--continue` regenerates these against our manifests.
	[/^(Cargo\.lock|pnpm-lock\.yaml)$/, 'theirs'],
]
