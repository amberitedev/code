<script setup lang="ts">
import type { CoreRole } from '@modrinth/api-client'
import { PencilIcon, PlusIcon, SettingsIcon, ShieldIcon, UserPlusIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	Combobox,
	ConfirmModal,
	NewModal,
	StyledInput,
	Toggle,
	UnsavedChangesPopup,
} from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, onMounted, ref, watch } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { useConnectedCore } from '@/core/connected-core'

const permissions = [
	{ id: 'invite-members', name: 'Invite members', group: 'Members' },
	{ id: 'remove-members', name: 'Remove members', group: 'Members' },
	{ id: 'ban-members', name: 'Ban members', group: 'Members' },
	{ id: 'approve-invites', name: 'Approve invites', group: 'Members' },
	{ id: 'edit-member-roles', name: 'Edit member roles', group: 'Members' },
	{ id: 'manage-roles', name: 'Manage roles', group: 'Members' },
	{ id: 'manage-instances', name: 'Manage instances', group: 'Instances' },
	{ id: 'start-stop-instances', name: 'Start and stop instances', group: 'Instances' },
	{ id: 'restart-instances', name: 'Restart instances', group: 'Instances' },
	{ id: 'manage-mods', name: 'Manage mods', group: 'Content' },
	{ id: 'manage-worlds', name: 'Manage worlds', group: 'Content' },
	{ id: 'manage-files', name: 'Manage files', group: 'Content' },
	{ id: 'manage-backups', name: 'Manage backups', group: 'Content' },
	{ id: 'manage-network', name: 'Manage networking', group: 'Core' },
	{ id: 'read-console', name: 'Read console', group: 'Core' },
	{ id: 'write-console', name: 'Write console', group: 'Core' },
	{ id: 'edit-settings', name: 'Edit Core settings', group: 'Core' },
	{ id: 'view-activity', name: 'View activity', group: 'Core' },
	{ id: 'export-data', name: 'Export data', group: 'Core' },
	{ id: 'dangerous-actions', name: 'Dangerous actions', group: 'Core' },
]
const icons = [
	{ id: 'shield', label: 'Shield', component: ShieldIcon },
	{ id: 'pencil', label: 'Editor', component: PencilIcon },
	{ id: 'settings', label: 'Settings', component: SettingsIcon },
	{ id: 'user', label: 'Member', component: UserPlusIcon },
]
const defaultRoleGrants = [
	'invite-members',
	'remove-members',
	'ban-members',
	'manage-roles',
	'edit-member-roles',
	'approve-invites',
	'manage-instances',
	'start-stop-instances',
	'restart-instances',
	'manage-mods',
	'manage-worlds',
	'manage-files',
	'manage-backups',
	'manage-network',
	'read-console',
	'write-console',
	'edit-settings',
	'view-activity',
	'export-data',
	'dangerous-actions',
]
const defaultRoles: CoreRole[] = [
	{
		id: 'role-admin',
		name: 'Admin',
		description: 'Manages members and Core settings.',
		icon: 'shield',
		grants_json: JSON.stringify(defaultRoleGrants),
		created_at: '1970-01-01T00:00:00.000Z',
		updated_at: '1970-01-01T00:00:00.000Z',
	},
	{
		id: 'role-member',
		name: 'Member',
		description: 'Uses shared Core access.',
		icon: 'user',
		grants_json: JSON.stringify([
			'start-stop-instances',
			'restart-instances',
			'read-console',
			'view-activity',
		]),
		created_at: '1970-01-01T00:00:00.000Z',
		updated_at: '1970-01-01T00:00:00.000Z',
	},
]
const core = useCoreClient()
const connectedCore = useConnectedCore()
const localRoles = useStorage<CoreRole[]>(
	'amberite:core-new-role-prototype',
	cloneRoles(defaultRoles),
)
const roles = ref<CoreRole[]>([])
const selectedId = ref('')
const isLoading = ref(false)
const isSaving = ref(false)
const createModal = ref<InstanceType<typeof NewModal>>()
const deleteModal = ref<InstanceType<typeof ConfirmModal>>()
const draftName = ref('')
const draftDescription = ref('')
const draftIcon = ref('shield')
const draftGrants = ref<string[]>([])
const createName = ref('')
const createDescription = ref('')
const createIcon = ref('shield')
const duplicateFrom = ref('')
const usingLocalRoles = ref(false)

const selectedRole = computed(() => roles.value.find((role) => role.id === selectedId.value))
const groupedPermissions = computed(() => {
	const groups = new Map<string, typeof permissions>()
	for (const permission of permissions)
		groups.set(permission.group, [...(groups.get(permission.group) ?? []), permission])
	return [...groups]
})
const iconComponent = computed(
	() => icons.find((icon) => icon.id === draftIcon.value)?.component ?? ShieldIcon,
)
const duplicateOptions = computed(() =>
	roles.value.map((role) => ({ value: role.id, label: role.name })),
)
const canCreate = computed(() => createName.value.trim().length > 0 && !!duplicateFrom.value)
const originalDraft = computed(() => (selectedRole.value ? roleDraft(selectedRole.value) : {}))
const modifiedDraft = computed(() => ({
	name: draftName.value,
	description: draftDescription.value,
	icon: draftIcon.value,
	grants: draftGrants.value,
}))

watch(
	selectedRole,
	(role) => {
		if (!role) return
		draftName.value = role.name
		draftDescription.value = role.description
		draftIcon.value = role.icon
		draftGrants.value = parseGrants(role.grants_json)
	},
	{ immediate: true },
)

async function load() {
	isLoading.value = true
	try {
		if (!connectedCore.value) {
			loadLocalRoles()
			return
		}
		const configuration = await core.getCoreRoles()
		roles.value = configuration.roles
		usingLocalRoles.value = false
		if (!roles.value.some((role) => role.id === selectedId.value))
			selectedId.value = roles.value[0]?.id ?? ''
	} catch {
		loadLocalRoles()
	} finally {
		isLoading.value = false
	}
}

async function saveSelected() {
	if (!selectedRole.value || !draftName.value.trim()) return
	isSaving.value = true
	try {
		const input = {
			id: selectedRole.value.id,
			name: draftName.value,
			description: draftDescription.value,
			icon: draftIcon.value,
			grants: draftGrants.value,
		}
		const role = usingLocalRoles.value ? saveLocalRole(input) : await core.saveCoreRole(input)
		roles.value = roles.value.map((item) => (item.id === role.id ? role : item))
		persistLocalRoles()
	} finally {
		isSaving.value = false
	}
}

function showCreate() {
	const source = roles.value[0]
	createName.value = ''
	createDescription.value = source?.description ?? ''
	createIcon.value = source?.icon ?? 'shield'
	duplicateFrom.value = source?.id ?? ''
	createModal.value?.show()
}

async function createRole() {
	const source = roles.value.find((role) => role.id === duplicateFrom.value)
	if (!source || !canCreate.value) return
	const input = {
		name: createName.value,
		description: createDescription.value,
		icon: createIcon.value,
		grants: parseGrants(source.grants_json),
	}
	const role = usingLocalRoles.value ? saveLocalRole(input) : await core.saveCoreRole(input)
	roles.value = [...roles.value, role]
	persistLocalRoles()
	selectedId.value = role.id
	createModal.value?.hide()
}

async function deleteSelected() {
	if (!selectedRole.value) return
	if (usingLocalRoles.value) {
		roles.value = roles.value.filter((role) => role.id !== selectedRole.value?.id)
		persistLocalRoles()
	} else {
		await core.retireCoreRole(selectedRole.value.id)
		roles.value = roles.value.filter((role) => role.id !== selectedRole.value?.id)
	}
	selectedId.value = roles.value[0]?.id ?? ''
}

function resetSelected() {
	if (!selectedRole.value) return
	const draft = roleDraft(selectedRole.value)
	draftName.value = draft.name
	draftDescription.value = draft.description
	draftIcon.value = draft.icon
	draftGrants.value = draft.grants
}

function loadLocalRoles() {
	roles.value = cloneRoles(localRoles.value.length ? localRoles.value : defaultRoles)
	usingLocalRoles.value = true
	if (!roles.value.some((role) => role.id === selectedId.value))
		selectedId.value = roles.value[0]?.id ?? ''
}

function persistLocalRoles() {
	if (usingLocalRoles.value) localRoles.value = cloneRoles(roles.value)
}

function saveLocalRole(input: {
	id?: string
	name: string
	description: string
	icon: string
	grants: string[]
}): CoreRole {
	const existing = input.id ? roles.value.find((role) => role.id === input.id) : undefined
	const now = new Date().toISOString()
	return {
		id: existing?.id ?? `local-role-${crypto.randomUUID()}`,
		name: input.name.trim(),
		description: input.description.trim(),
		icon: input.icon,
		grants_json: JSON.stringify(input.grants),
		created_at: existing?.created_at ?? now,
		updated_at: now,
	}
}

function parseGrants(value: string): string[] {
	try {
		return JSON.parse(value) as string[]
	} catch {
		return []
	}
}

function roleDraft(role: CoreRole) {
	return {
		name: role.name,
		description: role.description,
		icon: role.icon,
		grants: parseGrants(role.grants_json),
	}
}

function cloneRoles(source: CoreRole[]): CoreRole[] {
	return source.map((role) => ({
		...role,
		grants_json: JSON.stringify(parseGrants(role.grants_json)),
	}))
}

function toggleGrant(id: string, enabled: boolean) {
	draftGrants.value = enabled
		? [...new Set([...draftGrants.value, id])]
		: draftGrants.value.filter((grant) => grant !== id)
}

watch(connectedCore, () => void load())
onMounted(load)
</script>

<template>
	<div class="flex min-h-0 flex-1 gap-4">
		<aside class="flex w-64 shrink-0 flex-col gap-2 rounded-xl bg-surface-2 p-3">
			<div class="flex items-center justify-between gap-2 px-1">
				<span class="font-semibold text-contrast">Roles</span>
				<span class="text-sm text-secondary">{{ roles.length }} / 4</span>
			</div>
			<button
				v-for="role in roles"
				:key="role.id"
				class="flex items-center gap-3 rounded-lg p-3 text-left transition-colors"
				:class="
					role.id === selectedId
						? 'bg-brand-highlight text-contrast'
						: 'text-primary hover:bg-surface-3'
				"
				@click="selectedId = role.id"
			>
				<component
					:is="icons.find((icon) => icon.id === role.icon)?.component ?? ShieldIcon"
					class="size-5"
				/>
				<span class="min-w-0 truncate font-semibold">{{ role.name }}</span>
			</button>
			<ButtonStyled color="brand" class="mt-auto">
				<button :disabled="roles.length >= 4" @click="showCreate">
					<PlusIcon /> Add role ({{ roles.length }} / 4)
				</button>
			</ButtonStyled>
		</aside>
		<section
			v-if="selectedRole"
			class="flex min-w-0 flex-1 flex-col gap-5 rounded-xl bg-surface-2 p-6"
		>
			<div class="flex items-start justify-between gap-4">
				<div class="flex items-center gap-3">
					<component :is="iconComponent" class="size-8 text-brand" />
					<div>
						<h2 class="m-0 text-2xl font-semibold text-contrast">{{ selectedRole.name }}</h2>
						<p class="m-0 text-secondary">
							{{
								usingLocalRoles
									? 'Saved locally until a Core is linked.'
									: 'Edit this role and its permissions in one place.'
							}}
						</p>
					</div>
				</div>
				<ButtonStyled color="red"
					><button :disabled="roles.length <= 1" @click="deleteModal?.show()">
						Delete role
					</button></ButtonStyled
				>
			</div>
			<div class="grid gap-3 md:grid-cols-2">
				<StyledInput v-model="draftName" label="Role name" :maxlength="20" /><StyledInput
					v-model="draftDescription"
					label="Description"
				/>
			</div>
			<div>
				<span class="font-semibold text-contrast">Icon</span>
				<div class="mt-2 flex flex-wrap gap-2">
					<button
						v-for="icon in icons"
						:key="icon.id"
						class="flex items-center gap-2 rounded-lg border border-solid px-3 py-2"
						:class="draftIcon === icon.id ? 'border-brand bg-brand-highlight' : 'border-surface-5'"
						@click="draftIcon = icon.id"
					>
						<component :is="icon.component" class="size-4" />{{ icon.label }}
					</button>
				</div>
			</div>
			<div v-for="[group, items] in groupedPermissions" :key="group" class="flex flex-col gap-2">
				<h3 class="m-0 text-lg font-semibold text-contrast">{{ group }}</h3>
				<div class="grid gap-2 md:grid-cols-2">
					<label
						v-for="permission in items"
						:key="permission.id"
						class="flex items-center justify-between gap-3 rounded-lg bg-surface-3 px-3 py-2 text-primary"
						><span>{{ permission.name }}</span
						><Toggle
							:model-value="draftGrants.includes(permission.id)"
							@update:model-value="toggleGrant(permission.id, $event)"
					/></label>
				</div>
			</div>
			<div class="sticky bottom-0 flex justify-end bg-surface-2 pt-3">
				<ButtonStyled
					><button :disabled="isSaving" @click="resetSelected">Reset</button></ButtonStyled
				><ButtonStyled color="brand"
					><button :disabled="isSaving || !draftName.trim()" @click="saveSelected">
						Save role
					</button></ButtonStyled
				>
			</div>
		</section>
		<div v-else class="flex flex-1 items-center justify-center text-secondary">
			{{ isLoading ? 'Loading roles…' : 'No roles available.' }}
		</div>
		<NewModal ref="createModal" header="Create role" max-width="34rem"
			><div class="flex flex-col gap-4">
				<StyledInput v-model="createName" label="Role name" :maxlength="20" /><StyledInput
					v-model="createDescription"
					label="Description"
				/><Combobox
					v-model="duplicateFrom"
					:options="duplicateOptions"
					display-value="Duplicate permissions from"
				/>
				<div>
					<span class="font-semibold text-contrast">Icon</span>
					<div class="mt-2 flex gap-2">
						<button
							v-for="icon in icons"
							:key="icon.id"
							class="rounded-lg border border-solid p-2"
							:class="
								createIcon === icon.id ? 'border-brand bg-brand-highlight' : 'border-surface-5'
							"
							@click="createIcon = icon.id"
						>
							<component :is="icon.component" class="size-5" />
						</button>
					</div>
				</div>
			</div>
			<template #actions
				><div class="flex justify-end gap-2">
					<ButtonStyled><button @click="createModal?.hide()">Cancel</button></ButtonStyled
					><ButtonStyled color="brand"
						><button :disabled="!canCreate" @click="createRole">Create role</button></ButtonStyled
					>
				</div></template
			></NewModal
		>
		<ConfirmModal
			ref="deleteModal"
			title="Delete role?"
			description="Members and pending invites keep this role’s current permissions until an approver assigns a new role. They will be marked as needing reassignment."
			proceed-label="Delete role"
			danger
			@proceed="deleteSelected"
		/>
		<UnsavedChangesPopup
			:original="originalDraft"
			:modified="modifiedDraft"
			:saving="isSaving"
			@save="saveSelected"
			@reset="resetSelected"
		/>
	</div>
</template>
