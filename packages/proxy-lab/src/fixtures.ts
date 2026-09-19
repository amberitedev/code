import type { Archon } from '../../api-client/src/modules/archon/types.ts'
import type { Labrinth } from '../../api-client/src/modules/labrinth/types.ts'
import type { SharedInstances } from '../../api-client/src/modules/shared-instances/types.ts'

import type { LabState, Persona } from './types.ts'

export const HOST = '127.0.0.1'
export const PORT = 8000
export const ORIGIN = `http://${HOST}:${PORT}`
export const SERVER_ID = 'proxy-lab-server'
export const WORLD_ID = 'proxy-lab-world'
export const SHARED_INSTANCE_ID = 'proxy-lab-shared-instance'
export const OWNER_USER_ID = 'proxy-lab-owner'
export const RECIPIENT_USER_ID = 'proxy-lab-recipient'
const SERVER_ADMIN_PERMISSIONS = -32_768

const created = '2026-01-01T00:00:00.000Z'

function persona(id: Persona['id'], username: string, userId: string): Persona {
	return {
		id,
		token: `proxy-lab-${id}-token`,
		user: {
			id: userId,
			username,
			bio: 'Local proxy-lab fixture account',
			created,
			role: 'developer',
			badges: 0,
			campaigns: { pride_26: null },
			email: `${id}@proxy-lab.invalid`,
			email_verified: true,
			has_password: false,
			has_totp: false,
			allow_friend_requests: true,
		},
	}
}

export const PERSONAS = {
	owner: persona('owner', 'ProxyLabOwner', OWNER_USER_ID),
	recipient: persona('recipient', 'ProxyLabRecipient', RECIPIENT_USER_ID),
} satisfies Record<Persona['id'], Persona>

const ownerV2User = {
	id: OWNER_USER_ID,
	username: PERSONAS.owner.user.username,
	name: PERSONAS.owner.user.username,
	bio: PERSONAS.owner.user.bio ?? '',
	created,
	role: 'developer',
	badges: 0,
} satisfies Labrinth.Users.v2.User

export function v2UserFor(persona: Persona): Labrinth.Users.v2.User {
	return {
		...ownerV2User,
		id: persona.user.id,
		username: persona.user.username,
		name: persona.user.username,
		...(persona.user.email ? { email: persona.user.email } : {}),
	}
}

export const DEFAULT_PREFERENCES = {
	appearance: { auto: true, theme: 'dark' },
	behavior: {
		minimize_app: false,
		hide_right_sidebar: false,
		show_jump_in: true,
		compact_instance_cards: false,
		show_play_time: true,
		hide_nametag: false,
		show_all_screenshots: false,
		show_files_tab_in_instances: false,
		show_worlds_tab_in_instances: true,
		show_screenshots_tab_in_instances: true,
		show_skin_selector_in_sidebar: true,
		quick_instance_count: 5,
		warn_on_unknown_modpacks: true,
		skip_non_essential_warnings: false,
	},
	localization: { locale: 'en-US' },
	layouts: {
		mods: 'grid',
		plugins: 'grid',
		datapacks: 'grid',
		shaders: 'grid',
		resourcepacks: 'grid',
		modpacks: 'grid',
		servers: 'grid',
		users: 'grid',
	},
	sidebars: {
		right_aligned_search: false,
		left_aligned_content: false,
	},
	social: {
		friend_privacy: 'everyone',
		shared_instances_privacy: 'everyone',
		hosting_access_privacy: 'everyone',
	},
} satisfies Labrinth.Users.v3.UserPreferences

const initialBackup = {
	id: 'proxy-lab-backup-1',
	physical_id: 'proxy-lab-backup-1',
	name: 'Before the lighthouse',
	created_at: '2026-01-03T12:00:00.000Z',
	automated: false,
	status: 'done',
	interrupted: false,
	ongoing: false,
	locked: false,
} satisfies Archon.Backups.v1.Backup

const initialAddon = {
	id: 'sodium',
	filename: 'sodium-fabric.jar',
	filesize: 1_431_552,
	btime: '2026-01-02T10:00:00.000Z',
	disabled: false,
	kind: 'mod',
	from_modpack: false,
	status: 'installed',
	pack_client_retained: false,
	pack_client_depends: true,
	has_update: 'YAGZ1cCS',
	name: 'Sodium',
	project_id: 'AANobbMI',
	version: {
		id: 'yaoBL9D9',
		name: 'Sodium fixture version',
		environment: null,
	},
	owner: {
		id: 'jellysquid3',
		name: 'CaffeineMC',
		type: 'organization',
		icon_url: null,
	},
	icon_url: 'https://cdn.modrinth.com/data/AANobbMI/icon.png',
} satisfies Archon.Content.v1.Addon

export function createSeedState(): LabState {
	const serverV0 = {
		server_id: SERVER_ID,
		name: 'Proxy Lab Lighthouse',
		owner_id: OWNER_USER_ID,
		net: {
			ip: '127.0.0.1',
			port: 25565,
			domain: 'proxy-lab.invalid',
		},
		game: 'Minecraft',
		backup_quota: 10 * 1024 * 1024 * 1024,
		used_backup_quota: 512 * 1024 * 1024,
		status: 'available',
		suspension_reason: null,
		loader: 'Fabric',
		loader_version: '0.16.10',
		mc_version: '1.21.1',
		upstream: null,
		sftp_username: 'proxy-lab',
		sftp_password: 'fixture-only',
		sftp_host: HOST,
		datacenter: 'local',
		notices: [
			{
				id: 1,
				dismissable: true,
				title: 'Local simulation',
				message: 'This server exists only inside proxy-lab.',
				level: 'info',
				announced: created,
			},
		],
		node: { token: 'proxy-lab-node-token', instance: `${ORIGIN}/node` },
		flows: { intro: false },
		is_medal: false,
		current_user_permissions: SERVER_ADMIN_PERMISSIONS,
	} satisfies Archon.Servers.v0.Server

	const content = {
		modloader: 'fabric',
		modloader_version: '0.16.10',
		game_version: '1.21.1',
		modpack: null,
		addons: [initialAddon],
	} satisfies Archon.Content.v1.Addons

	const serverV1 = {
		id: SERVER_ID,
		name: serverV0.name,
		subdomain: 'proxy-lab',
		specs: { cpu: 2, memory_mb: 4096, storage_mb: 20_480, swap_mb: 1024 },
		sftp_username: serverV0.sftp_username,
		sftp_password: serverV0.sftp_password,
		tags: ['fixture', 'local'],
		location: {
			status: 'assigned',
			location_metadata: {
				region: 'local',
				region_should_be_user_displayed: true,
				hostname: 'proxy-lab',
				url_host: HOST,
				is_decommissioned_node: false,
			},
		},
		worlds: [
			{
				id: WORLD_ID,
				name: 'Lighthouse World',
				created_at: created,
				is_active: true,
				download_method: { method_type: 'backup', backup_id: initialBackup.id },
				backups: [initialBackup],
				content: {
					modloader: 'fabric',
					modloader_version: '0.16.10',
					game_version: '1.21.1',
					java_version: 21,
					invocation: 'java -Xmx4G -jar server.jar nogui',
					original_invocation: 'java -jar server.jar nogui',
				},
				readiness: { data_synchronized_fetched: true },
			},
		],
	} satisfies Archon.Servers.v1.ServerFull

	const sharedVersion = {
		version: 1,
		modrinth_ids: [],
		ready: true,
		external_files: [
			{
				file_name: 'proxy-lab-empty.jar',
				file_type: 'mod',
				url: `${ORIGIN}/fixtures/proxy-lab-empty.jar`,
				file_size: 22,
				uploaded: true,
			},
		],
		modpack_id: null,
		game_version: '1.21.1',
		loader: 'fabric',
		loader_version: '0.16.10',
	} satisfies SharedInstances.Instances.v1.InstanceVersion & {
		external_files: Array<SharedInstances.Instances.v1.ExternalFile & { uploaded: boolean }>
	}

	return {
		schema_version: 1,
		next_backup: 2,
		next_invite: 2,
		next_shared_version: 2,
		archon: {
			server_v0: serverV0,
			server_v1: serverV1,
			power_state: 'running',
			started_at: '2026-01-01T12:00:00.000Z',
			content,
			properties: {
				known: {
					difficulty: 'normal',
					gamemode: 'survival',
					max_players: '12',
					motd: 'Proxy Lab Lighthouse',
					view_distance: '12',
				},
				custom: { 'proxy-lab-fixture': 'true' },
			},
			startup: {
				java_version: 21,
				jre_vendor: 'temurin',
				original_invocation: 'java -jar server.jar nogui',
				startup_command: 'java -Xmx4G -jar server.jar nogui',
			},
			allocations: [
				{ port: 25565, name: 'Minecraft' },
				{ port: 25566, name: 'Simple Voice Chat' },
			],
			users: [
				{
					user: {
						id: OWNER_USER_ID,
						username: PERSONAS.owner.user.username,
						avatar_url: null,
					},
					added_on: created,
					last_invite_sent: null,
					permissions: 'SERVER_ADMIN',
				},
				{
					user: {
						id: RECIPIENT_USER_ID,
						username: PERSONAS.recipient.user.username,
						avatar_url: null,
					},
					added_on: '2026-01-02T00:00:00.000Z',
					last_invite_sent: null,
					permissions: 1,
				},
			],
			backups: [initialBackup],
			backup_queue: {
				active_operations: [],
				backups: [
					{
						id: initialBackup.id,
						name: initialBackup.name,
						created_at: initialBackup.created_at,
						status: 'done',
						locked: false,
						automated: false,
						history: [],
					},
				],
			},
			actions: [
				{
					actor: { type: 'user', user_id: OWNER_USER_ID },
					action: { action: 'server_created' },
					server_id: SERVER_ID,
					world_id: WORLD_ID,
					timestamp: created,
				},
			],
		},
		shared_instances: [
			{
				id: SHARED_INSTANCE_ID,
				name: 'Proxy Lab Shared Pack',
				icon: null,
				quarantine: false,
				owner_id: OWNER_USER_ID,
				users: [
					{
						id: OWNER_USER_ID,
						joined_at: created,
						join_type: 'owner',
						last_played: null,
					},
					{
						id: RECIPIENT_USER_ID,
						joined_at: '2026-01-02T00:00:00.000Z',
						join_type: 'invite',
						last_played: null,
					},
				],
				tokens: 1,
				versions: [sharedVersion],
				invites: [
					{
						id: 'proxy-lab-invite-1',
						expiration: '2030-01-01T00:00:00.000Z',
						max_uses: 5,
						uses: 1,
					},
				],
				pending_user_ids: [],
			},
		],
		preferences: {
			[OWNER_USER_ID]: structuredClone(DEFAULT_PREFERENCES),
			[RECIPIENT_USER_ID]: structuredClone(DEFAULT_PREFERENCES),
		},
	}
}
