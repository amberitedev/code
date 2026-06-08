<script setup lang="ts">
import { PlusIcon, TrashIcon } from '@modrinth/assets'
import { ButtonStyled, NewModal, StyledInput, Table, Toggle, UnsavedChangesPopup } from '@modrinth/ui'
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

interface RolePermissionsState {
	roles: RoleConfig[]
	grants: Record<string, Record<string, boolean>>
}

const permissions: PermissionRow[] = [
	{ id: 'invite-members', name: 'Invite members', description: 'Create group invites.' },
	{ id: 'remove-members', name: 'Remove members', description: 'Kick non-owner members.' },
	{ id: 'ban-members', name: 'Ban members', description: 'Ban users from the group.' },
	{ id: 'manage-content', name: 'Manage shared content', description: 'Edit group content.' },
	{ id: 'view-activity', name: 'View activity', description: 'Read member activity.' },
]

const defaultState: RolePermissionsState = {
	roles: [
		{ id: 'admin', label: 'Admin', builtIn: true },
		{ id: 'member', label: 'Member', builtIn: true },
	],
	grants: {
		admin: {
			'invite-members': true,
			'remove-members': true,
			'ban-members': true,
			'manage-content': true,
			'view-activity': true,
		},
		member: { 'view-activity': true },
	},
}

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const savedState = useStorage<RolePermissionsState>('amberite-core-role-permissions', defaultState)
const roles = ref<RoleConfig[]>(cloneRoles(savedState.value.roles))
const grants = ref(cloneGrants(savedState.value.grants))
const newRoleName = ref('')
const isUpdating = ref(false)

const columns = computed(() => [
	{ key: 'name', label: 'Permission' },
	...roles.value.map((role) => ({
		key: role.id,
		label: role.label,
		align: 'center' as const,
		width: '8rem',
	})),
	{ key: 'actions', label: '', align: 'right' as const, width: '5rem' },
])
const modifiedState = computed(() => ({ roles: roles.value, grants: grants.value }))

function cloneRoles(source: RoleConfig[]) {
	return source.map((role) => ({ ...role }))
}

function cloneGrants(source: Record<string, Record<string, boolean>>) {
	return Object.fromEntries(
		Object.entries(source).map(([role, roleGrants]) => [role, { ...roleGrants }]),
	)
}

function normalizeRoleId(name: string) {
	return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
}

function addRole() {
	const label = newRoleName.value.trim()
	const id = normalizeRoleId(label)
	if (!label || !id || roles.value.some((role) => role.id === id)) return
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
	savedState.value = {
		roles: cloneRoles(roles.value),
		grants: cloneGrants(grants.value),
	}
	isUpdating.value = false
}

function reset() {
	roles.value = cloneRoles(savedState.value.roles)
	grants.value = cloneGrants(savedState.value.grants)
	newRoleName.value = ''
}

function show() {
	reset()
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
	<NewModal ref="modal" header="Manage roles" width="min(840px, calc(95vw - 2rem))" scrollable>
		<div class="flex flex-col gap-4">
			<p class="m-0 text-secondary">
				Configure role permissions. Owner access is always full and is not editable here.
			</p>
			<Table :columns="columns" :data="permissions" row-key="id" table-min-width="680px">
				<template #header>
					<div class="flex flex-col gap-2 sm:flex-row">
						<StyledInput
							v-model="newRoleName"
							placeholder="New role, e.g. Guest"
							wrapper-class="min-w-0 flex-1"
							@keyup.enter="addRole"
						/>
						<ButtonStyled color="brand">
							<button :disabled="!newRoleName.trim()" @click="addRole">
								<PlusIcon />
								Add role
							</button>
						</ButtonStyled>
					</div>
				</template>
				<template #cell-name="{ row }">
					<div class="flex min-w-0 flex-col py-2">
						<span class="font-semibold text-primary">{{ row.name }}</span>
						<span class="text-sm text-secondary">{{ row.description }}</span>
					</div>
				</template>
				<template v-for="role in roles" #[`header-${role.id}`]>
					<div class="flex items-center justify-center gap-1">
						<span>{{ role.label }}</span>
						<ButtonStyled v-if="!role.builtIn" type="transparent" color="red" circular>
							<button v-tooltip="'Delete role'" aria-label="Delete role" @click="removeRole(role)">
								<TrashIcon />
							</button>
						</ButtonStyled>
					</div>
				</template>
				<template v-for="role in roles" #[`cell-${role.id}`]="{ row }">
					<div class="flex justify-center">
						<Toggle v-model="grants[role.id][row.id]" small />
					</div>
				</template>
			</Table>
		</div>
		<UnsavedChangesPopup
			:original="savedState"
			:modified="modifiedState"
			:saving="isUpdating"
			@save="save"
			@reset="reset"
		/>
	</NewModal>
</template>
