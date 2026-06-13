<script setup lang="ts">
import { PlusIcon, ShieldIcon, TrashIcon } from '@modrinth/assets'
import { ButtonStyled, StyledInput, Table, Toggle, UnsavedChangesPopup } from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, ref } from 'vue'

interface RoleConfig {
	id: string
	label: string
	builtIn?: boolean
}

interface PermissionRow {
	[key: string]: unknown
	id: string
	name: string
	description: string
}

type RoleGrants = Record<string, Record<string, boolean>>
type RolePermissionsState = { roles: RoleConfig[]; grants: RoleGrants }
const MAX_ROLES = 4
const MAX_ROLE_NAME_LENGTH = 20
const permissions: PermissionRow[] = [
	{ id: 'invite-members', name: 'Invite members', description: 'Create group invites.' },
	{ id: 'remove-members', name: 'Remove members', description: 'Kick non-owner members.' },
	{ id: 'ban-members', name: 'Ban members', description: 'Ban users from the group.' },
	{ id: 'manage-roles', name: 'Manage roles', description: 'Create and edit Core roles.' },
	{ id: 'edit-member-roles', name: 'Edit member roles', description: 'Change member roles.' },
	{ id: 'approve-invites', name: 'Approve invites', description: 'Review pending invite requests.' },
	{ id: 'manage-instances', name: 'Manage instances', description: 'Create and edit Core instances.' },
	{ id: 'start-stop-instances', name: 'Start and stop instances', description: 'Control power state.' },
	{ id: 'restart-instances', name: 'Restart instances', description: 'Restart running instances.' },
	{ id: 'manage-mods', name: 'Manage mods', description: 'Install, update, and remove mods.' },
	{ id: 'manage-worlds', name: 'Manage worlds', description: 'Create, edit, and switch worlds.' },
	{ id: 'manage-files', name: 'Manage files', description: 'Edit Core-managed files.' },
	{ id: 'manage-backups', name: 'Manage backups', description: 'Create and restore backups.' },
	{ id: 'manage-network', name: 'Manage networking', description: 'Edit network settings.' },
	{ id: 'read-console', name: 'Read console', description: 'View instance console output.' },
	{ id: 'write-console', name: 'Write console', description: 'Send console commands.' },
	{ id: 'edit-settings', name: 'Edit Core settings', description: 'Change global Core settings.' },
	{ id: 'view-activity', name: 'View activity', description: 'Read member activity.' },
	{ id: 'export-data', name: 'Export data', description: 'Export Core-managed data.' },
	{ id: 'dangerous-actions', name: 'Dangerous actions', description: 'Run destructive Core actions.' },
]
const defaultState: RolePermissionsState = {
	roles: [
		{ id: 'admin', label: 'Admin', builtIn: true },
		{ id: 'member', label: 'Member', builtIn: true },
	],
	grants: {
		admin: Object.fromEntries(permissions.map((permission) => [permission.id, true])),
		member: {
			'start-stop-instances': true,
			'restart-instances': true,
			'read-console': true,
			'view-activity': true,
		},
	},
}
const savedState = useStorage<RolePermissionsState>('copal-role-permissions', defaultState)
const roles = ref(cloneRoles(savedState.value.roles).slice(0, MAX_ROLES))
const grants = ref(cloneGrants(savedState.value.grants))
const newRoleName = ref('')
const isUpdating = ref(false)
const columns = computed(() => [
	{ key: 'name', label: 'Permission', width: '28%' },
	...roles.value.map((role) => ({
		key: role.id,
		label: role.label,
		align: 'center' as const,
		width: `${68 / Math.max(roles.value.length, 1)}%`,
	})),
	{ key: 'actions', label: '', align: 'right' as const, width: '4rem' },
])
const modifiedState = computed(() => ({ roles: roles.value, grants: grants.value }))
const newRoleLabel = computed(() => newRoleName.value.trim().slice(0, MAX_ROLE_NAME_LENGTH))
const canAddRole = computed(() => newRoleLabel.value.length > 0 && canUseRoleId.value)
const canUseRoleId = computed(() => {
	const id = normalizeRoleId(newRoleLabel.value)
	return roles.value.length < MAX_ROLES && id.length > 0 && !hasRole(id)
})

function cloneRoles(source: RoleConfig[]) {
	return source.map((role) => ({ ...role }))
}
function cloneGrants(source: RoleGrants) {
	return Object.fromEntries(Object.entries(source).map(([role, grants]) => [role, { ...grants }]))
}
function normalizeRoleId(name: string) {
	return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
}
function hasRole(id: string) {
	return roles.value.some((role) => role.id === id)
}
function addRole() {
	const label = newRoleLabel.value
	const id = normalizeRoleId(label)
	if (!canAddRole.value) return
	roles.value.push({ id, label })
	grants.value[id] = {}
	newRoleName.value = ''
}
function removeRole(role: RoleConfig) {
	if (role.builtIn) return
	roles.value = roles.value.filter((item) => item.id !== role.id)
	const next = { ...grants.value }
	delete next[role.id]
	grants.value = next
}
function save() {
	isUpdating.value = true
	savedState.value = { roles: cloneRoles(roles.value), grants: cloneGrants(grants.value) }
	isUpdating.value = false
}
function reset() {
	roles.value = cloneRoles(savedState.value.roles).slice(0, MAX_ROLES)
	grants.value = cloneGrants(savedState.value.grants)
	newRoleName.value = ''
}
function getRoleGrants(roleId: string) {
	if (!grants.value[roleId]) grants.value[roleId] = {}
	return grants.value[roleId]
}
</script>

<template>
	<div class="relative flex flex-col gap-4 pb-10">
		<div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
			<div class="flex min-w-0 flex-col gap-1">
				<h2 class="m-0 text-2xl font-semibold text-contrast">Roles</h2>
				<p class="m-0 text-secondary">Configure the permissions each Core role grants.</p>
			</div>
			<div class="flex min-w-0 flex-col gap-2 sm:flex-row md:w-[42rem]">
				<div class="flex h-10 shrink-0 items-center gap-2 rounded-full bg-surface-3 px-3 text-sm font-semibold text-secondary">
					<ShieldIcon class="size-4" />
					<span>{{ roles.length }}/{{ MAX_ROLES }}</span>
				</div>
				<StyledInput
					v-model="newRoleName"
					placeholder="New role"
					wrapper-class="min-w-0 flex-1"
					input-class="!h-10"
					:maxlength="MAX_ROLE_NAME_LENGTH"
					:disabled="roles.length >= MAX_ROLES"
					@keyup.enter="addRole"
				/>
				<ButtonStyled color="brand">
					<button class="!h-10" :disabled="!canAddRole" @click="addRole">
						<PlusIcon />
						Add role
					</button>
				</ButtonStyled>
			</div>
		</div>
		<Table :columns="columns" :data="permissions" row-key="id">
			<template #cell-name="{ row }">
				<div v-tooltip="row.description" class="flex min-w-0 flex-col py-2">
					<span class="truncate font-semibold text-primary">{{ row.name }}</span>
				</div>
			</template>
			<template v-for="role in roles" #[`header-${role.id}`]>
				<div class="flex min-w-0 items-center justify-center gap-1">
					<span class="min-w-0 text-center text-sm font-semibold leading-tight">{{ role.label }}</span>
					<ButtonStyled v-if="!role.builtIn" type="transparent" color="red" circular>
						<button v-tooltip="'Delete role'" aria-label="Delete role" @click="removeRole(role)">
							<TrashIcon />
						</button>
					</ButtonStyled>
				</div>
			</template>
			<template v-for="role in roles" #[`cell-${role.id}`]="{ row }">
				<div class="flex justify-center">
					<Toggle v-model="getRoleGrants(role.id)[row.id]" small />
				</div>
			</template>
		</Table>
		<UnsavedChangesPopup
			:original="savedState"
			:modified="modifiedState"
			:saving="isUpdating"
			@save="save"
			@reset="reset"
		/>
	</div>
</template>
