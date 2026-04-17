export interface Instance {
  id: string;
  name: string;
  game_type: 'vanilla' | 'forge' | 'fabric' | 'quilt' | 'paper' | 'spigot' | 'purpur' | 'mohist';
  version: string;
  java_version: string | null;
  status: 'stopped' | 'running' | 'starting' | 'stopping' | 'crashed';
  port: number | null;
  uuid: string;
  modpack_config: ModpackConfig | null;
  created_at: string;
  updated_at: string;
}

export interface ModpackConfig {
  project_id: string;
  version_id: string;
  overrides: Record<string, string>;
  linked_files: LinkedFile[];
}

export interface LinkedFile {
  path: string;
  project_id: string;
  file_id: string;
  hashes: Record<string, string>;
}

export interface Notification {
  id: string;
  type: 'server_status' | 'modpack_update' | 'friend_request' | 'invite_received' | 'system';
  title: string;
  body: string;
  read: boolean;
  created_at: string;
  metadata: Record<string, unknown>;
}

export interface Friend {
  id: string;
  modrinth_user_id: string;
  username: string;
  avatar_url: string | null;
  online: boolean;
  last_seen: string | null;
}

export interface FriendRequest {
  id: string;
  from_user_id: string;
  from_username: string;
  to_user_id: string;
  status: 'pending' | 'accepted' | 'declined';
  created_at: string;
}

export interface Group {
  id: string;
  name: string;
  owner_id: string;
  member_ids: string[];
  created_at: string;
}

export interface Presence {
  user_id: string;
  online: boolean;
  instance_id: string | null;
  instance_name: string | null;
  last_activity: string;
}

export interface ApiKey {
  id: string;
  type: 'owner' | 'member';
  permissions: string[];
  modrinth_user_id: string | null;
  label: string | null;
  created_at: string;
  revoked_at: string | null;
}

export interface ConnectionInfo {
  host: string;
  port: number;
  apiKey: string;
  keyType: 'owner' | 'member';
}

export interface AuthResult {
  success: boolean;
  key_type: 'owner' | 'member';
  permissions: string[];
  modrinth_user_id: string | null;
  username: string | null;
}

export interface ConsoleMessage {
  timestamp: number;
  line: string;
  type: 'stdout' | 'stderr' | 'system';
}

export interface SyncStatus {
  instance_id: string;
  total_mods: number;
  synced_mods: number;
  missing_mods: ModDownload[];
  errors: SyncError[];
}

export interface ModDownload {
  project_id: string;
  version_id: string;
  file_id: string;
  hash: string;
  path: string;
  status: 'pending' | 'downloading' | 'completed' | 'failed';
  progress?: number;
}

export interface SyncError {
  project_id: string;
  file_id: string;
  error: string;
}

export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

export interface HealthCheck {
  status: 'ok' | 'error';
  version: string;
  uptime: number;
}

export interface Invite {
  id: string;
  from_user_id: string;
  from_username: string;
  to_user_id: string;
  core_url: string;
  member_key: string;
  permissions: string[];
  created_at: string;
}

export interface CoreRegistry {
  modrinth_user_id: string;
  core_url: string;
  last_heartbeat: string;
}